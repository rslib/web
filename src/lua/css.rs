//! CSS processing module (rs.css)
//!
//! Provides:
//! - rs.css.concat(input, output, options?) - Concatenate CSS files with optional minification
//! - rs.css.bundle(input, output, options?) - Bundle CSS files with @import resolution (LightningCSS)
//! - rs.css.bundle_many(input, output_dir, options?) - Bundle multiple CSS entries to separate files
//! - CSS purging (dead code elimination) - Remove unused CSS rules based on HTML output

use crate::assets::minify_css;
use crate::lua::async_io::{AsyncIOTask, runtime};
use crate::tracker::SharedTracker;
use lightningcss::bundler::{Bundler as CssBundler, FileProvider};
use lightningcss::printer::PrinterOptions;
use lightningcss::stylesheet::{MinifyOptions, ParserOptions};
use lightningcss::targets::Targets;
use mlua::{Lua, Result, Table, Value};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Create the rs.css module table
pub fn create_module(lua: &Lua, project_root: &Path, tracker: SharedTracker) -> Result<Table> {
    let css_module = lua.create_table()?;
    let root = project_root.to_path_buf();

    // rs.css.concat(paths_or_pattern, output_path, options?) - async
    // Concatenates CSS files with optional minification and purging
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let concat_fn = lua.create_function(
        move |lua, (input, output_path, options): (Value, String, Option<Table>)| {
            let opts = CssBundleOptions::from_lua(&options, lua)?;

            let files = collect_files(&input, &root_clone, "css")?;
            if files.is_empty() {
                return Err(mlua::Error::external("No CSS files found"));
            }

            let output = resolve_output_path(&output_path, &root_clone);
            let tracker = tracker_clone.clone();

            let handle = runtime().spawn(async move {
                let contents = read_files_async(&files, &tracker).await?;
                let buffer: String = contents.join("\n");

                let minified = if opts.minify {
                    tokio::task::spawn_blocking(move || minify_css(&buffer))
                        .await
                        .map_err(|e| e.to_string())??
                } else {
                    buffer
                };

                // Apply CSS purging if enabled
                let output_content = if opts.purge {
                    // Merge thread locals first to ensure all HTML writes are visible
                    tracker.merge_thread_locals();

                    // Collect HTML from all written files
                    let html_content = collect_html_from_tracker(&tracker);

                    if html_content.is_empty() {
                        log::warn!("CSS purge: No HTML files found in tracker, keeping all CSS");
                        minified
                    } else {
                        // Extract used selectors from HTML
                        let used = extract_used_selectors(&html_content);
                        let safelist = compile_safelist(&opts.safelist);

                        let original_len = minified.len();
                        let purged = purge_css(&minified, &used, &safelist);
                        let purged_len = purged.len();

                        if original_len > purged_len {
                            let saved = original_len - purged_len;
                            let percent = (saved as f64 / original_len as f64) * 100.0;
                            log::info!(
                                "CSS purge: removed {} bytes ({:.1}% reduction)",
                                saved,
                                percent
                            );
                        }

                        purged
                    }
                } else {
                    minified
                };

                write_output_async(&output, &output_content, &tracker).await
            });

            Ok(AsyncIOTask::new(handle))
        },
    )?;
    css_module.set("concat", concat_fn)?;

    // rs.css.bundle(paths_or_pattern, output_path, options?) - async
    // Bundles CSS files using LightningCSS with @import resolution
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let bundle_fn = lua.create_function(
        move |lua, (input, output_path, options): (Value, String, Option<Table>)| {
            let opts = CssBundleOptions::from_lua(&options, lua)?;
            let entries = collect_entries(&input, &root_clone)?;

            if entries.is_empty() {
                return Err(mlua::Error::external("No CSS entry files found"));
            }

            let output = resolve_output_path(&output_path, &root_clone);
            let tracker = tracker_clone.clone();

            let handle = runtime().spawn(async move {
                bundle_css_with_lightningcss(&entries, &output, &opts, &tracker).await
            });

            Ok(AsyncIOTask::new(handle))
        },
    )?;
    css_module.set("bundle", bundle_fn)?;

    // rs.css.bundle_many(paths_or_pattern, output_dir, options?) - async
    // Bundles multiple CSS entries to separate output files
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let bundle_many_fn = lua.create_function(
        move |lua, (input, output_dir, options): (Value, String, Option<Table>)| {
            let opts = CssBundleOptions::from_lua(&options, lua)?;
            let entries = collect_entries(&input, &root_clone)?;

            if entries.is_empty() {
                return Err(mlua::Error::external("No CSS entry files found"));
            }

            let output = resolve_output_path(&output_dir, &root_clone);
            let tracker = tracker_clone.clone();

            let handle = runtime().spawn(async move {
                bundle_many_css_with_lightningcss(&entries, &output, &opts, &tracker).await
            });

            Ok(AsyncIOTask::new(handle))
        },
    )?;
    css_module.set("bundle_many", bundle_many_fn)?;

    // rs.css.purge(css_path, options?) - async
    // Purges unused CSS rules from an existing CSS file based on HTML/JS output
    // Call this in after_build after HTML is generated
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let purge_fn =
        lua.create_function(move |_lua, (css_path, options): (String, Option<Table>)| {
            let safelist: Vec<String> = options
                .as_ref()
                .and_then(|t| {
                    t.get::<Table>("safelist").ok().map(|tbl| {
                        tbl.pairs::<i64, String>()
                            .filter_map(|r| r.ok().map(|(_, v)| v))
                            .collect()
                    })
                })
                .unwrap_or_default();

            let css_file = resolve_output_path(&css_path, &root_clone);
            let tracker = tracker_clone.clone();

            let handle = runtime().spawn(async move {
                // Read existing CSS
                let css_content = tokio::fs::read_to_string(&css_file)
                    .await
                    .map_err(|e| format!("Failed to read CSS file: {}", e))?;

                // Merge thread locals to ensure all writes are visible
                tracker.merge_thread_locals();

                // Collect content from HTML and JS files
                let content = collect_content_for_purge(&tracker);

                if content.is_empty() {
                    log::warn!("CSS purge: No HTML/JS files found in tracker, keeping all CSS");
                    return Ok(());
                }

                // Extract used selectors
                let used = extract_used_selectors(&content);
                let safelist_regex = compile_safelist(&safelist);

                let original_len = css_content.len();
                let purged = purge_css(&css_content, &used, &safelist_regex);
                let purged_len = purged.len();

                if original_len > purged_len {
                    let saved = original_len - purged_len;
                    let percent = (saved as f64 / original_len as f64) * 100.0;
                    log::info!(
                        "CSS purge: removed {} bytes ({:.1}% reduction)",
                        saved,
                        percent
                    );

                    // Write purged CSS back
                    tokio::fs::write(&css_file, &purged)
                        .await
                        .map_err(|e| format!("Failed to write purged CSS: {}", e))?;

                    // Update tracker
                    let canonical = css_file.canonicalize().unwrap_or(css_file);
                    tracker.record_write_async(canonical, purged.as_bytes());
                }

                Ok(())
            });

            Ok(AsyncIOTask::new(handle))
        })?;
    css_module.set("purge", purge_fn)?;

    Ok(css_module)
}

/// Options for CSS bundling
#[derive(Debug, Clone)]
struct CssBundleOptions {
    minify: bool,
    sourcemap: bool,
    nesting: bool,
    /// Remove unused CSS rules based on HTML output (default: true)
    purge: bool,
    /// Additional safelist patterns to always keep (regex patterns)
    safelist: Vec<String>,
}

impl Default for CssBundleOptions {
    fn default() -> Self {
        Self {
            minify: false,
            sourcemap: false,
            nesting: true,
            // Default to false because purging requires HTML files to exist first
            // (CSS must be built in after_build hook for purging to work)
            purge: false,
            safelist: Vec::new(),
        }
    }
}

impl CssBundleOptions {
    fn from_lua(options: &Option<Table>, _lua: &Lua) -> mlua::Result<Self> {
        let mut opts = Self::default();

        if let Some(t) = options {
            if let Ok(minify) = t.get::<bool>("minify") {
                opts.minify = minify;
            }
            if let Ok(sourcemap) = t.get::<bool>("sourcemap") {
                opts.sourcemap = sourcemap;
            }
            if let Ok(nesting) = t.get::<bool>("nesting") {
                opts.nesting = nesting;
            }
            if let Ok(purge) = t.get::<bool>("purge") {
                opts.purge = purge;
            }
            // safelist can be a table of patterns
            if let Ok(safelist) = t.get::<Table>("safelist") {
                for (_, pattern) in safelist.pairs::<i64, String>().flatten() {
                    opts.safelist.push(pattern);
                }
            }
        }

        Ok(opts)
    }
}

/// Collect entry files from input (single path, glob pattern, or array of paths)
fn collect_entries(input: &Value, root: &Path) -> mlua::Result<Vec<PathBuf>> {
    match input {
        Value::Table(table) => {
            let mut files = Vec::new();
            for pair in table.pairs::<i64, String>() {
                let (_, path_str) = pair
                    .map_err(|e| mlua::Error::external(format!("Invalid path in array: {}", e)))?;
                let path = if Path::new(&path_str).is_absolute() {
                    PathBuf::from(&path_str)
                } else {
                    root.join(&path_str)
                };
                if path.exists() && path.is_file() {
                    files.push(path);
                } else {
                    return Err(mlua::Error::external(format!(
                        "CSS entry file not found: {}",
                        path_str
                    )));
                }
            }
            Ok(files)
        }
        Value::String(pattern_str) => {
            let pattern = pattern_str
                .to_str()
                .map_err(|e| mlua::Error::external(format!("Invalid pattern: {}", e)))?
                .to_string();

            // Check if it's a single file path (not a glob pattern)
            let path = if Path::new(&pattern).is_absolute() {
                PathBuf::from(&pattern)
            } else {
                root.join(&pattern)
            };

            // If it exists as a file, return it directly
            if path.exists() && path.is_file() {
                return Ok(vec![path]);
            }

            // Otherwise treat as glob pattern
            let glob_pattern = if Path::new(&pattern).is_absolute() {
                pattern
            } else {
                root.join(&pattern).to_string_lossy().to_string()
            };

            let mut files: Vec<PathBuf> = glob::glob(&glob_pattern)
                .map_err(|e| mlua::Error::external(format!("Invalid glob: {}", e)))?
                .filter_map(|e| e.ok())
                .filter(|p| p.is_file())
                .collect();
            files.sort();
            Ok(files)
        }
        _ => Err(mlua::Error::external(
            "css.bundle: first argument must be path, glob pattern, or array of paths",
        )),
    }
}

/// Bundle CSS files with LightningCSS (single output)
async fn bundle_css_with_lightningcss(
    entries: &[PathBuf],
    output: &Path,
    opts: &CssBundleOptions,
    tracker: &SharedTracker,
) -> std::result::Result<(), String> {
    // Create output directory
    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    let opts = opts.clone();
    let entries = entries.to_vec();
    let output = output.to_path_buf();
    let tracker = tracker.clone();

    // LightningCSS bundler is sync, run in blocking task
    tokio::task::spawn_blocking(move || {
        let fs = FileProvider::new();
        let mut combined_css = String::new();

        for entry in &entries {
            let parser_options = ParserOptions::default();
            let mut bundler = CssBundler::new(&fs, None, parser_options);

            let stylesheet = bundler
                .bundle(entry)
                .map_err(|e| format!("Failed to bundle {:?}: {}", entry, e))?;

            // Minify if requested
            let mut stylesheet = stylesheet;
            if opts.minify {
                let targets = Targets::default();
                stylesheet
                    .minify(MinifyOptions {
                        targets,
                        ..Default::default()
                    })
                    .map_err(|e| format!("Failed to minify: {:?}", e))?;
            }

            // Convert to string
            let printer_options = PrinterOptions {
                minify: opts.minify,
                ..Default::default()
            };
            let result = stylesheet
                .to_css(printer_options)
                .map_err(|e| format!("Failed to serialize CSS: {:?}", e))?;

            combined_css.push_str(&result.code);
            combined_css.push('\n');
        }

        // Apply CSS purging if enabled
        let final_css = if opts.purge {
            // Merge thread locals first to ensure all HTML writes are visible
            tracker.merge_thread_locals();

            // Collect HTML from all written files
            let html_content = collect_html_from_tracker(&tracker);

            if html_content.is_empty() {
                log::warn!("CSS purge: No HTML files found in tracker, keeping all CSS");
                combined_css
            } else {
                // Extract used selectors from HTML
                let used = extract_used_selectors(&html_content);
                let safelist = compile_safelist(&opts.safelist);

                let original_len = combined_css.len();
                let purged = purge_css(&combined_css, &used, &safelist);
                let purged_len = purged.len();

                if original_len > purged_len {
                    let saved = original_len - purged_len;
                    let percent = (saved as f64 / original_len as f64) * 100.0;
                    log::info!(
                        "CSS purge: removed {} bytes ({:.1}% reduction)",
                        saved,
                        percent
                    );
                }

                purged
            }
        } else {
            combined_css
        };

        // Write output
        std::fs::write(&output, &final_css)
            .map_err(|e| format!("Failed to write output: {}", e))?;

        // Track the write
        let canonical = output.canonicalize().unwrap_or_else(|_| output.clone());
        tracker.record_write(canonical, final_css.as_bytes());

        Ok(())
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

/// Bundle CSS files with LightningCSS (multiple outputs)
async fn bundle_many_css_with_lightningcss(
    entries: &[PathBuf],
    output_dir: &Path,
    opts: &CssBundleOptions,
    tracker: &SharedTracker,
) -> std::result::Result<(), String> {
    // Create output directory
    tokio::fs::create_dir_all(output_dir)
        .await
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    let opts = opts.clone();
    let entries = entries.to_vec();
    let output_dir = output_dir.to_path_buf();
    let tracker = tracker.clone();

    // LightningCSS bundler is sync, run in blocking task
    tokio::task::spawn_blocking(move || {
        let fs = FileProvider::new();

        // Pre-compute purge data if enabled
        let (used, safelist) = if opts.purge {
            tracker.merge_thread_locals();
            let html_content = collect_html_from_tracker(&tracker);
            if html_content.is_empty() {
                log::warn!("CSS purge: No HTML files found in tracker, keeping all CSS");
                (None, Vec::new())
            } else {
                (
                    Some(extract_used_selectors(&html_content)),
                    compile_safelist(&opts.safelist),
                )
            }
        } else {
            (None, Vec::new())
        };

        for entry in &entries {
            let parser_options = ParserOptions::default();
            let mut bundler = CssBundler::new(&fs, None, parser_options);

            let stylesheet = bundler
                .bundle(entry)
                .map_err(|e| format!("Failed to bundle {:?}: {}", entry, e))?;

            // Minify if requested
            let mut stylesheet = stylesheet;
            if opts.minify {
                let targets = Targets::default();
                stylesheet
                    .minify(MinifyOptions {
                        targets,
                        ..Default::default()
                    })
                    .map_err(|e| format!("Failed to minify: {:?}", e))?;
            }

            // Convert to string
            let printer_options = PrinterOptions {
                minify: opts.minify,
                ..Default::default()
            };
            let result = stylesheet
                .to_css(printer_options)
                .map_err(|e| format!("Failed to serialize CSS: {:?}", e))?;

            // Apply purging if enabled and we have HTML content
            let final_css = if let Some(ref used) = used {
                purge_css(&result.code, used, &safelist)
            } else {
                result.code
            };

            // Determine output filename
            let filename = entry
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("output.css");
            let output_path = output_dir.join(filename);

            // Write output
            std::fs::write(&output_path, &final_css)
                .map_err(|e| format!("Failed to write output: {}", e))?;

            // Track the write
            let canonical = output_path.canonicalize().unwrap_or(output_path);
            tracker.record_write(canonical, final_css.as_bytes());
        }

        Ok(())
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?
}

/// Collect files from input (glob pattern or array of paths)
fn collect_files(input: &Value, root: &Path, ext: &str) -> mlua::Result<Vec<PathBuf>> {
    match input {
        Value::Table(table) => {
            let mut files = Vec::new();
            for pair in table.pairs::<i64, String>() {
                let (_, path_str) = pair
                    .map_err(|e| mlua::Error::external(format!("Invalid path in array: {}", e)))?;
                let path = if Path::new(&path_str).is_absolute() {
                    PathBuf::from(&path_str)
                } else {
                    root.join(&path_str)
                };
                if path.exists() && path.is_file() {
                    files.push(path);
                } else {
                    return Err(mlua::Error::external(format!(
                        "{} file not found: {}",
                        ext.to_uppercase(),
                        path_str
                    )));
                }
            }
            Ok(files)
        }
        Value::String(pattern_str) => {
            let pattern = pattern_str
                .to_str()
                .map_err(|e| mlua::Error::external(format!("Invalid pattern: {}", e)))?
                .to_string();
            let glob_pattern = if Path::new(&pattern).is_absolute() {
                pattern.clone()
            } else {
                root.join(&pattern).to_string_lossy().to_string()
            };

            let mut files: Vec<PathBuf> = glob::glob(&glob_pattern)
                .map_err(|e| mlua::Error::external(format!("Invalid glob: {}", e)))?
                .filter_map(|e| e.ok())
                .filter(|p| p.is_file())
                .collect();
            files.sort();
            Ok(files)
        }
        _ => Err(mlua::Error::external(
            "css.concat: first argument must be glob pattern or array of paths",
        )),
    }
}

fn resolve_output_path(output_path: &str, root: &Path) -> PathBuf {
    if Path::new(output_path).is_absolute() {
        PathBuf::from(output_path)
    } else {
        root.join(output_path)
    }
}

async fn read_files_async(
    files: &[PathBuf],
    tracker: &SharedTracker,
) -> std::result::Result<Vec<String>, String> {
    let futures: Vec<_> = files
        .iter()
        .map(|path| {
            let path = path.clone();
            let tracker = tracker.clone();
            async move {
                let content = tokio::fs::read_to_string(&path)
                    .await
                    .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
                let canonical = path.canonicalize().unwrap_or(path);
                tracker.record_read(canonical, content.as_bytes());
                Ok::<_, String>(content)
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;
    results.into_iter().collect()
}

async fn write_output_async(
    output: &Path,
    content: &str,
    tracker: &SharedTracker,
) -> std::result::Result<(), String> {
    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    tokio::fs::write(output, content)
        .await
        .map_err(|e| format!("Failed to write: {}", e))?;

    let canonical = output
        .canonicalize()
        .unwrap_or_else(|_| output.to_path_buf());
    tracker.record_write(canonical, content.as_bytes());
    Ok(())
}

// =============================================================================
// CSS Purging (Dead Code Elimination)
// =============================================================================

/// Extract all HTML attributes for attribute selector matching
static ATTR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"([a-zA-Z][a-zA-Z0-9_-]*)\s*=\s*["']([^"']+)["']"#).unwrap());
/// Extract data-* attribute names
static DATA_ATTR_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(data-[a-zA-Z0-9_-]+)"#).unwrap());

/// Regex for extracting selector components (classes, IDs, tags)
static SELECTOR_CLASS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\.([a-zA-Z_-][\w-]*)").unwrap());
static SELECTOR_ID_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"#([a-zA-Z_-][\w-]*)").unwrap());
static SELECTOR_TAG_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:^|[\s>+~,])([a-zA-Z][a-zA-Z0-9-]*)").unwrap());
/// Regex for CSS attribute selectors [attr], [attr=value], [attr~=value], etc.
static CSS_ATTR_SELECTOR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\[([a-zA-Z][a-zA-Z0-9_-]*)(?:[~|^$*]?=["']?([^"'\]]+)["']?)?\]"#).unwrap()
});

/// Regex for extracting all potential selector words from content (PurgeCSS-style)
/// Matches any word that could be a CSS class name or ID
static WORD_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"[a-zA-Z_][a-zA-Z0-9_-]*").unwrap());

/// Extract used CSS selectors from HTML/JS content (PurgeCSS-style approach)
/// Instead of trying to parse HTML structure, extract ALL potential selector words
fn extract_used_selectors(content: &str) -> UsedSelectors {
    let mut selectors = UsedSelectors::default();

    // PurgeCSS-style: Extract ALL words that could be selectors
    // This is more conservative and rarely misses anything
    for cap in WORD_REGEX.captures_iter(content) {
        if let Some(word) = cap.get(0) {
            let w = word.as_str();
            // Add full word (case-sensitive)
            selectors.words.insert(w.to_string());
            // Also add lowercase for tag matching
            selectors.words.insert(w.to_lowercase());

            // Also split hyphenated words and add each part
            // e.g., "solution-tabs" -> "solution", "tabs"
            // This is important because CSS may have .tabs but HTML has .solution-tabs
            for part in w.split('-') {
                if !part.is_empty() {
                    selectors.words.insert(part.to_string());
                    selectors.words.insert(part.to_lowercase());
                }
            }
            // Also split on underscores
            for part in w.split('_') {
                if !part.is_empty() {
                    selectors.words.insert(part.to_string());
                    selectors.words.insert(part.to_lowercase());
                }
            }
        }
    }

    // Also extract from HTML attributes for attribute selector matching
    for cap in ATTR_REGEX.captures_iter(content) {
        if let Some(attr_name) = cap.get(1) {
            selectors
                .attributes
                .insert(attr_name.as_str().to_lowercase());
        }
    }

    // Extract data-* attribute names
    for cap in DATA_ATTR_REGEX.captures_iter(content) {
        if let Some(attr) = cap.get(1) {
            selectors.attributes.insert(attr.as_str().to_lowercase());
        }
    }

    log::debug!(
        "CSS purge: extracted {} unique words, {} attributes",
        selectors.words.len(),
        selectors.attributes.len()
    );

    selectors
}

#[derive(Debug, Default)]
struct UsedSelectors {
    /// All words that could be CSS selectors (classes, IDs, tags)
    words: HashSet<String>,
    /// HTML attribute names (lowercase)
    attributes: HashSet<String>,
}

impl UsedSelectors {
    /// Check if a CSS selector uses any of the used selectors (PurgeCSS-style)
    fn is_selector_used(&self, selector: &str, safelist: &[Regex]) -> bool {
        // Check safelist first
        for pattern in safelist {
            if pattern.is_match(selector) {
                return true;
            }
        }

        // Always keep @-rules (keyframes, media, etc.) - they'll be filtered by their contents
        if selector.trim().starts_with('@') {
            return true;
        }

        // Always keep :root, html, body, * selectors
        let trimmed = selector.trim();
        if trimmed == "*"
            || trimmed == ":root"
            || trimmed.starts_with(":root")
            || trimmed == "html"
            || trimmed == "body"
        {
            return true;
        }

        // Check for attribute selectors [attr] or [attr=value]
        // Keep if the attribute exists in HTML or is a common attribute
        for cap in CSS_ATTR_SELECTOR_REGEX.captures_iter(selector) {
            if let Some(attr_name) = cap.get(1) {
                let attr = attr_name.as_str().to_lowercase();
                if self.attributes.contains(&attr) {
                    return true;
                }
                // Common attributes that should always be kept
                if matches!(
                    attr.as_str(),
                    "type"
                        | "name"
                        | "value"
                        | "checked"
                        | "disabled"
                        | "open"
                        | "hidden"
                        | "readonly"
                        | "required"
                        | "selected"
                        | "href"
                        | "src"
                        | "role"
                        | "aria-expanded"
                        | "aria-hidden"
                        | "aria-selected"
                        | "data-"
                ) || attr.starts_with("data-")
                    || attr.starts_with("aria-")
                {
                    return true;
                }
            }
        }

        // PurgeCSS-style: Extract all class names and IDs from the selector
        // and check if ANY of them appear in our content words
        let mut has_any_selector = false;

        // Check classes (.class-name)
        for cap in SELECTOR_CLASS_REGEX.captures_iter(selector) {
            has_any_selector = true;
            if let Some(class) = cap.get(1) {
                let class_str = class.as_str();
                // Check full class name
                if self.words.contains(class_str) {
                    return true;
                }
                // Also check parts of hyphenated class names
                // e.g., .tab-buttons should match if "tab" or "buttons" is in content
                for part in class_str.split('-') {
                    if !part.is_empty() && self.words.contains(part) {
                        return true;
                    }
                }
                for part in class_str.split('_') {
                    if !part.is_empty() && self.words.contains(part) {
                        return true;
                    }
                }
            }
        }

        // Check IDs (#id-name)
        for cap in SELECTOR_ID_REGEX.captures_iter(selector) {
            has_any_selector = true;
            if let Some(id) = cap.get(1) {
                let id_str = id.as_str();
                if self.words.contains(id_str) {
                    return true;
                }
                // Also check parts of hyphenated IDs
                for part in id_str.split('-') {
                    if !part.is_empty() && self.words.contains(part) {
                        return true;
                    }
                }
                for part in id_str.split('_') {
                    if !part.is_empty() && self.words.contains(part) {
                        return true;
                    }
                }
            }
        }

        // Check tags - always keep common HTML tags
        for cap in SELECTOR_TAG_REGEX.captures_iter(selector) {
            has_any_selector = true;
            if let Some(tag) = cap.get(1) {
                let tag_lower = tag.as_str().to_lowercase();
                // Always keep common HTML tags
                if is_common_html_tag(&tag_lower) {
                    return true;
                }
                // Also check if tag appears in content
                if self.words.contains(&tag_lower) {
                    return true;
                }
            }
        }

        // If no class/id/tag found in selector (e.g., pure pseudo selectors), keep it
        !has_any_selector
    }
}

/// Purge unused CSS rules from the given CSS content using proper state machine parsing
fn purge_css(css: &str, used: &UsedSelectors, safelist: &[Regex]) -> String {
    let mut result = String::with_capacity(css.len());
    let chars: Vec<char> = css.chars().collect();
    let len = chars.len();

    let mut i = 0;
    let mut current_selector = String::new();
    let mut current_body = String::new();
    let mut depth = 0;

    while i < len {
        let c = chars[i];

        // Handle comments /* ... */
        if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            let comment_start = i;
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2; // skip */
            let comment: String = chars[comment_start..i.min(len)].iter().collect();
            if depth == 0 {
                current_selector.push_str(&comment);
            } else {
                current_body.push_str(&comment);
            }
            continue;
        }

        // Handle strings (both single and double quoted)
        if c == '"' || c == '\'' {
            let quote = c;
            let string_start = i;
            i += 1;
            while i < len {
                if chars[i] == '\\' && i + 1 < len {
                    i += 2; // skip escaped char
                } else if chars[i] == quote {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            let string_content: String = chars[string_start..i].iter().collect();
            if depth == 0 {
                current_selector.push_str(&string_content);
            } else {
                current_body.push_str(&string_content);
            }
            continue;
        }

        match c {
            '{' => {
                if depth == 0 {
                    // Starting a new rule
                    depth = 1;
                } else {
                    depth += 1;
                    current_body.push(c);
                }
            }
            '}' => {
                if depth <= 1 {
                    // End of top-level rule - decide whether to keep it
                    depth = 0;
                    let selector = current_selector.trim();

                    if selector.is_empty() {
                        // Nothing to do, reset
                    } else if selector.starts_with('@') {
                        // @-rule handling
                        if selector.contains("@media")
                            || selector.contains("@supports")
                            || selector.contains("@layer")
                            || selector.contains("@container")
                        {
                            // Recursively purge inner rules
                            let purged_body = purge_css(&current_body, used, safelist);
                            if !purged_body.trim().is_empty() {
                                result.push_str(selector);
                                result.push('{');
                                result.push_str(&purged_body);
                                result.push('}');
                            }
                        } else {
                            // Keep @keyframes, @font-face, @import, @charset as-is
                            result.push_str(selector);
                            result.push('{');
                            result.push_str(&current_body);
                            result.push('}');
                        }
                    } else {
                        // Regular rule - filter unused selectors from comma-separated list
                        let selectors: Vec<&str> = selector.split(',').collect();
                        let used_selectors: Vec<&str> = selectors
                            .iter()
                            .filter(|s| used.is_selector_used(s, safelist))
                            .copied()
                            .collect();

                        // Debug: log removed selectors
                        for s in &selectors {
                            if !used.is_selector_used(s, safelist) {
                                log::debug!("CSS purge: removing selector '{}'", s.trim());
                            }
                        }

                        if !used_selectors.is_empty() {
                            result.push_str(&used_selectors.join(","));
                            result.push('{');
                            result.push_str(&current_body);
                            result.push('}');
                        }
                    }

                    current_selector.clear();
                    current_body.clear();
                } else {
                    depth -= 1;
                    current_body.push(c);
                }
            }
            _ => {
                if depth == 0 {
                    current_selector.push(c);
                } else {
                    current_body.push(c);
                }
            }
        }

        i += 1;
    }

    result
}

/// Collect HTML content from all written HTML files in tracker
fn collect_html_from_tracker(tracker: &SharedTracker) -> String {
    let writes = tracker.get_writes();
    let mut html_content = String::new();

    for (path, _) in writes {
        if path.extension().and_then(|e| e.to_str()) == Some("html")
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            html_content.push_str(&content);
            html_content.push('\n');
        }
    }

    html_content
}

/// Collect content from HTML and JS files for CSS purging
fn collect_content_for_purge(tracker: &SharedTracker) -> String {
    let writes = tracker.get_writes();
    let mut content = String::new();
    let mut html_count = 0;
    let mut js_count = 0;

    for path in writes.keys() {
        let ext = path.extension().and_then(|e| e.to_str());
        if ext == Some("html") {
            html_count += 1;
            if let Ok(file_content) = std::fs::read_to_string(path) {
                content.push_str(&file_content);
                content.push('\n');
            }
        } else if ext == Some("js") {
            js_count += 1;
            if let Ok(file_content) = std::fs::read_to_string(path) {
                content.push_str(&file_content);
                content.push('\n');
            }
        }
    }

    log::debug!(
        "CSS purge: collected content from {} HTML files, {} JS files ({} bytes total)",
        html_count,
        js_count,
        content.len()
    );

    content
}

/// Compile safelist patterns to regex
fn compile_safelist(patterns: &[String]) -> Vec<Regex> {
    patterns.iter().filter_map(|p| Regex::new(p).ok()).collect()
}

/// Check if a tag is a common HTML tag that should always be kept
/// These tags might appear in markdown content or dynamically generated HTML
fn is_common_html_tag(tag: &str) -> bool {
    matches!(
        tag,
        // Document structure
        "html" | "head" | "body" | "main" | "header" | "footer" | "nav" | "aside" |
        "article" | "section" | "div" | "span" |
        // Headings
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" |
        // Text content
        "p" | "a" | "strong" | "b" | "em" | "i" | "u" | "s" | "del" | "ins" |
        "mark" | "small" | "sub" | "sup" | "abbr" | "cite" | "code" | "kbd" |
        "samp" | "var" | "time" | "q" | "blockquote" | "pre" | "address" |
        // Lists
        "ul" | "ol" | "li" | "dl" | "dt" | "dd" |
        // Tables
        "table" | "thead" | "tbody" | "tfoot" | "tr" | "th" | "td" | "caption" |
        "colgroup" | "col" |
        // Forms
        "form" | "fieldset" | "legend" | "input" | "button" | "select" |
        "option" | "optgroup" | "textarea" | "label" | "output" | "datalist" |
        // Media
        "img" | "picture" | "source" | "video" | "audio" | "track" |
        "figure" | "figcaption" | "canvas" | "svg" | "iframe" | "embed" | "object" |
        // Interactive
        "details" | "summary" | "dialog" | "menu" |
        // Other
        "hr" | "br" | "wbr" | "template" | "slot" | "noscript"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_css_minify() {
        let input = r#"
            .class {
                color: red;
                margin: 0;
            }
        "#;
        let result = minify_css(input).unwrap();
        assert!(!result.contains('\n') || result.len() < input.len());
    }

    #[test]
    fn test_extract_selectors_from_html() {
        let html = r#"
            <div class="container main-content" id="app">
                <span class="text-red">Hello</span>
                <button class="btn btn-primary">Click</button>
            </div>
        "#;
        let selectors = extract_used_selectors(html);

        // Words extracted from content (PurgeCSS-style)
        assert!(selectors.words.contains("container"));
        assert!(selectors.words.contains("main")); // split from main-content
        assert!(selectors.words.contains("content")); // split from main-content
        assert!(selectors.words.contains("text")); // split from text-red
        assert!(selectors.words.contains("red")); // split from text-red
        assert!(selectors.words.contains("btn"));
        assert!(selectors.words.contains("primary")); // split from btn-primary
        assert!(selectors.words.contains("app"));
        assert!(selectors.words.contains("div"));
        assert!(selectors.words.contains("span"));
        assert!(selectors.words.contains("button"));
    }

    #[test]
    fn test_purge_css_removes_unused() {
        let css = ".used { color: red; } .unused { color: blue; }";
        let html = r#"<div class="used">Hello</div>"#;
        let used = extract_used_selectors(html);
        let result = purge_css(css, &used, &[]);

        assert!(result.contains(".used"));
        assert!(!result.contains(".unused"));
    }

    #[test]
    fn test_purge_css_keeps_root_html_body() {
        let css = ":root { --color: red; } html { font-size: 16px; } body { margin: 0; } .unused { color: blue; }";
        let html = r#"<div>Hello</div>"#;
        let used = extract_used_selectors(html);
        let result = purge_css(css, &used, &[]);

        assert!(result.contains(":root"));
        assert!(result.contains("html"));
        assert!(result.contains("body"));
        assert!(!result.contains(".unused"));
    }

    #[test]
    fn test_purge_css_handles_media_queries() {
        let css = "@media (min-width: 768px) { .used { color: red; } .unused { color: blue; } }";
        let html = r#"<div class="used">Hello</div>"#;
        let used = extract_used_selectors(html);
        let result = purge_css(css, &used, &[]);

        assert!(result.contains("@media"));
        assert!(result.contains(".used"));
        assert!(!result.contains(".unused"));
    }

    #[test]
    fn test_purge_css_keeps_keyframes() {
        let css = "@keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } } .unused { color: blue; }";
        let html = r#"<div>Hello</div>"#;
        let used = extract_used_selectors(html);
        let result = purge_css(css, &used, &[]);

        assert!(result.contains("@keyframes"));
        assert!(result.contains("fadeIn"));
        assert!(!result.contains(".unused"));
    }

    #[test]
    fn test_purge_css_safelist() {
        let css = ".dynamic-class { color: red; } .unused { color: blue; }";
        let html = r#"<div>Hello</div>"#;
        let used = extract_used_selectors(html);
        let safelist = compile_safelist(&["dynamic-.*".to_string()]);
        let result = purge_css(css, &used, &safelist);

        assert!(result.contains(".dynamic-class"));
        assert!(!result.contains(".unused"));
    }

    #[test]
    fn test_purge_css_compound_selectors() {
        // Note: With PurgeCSS-style approach, selectors with common HTML tags are kept
        // span.unused is kept because "span" is a common HTML tag
        let css = "div.container { color: red; } span.unused { color: blue; } .totally-unused { color: green; }";
        let html = r#"<div class="container">Hello</div>"#;
        let used = extract_used_selectors(html);
        let result = purge_css(css, &used, &[]);

        assert!(result.contains("div.container"));
        assert!(result.contains("span.unused")); // kept because span is common tag
        assert!(!result.contains(".totally-unused")); // removed because no match
    }

    #[test]
    fn test_purge_css_comma_selectors() {
        let css = ".used, .also-used, .unused { color: red; }";
        let html = r#"<div class="used also-used">Hello</div>"#;
        let used = extract_used_selectors(html);
        let result = purge_css(css, &used, &[]);

        assert!(result.contains(".used"));
        assert!(result.contains(".also-used"));
        assert!(!result.contains(".unused"));
    }

    #[test]
    fn test_extract_selectors_from_js() {
        let js = r#"
            element.classList.add('active');
            element.classList.remove('hidden');
            element.classList.toggle('visible');
            element.className = 'container main';
            element.className += 'extra';
            $(el).addClass('jquery-class');
            document.querySelector('.query-class');
            document.querySelectorAll('#query-id, .another-class');
        "#;
        let selectors = extract_used_selectors(js);

        // PurgeCSS-style: all words are extracted
        assert!(selectors.words.contains("active"));
        assert!(selectors.words.contains("hidden"));
        assert!(selectors.words.contains("visible"));
        assert!(selectors.words.contains("container"));
        assert!(selectors.words.contains("main"));
        assert!(selectors.words.contains("extra"));
        assert!(selectors.words.contains("jquery")); // split from jquery-class
        assert!(selectors.words.contains("class")); // split from jquery-class
        assert!(selectors.words.contains("query")); // split from query-class, query-id
        assert!(selectors.words.contains("another")); // split from another-class
    }

    #[test]
    fn test_purge_with_js_classes() {
        let css = ".active { color: green; } .inactive { color: gray; } .hidden { display: none; }";
        let content = r#"
            <div class="container">Hello</div>
            <script>element.classList.add('active'); element.classList.toggle('hidden');</script>
        "#;
        let used = extract_used_selectors(content);
        let result = purge_css(css, &used, &[]);

        assert!(result.contains(".active"));
        assert!(result.contains(".hidden"));
        assert!(!result.contains(".inactive"));
    }
}
