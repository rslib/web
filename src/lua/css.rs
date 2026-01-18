//! CSS processing module (rs.css)
//!
//! Provides:
//! - rs.css.concat(input, output, options?) - Concatenate CSS files with optional minification
//! - rs.css.bundle(input, output, options?) - Bundle CSS files with @import resolution (LightningCSS)
//! - rs.css.bundle_many(input, output_dir, options?) - Bundle multiple CSS entries to separate files

use crate::assets::minify_css;
use crate::lua::async_io::{AsyncIOTask, runtime};
use crate::tracker::SharedTracker;
use lightningcss::bundler::{Bundler as CssBundler, FileProvider};
use lightningcss::printer::PrinterOptions;
use lightningcss::stylesheet::{MinifyOptions, ParserOptions};
use lightningcss::targets::Targets;
use mlua::{Lua, Result, Table, Value};
use std::path::{Path, PathBuf};

/// Create the rs.css module table
pub fn create_module(lua: &Lua, project_root: &Path, tracker: SharedTracker) -> Result<Table> {
    let css_module = lua.create_table()?;
    let root = project_root.to_path_buf();

    // rs.css.concat(paths_or_pattern, output_path, options?) - async
    // Concatenates CSS files and optionally minifies
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let concat_fn = lua.create_function(
        move |_lua, (input, output_path, options): (Value, String, Option<Table>)| {
            let minify: bool = options
                .as_ref()
                .and_then(|t| t.get::<bool>("minify").ok())
                .unwrap_or(false);

            let files = collect_files(&input, &root_clone, "css")?;
            if files.is_empty() {
                return Err(mlua::Error::external("No CSS files found"));
            }

            let output = resolve_output_path(&output_path, &root_clone);
            let tracker = tracker_clone.clone();

            let handle = runtime().spawn(async move {
                let contents = read_files_async(&files, &tracker).await?;
                let buffer: String = contents.join("\n");

                let output_content = if minify {
                    tokio::task::spawn_blocking(move || minify_css(&buffer))
                        .await
                        .map_err(|e| e.to_string())??
                } else {
                    buffer
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

    Ok(css_module)
}

/// Options for CSS bundling
#[derive(Debug, Clone)]
struct CssBundleOptions {
    minify: bool,
    sourcemap: bool,
    nesting: bool,
}

impl Default for CssBundleOptions {
    fn default() -> Self {
        Self {
            minify: false,
            sourcemap: false,
            nesting: true,
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

        // Write output
        std::fs::write(&output, &combined_css)
            .map_err(|e| format!("Failed to write output: {}", e))?;

        // Track the write
        let canonical = output.canonicalize().unwrap_or_else(|_| output.clone());
        tracker.record_write(canonical, combined_css.as_bytes());

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

            // Determine output filename
            let filename = entry
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("output.css");
            let output_path = output_dir.join(filename);

            // Write output
            std::fs::write(&output_path, &result.code)
                .map_err(|e| format!("Failed to write output: {}", e))?;

            // Track the write
            let canonical = output_path.canonicalize().unwrap_or(output_path);
            tracker.record_write(canonical, result.code.as_bytes());
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
}
