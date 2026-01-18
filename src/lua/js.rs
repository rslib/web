//! JavaScript processing module (rs.js)
//!
//! Provides:
//! - rs.js.concat(input, output, options?) - Concatenate JS files with optional minification
//! - rs.js.bundle(input, output, options?) - Bundle JS files with Rolldown (ESM, tree-shaking)
//! - rs.js.bundle_many(input, output_dir, options?) - Bundle multiple entries to separate files

use crate::lua::async_io::{AsyncIOTask, runtime};
use crate::tracker::SharedTracker;
use brk_rolldown::{
    Bundler, BundlerOptions, InputItem, OutputFormat, RawMinifyOptions, ResolveOptions,
    SourceMapType,
};
use brk_rolldown_plugin::{
    __inner::SharedPluginable, HookLoadArgs, HookLoadOutput, HookResolveIdArgs,
    HookResolveIdOutput, HookUsage, Plugin, PluginContext,
};
use dashmap::DashMap;
use mlua::{Lua, Result, Table, Value};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// File Tracking Plugin for Rolldown
/// Tracks all local files loaded during bundling for hot reload
#[derive(Debug)]
struct FileTrackingPlugin {
    tracker: SharedTracker,
}

impl FileTrackingPlugin {
    fn new(tracker: SharedTracker) -> Self {
        Self { tracker }
    }

    fn is_local_file(id: &str) -> bool {
        !id.starts_with("https://") && !id.starts_with("http://") && !id.starts_with("data:")
    }
}

impl Plugin for FileTrackingPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("file-tracking")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Load
    }

    async fn load(
        &self,
        _ctx: &PluginContext,
        args: &HookLoadArgs<'_>,
    ) -> anyhow::Result<Option<HookLoadOutput>> {
        let id = args.id;

        // Only track local files, not URLs
        if !Self::is_local_file(id) {
            return Ok(None);
        }

        // Try to read and track the file
        let path = PathBuf::from(id);
        if path.exists()
            && path.is_file()
            && let Ok(content) = tokio::fs::read(&path).await
        {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            // Use record_read_async for proper tracking in async context
            self.tracker.record_read_async(canonical, &content);
        }

        // Return None to let the default loader handle it
        Ok(None)
    }
}

/// CDN Resolver Plugin for Rolldown
/// Resolves and fetches modules from CDN URLs (https://esm.sh, unpkg, etc.)
#[derive(Debug)]
struct CdnResolverPlugin {
    /// Cache for fetched CDN modules (URL -> code)
    cache: Arc<DashMap<String, String>>,
    /// HTTP client for fetching
    client: reqwest::Client,
}

impl CdnResolverPlugin {
    fn new() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
            client: reqwest::Client::new(),
        }
    }

    fn is_url(specifier: &str) -> bool {
        specifier.starts_with("https://") || specifier.starts_with("http://")
    }

    /// Extract base URL (scheme + host) from a URL
    fn get_base_url(url: &str) -> Option<&str> {
        // Find the scheme (https:// or http://)
        let (after_scheme, scheme_len) = if let Some(stripped) = url.strip_prefix("https://") {
            (stripped, 8)
        } else if let Some(stripped) = url.strip_prefix("http://") {
            (stripped, 7)
        } else {
            return None;
        };

        // Find the end of the host (first / after scheme)
        if let Some(slash_pos) = after_scheme.find('/') {
            Some(&url[..scheme_len + slash_pos])
        } else {
            Some(url)
        }
    }

    /// Normalize esm.sh URLs to canonical form
    /// esm.sh uses encoded paths like /X-ZEBjb2RlbWlycm9y.../es2022/pkg.mjs
    /// We normalize these to consistent URLs to avoid duplicate module loading
    fn normalize_esm_url(url: &str) -> String {
        // Check if this is an esm.sh URL with encoded deps (X- prefix in path)
        if !url.contains("esm.sh/") {
            return url.to_string();
        }

        // Pattern: https://esm.sh/@scope/pkg@version/X-encoded/es2022/file.mjs
        // We want to extract the package info and create a canonical URL

        // Split URL into parts
        let url_without_query = url.split('?').next().unwrap_or(url);

        // Check for encoded path segment (starts with X- after version)
        if let Some(x_pos) = url_without_query.find("/X-") {
            // Find where the package path starts
            let before_x = &url_without_query[..x_pos];
            // Find the file part after X-encoded/es2022/
            if let Some(file_start) = url_without_query[x_pos..].find("/es2022/") {
                let file_part = &url_without_query[x_pos + file_start + 8..]; // skip "/es2022/"
                // Reconstruct canonical URL: base + /es2022/ + file
                return format!("{}/es2022/{}", before_x, file_part);
            }
        }

        url.to_string()
    }

    /// Resolve relative imports from CDN modules
    fn resolve_relative_url(importer: &str, specifier: &str) -> Option<String> {
        if !Self::is_url(importer) {
            return None;
        }

        // Handle absolute paths from CDN (e.g., /@codemirror/... from esm.sh)
        // These should be resolved relative to the CDN base URL
        if specifier.starts_with('/')
            && !specifier.starts_with("//")
            && let Some(base) = Self::get_base_url(importer)
        {
            let resolved = format!("{}{}", base, specifier);
            return Some(Self::normalize_esm_url(&resolved));
        }

        if specifier.starts_with("./") || specifier.starts_with("../") {
            // Find base URL
            if let Some(last_slash) = importer.rfind('/') {
                let base = &importer[..last_slash];
                // Simple path resolution
                let resolved = if let Some(stripped) = specifier.strip_prefix("./") {
                    format!("{}/{}", base, stripped)
                } else {
                    // Handle ../
                    let mut parts: Vec<&str> = base.split('/').collect();
                    let mut spec_parts: Vec<&str> = specifier.split('/').collect();

                    for part in &spec_parts {
                        if *part == ".." {
                            parts.pop();
                        } else if *part != "." {
                            break;
                        }
                    }
                    spec_parts.retain(|p| *p != ".." && *p != ".");
                    format!("{}/{}", parts.join("/"), spec_parts.join("/"))
                };
                return Some(Self::normalize_esm_url(&resolved));
            }
        }
        None
    }

    /// Check if a specifier is a bare specifier (npm package name)
    fn is_bare_specifier(specifier: &str) -> bool {
        // Bare specifiers don't start with ., /, or a URL scheme
        !specifier.starts_with('.') && !specifier.starts_with('/') && !specifier.contains("://")
    }

    /// Resolve a bare specifier to a CDN URL based on the importer's CDN
    fn resolve_bare_to_cdn(importer: &str, specifier: &str) -> Option<String> {
        // Extract the CDN base from the importer
        // For esm.sh: https://esm.sh/*@codemirror/view@6.26.0 -> https://esm.sh
        if let Some(base) = Self::get_base_url(importer) {
            // Use bundle mode (*) to get self-contained bundles
            // This helps avoid version conflicts
            return Some(format!("{}/*{}", base, specifier));
        }
        None
    }
}

impl Plugin for CdnResolverPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("cdn-resolver")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId | HookUsage::Load
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> anyhow::Result<Option<HookResolveIdOutput>> {
        let specifier = args.specifier;

        // If it's a URL, mark it as resolved
        if Self::is_url(specifier) {
            return Ok(Some(HookResolveIdOutput::from_id(specifier)));
        }

        // Handle imports from CDN modules
        if let Some(importer) = args.importer {
            // Handle relative imports
            if let Some(resolved) = Self::resolve_relative_url(importer, specifier) {
                return Ok(Some(HookResolveIdOutput::from_id(resolved)));
            }

            // Handle bare specifiers from CDN modules (e.g., @codemirror/view from esm.sh)
            // Convert them to CDN URLs using the same CDN as the importer
            if Self::is_url(importer)
                && Self::is_bare_specifier(specifier)
                && let Some(cdn_url) = Self::resolve_bare_to_cdn(importer, specifier)
            {
                return Ok(Some(HookResolveIdOutput::from_id(cdn_url)));
            }
        }

        // Let other resolvers handle it
        Ok(None)
    }

    async fn load(
        &self,
        _ctx: &PluginContext,
        args: &HookLoadArgs<'_>,
    ) -> anyhow::Result<Option<HookLoadOutput>> {
        let id = args.id;

        // Only handle URLs
        if !Self::is_url(id) {
            return Ok(None);
        }

        // Check cache first
        if let Some(cached) = self.cache.get(id) {
            return Ok(Some(HookLoadOutput {
                code: cached.value().clone().into(),
                map: None,
                side_effects: None,
                module_type: None,
            }));
        }

        // Fetch from CDN
        let response = self
            .client
            .get(id)
            .header("User-Agent", "rs-web/0.3.0")
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch {}: {}", id, e))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch {}: HTTP {}",
                id,
                response.status()
            ));
        }

        let code = response
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read response from {}: {}", id, e))?;

        // Cache the result
        self.cache.insert(id.to_string(), code.clone());

        log::info!("Fetched CDN module: {}", id);

        Ok(Some(HookLoadOutput {
            code: code.into(),
            map: None,
            side_effects: None,
            module_type: None,
        }))
    }
}

/// Create the rs.js module table
pub fn create_module(lua: &Lua, project_root: &Path, tracker: SharedTracker) -> Result<Table> {
    let js_module = lua.create_table()?;
    let root = project_root.to_path_buf();

    // rs.js.concat(paths_or_pattern, output_path, options?) - async
    // Concatenates JS files and optionally minifies
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let concat_fn = lua.create_function(
        move |_lua, (input, output_path, options): (Value, String, Option<Table>)| {
            let minify: bool = options
                .as_ref()
                .and_then(|t| t.get::<bool>("minify").ok())
                .unwrap_or(false);

            let files = collect_files(&input, &root_clone, "js")?;
            if files.is_empty() {
                return Err(mlua::Error::external("No JS files found"));
            }

            let output = resolve_output_path(&output_path, &root_clone);
            let tracker = tracker_clone.clone();

            let handle = runtime().spawn(async move {
                let contents = read_files_async(&files, &tracker).await?;
                let buffer: String = contents.join("\n");

                let output_content = if minify {
                    tokio::task::spawn_blocking(move || minify_js(&buffer))
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
    js_module.set("concat", concat_fn)?;

    // rs.js.bundle(paths_or_pattern, output_path, options?) - async
    // Bundles JS files using Rolldown with tree-shaking, minification
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let bundle_fn = lua.create_function(
        move |lua, (input, output_path, options): (Value, String, Option<Table>)| {
            let opts = JsBundleOptions::from_lua(&options, lua)?;
            let entries = collect_entries(&input, &root_clone)?;

            if entries.is_empty() {
                return Err(mlua::Error::external("No JS entry files found"));
            }

            let output = resolve_output_path(&output_path, &root_clone);
            let cwd = root_clone.clone();
            let tracker = tracker_clone.clone();

            let handle = runtime().spawn(async move {
                bundle_with_rolldown(&entries, &output, &cwd, &opts, &tracker).await
            });

            Ok(AsyncIOTask::new(handle))
        },
    )?;
    js_module.set("bundle", bundle_fn)?;

    // rs.js.bundle_many(paths_or_pattern, output_dir, options?) - async
    // Bundles multiple entries to separate output files
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let bundle_many_fn = lua.create_function(
        move |lua, (input, output_dir, options): (Value, String, Option<Table>)| {
            let opts = JsBundleOptions::from_lua(&options, lua)?;
            let entries = collect_entries(&input, &root_clone)?;

            if entries.is_empty() {
                return Err(mlua::Error::external("No JS entry files found"));
            }

            let output = resolve_output_path(&output_dir, &root_clone);
            let cwd = root_clone.clone();
            let tracker = tracker_clone.clone();

            let handle = runtime().spawn(async move {
                bundle_many_with_rolldown(&entries, &output, &cwd, &opts, &tracker).await
            });

            Ok(AsyncIOTask::new(handle))
        },
    )?;
    js_module.set("bundle_many", bundle_many_fn)?;

    Ok(js_module)
}

/// Options for JS bundling
#[derive(Debug, Clone)]
struct JsBundleOptions {
    minify: bool,
    treeshake: bool,
    sourcemap: bool,
    format: String,
    external: Vec<String>,
    splitting: bool,
}

impl Default for JsBundleOptions {
    fn default() -> Self {
        Self {
            minify: false,
            treeshake: true,
            sourcemap: false,
            format: "esm".to_string(),
            external: Vec::new(),
            splitting: false,
        }
    }
}

impl JsBundleOptions {
    fn from_lua(options: &Option<Table>, _lua: &Lua) -> mlua::Result<Self> {
        let mut opts = Self::default();

        if let Some(t) = options {
            if let Ok(minify) = t.get::<bool>("minify") {
                opts.minify = minify;
            }
            if let Ok(treeshake) = t.get::<bool>("treeshake") {
                opts.treeshake = treeshake;
            }
            if let Ok(sourcemap) = t.get::<bool>("sourcemap") {
                opts.sourcemap = sourcemap;
            }
            if let Ok(format) = t.get::<String>("format") {
                opts.format = format;
            }
            if let Ok(splitting) = t.get::<bool>("splitting") {
                opts.splitting = splitting;
            }
            if let Ok(external) = t.get::<Table>("external") {
                for (_, ext) in external.pairs::<i64, String>().flatten() {
                    opts.external.push(ext);
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
                        "JS entry file not found: {}",
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
            "js.bundle: first argument must be path, glob pattern, or array of paths",
        )),
    }
}

/// Bundle files with Rolldown (single output)
async fn bundle_with_rolldown(
    entries: &[PathBuf],
    output: &Path,
    cwd: &Path,
    opts: &JsBundleOptions,
    tracker: &SharedTracker,
) -> std::result::Result<(), String> {
    // Create output directory
    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    // Build input items
    let input_items: Vec<InputItem> = entries
        .iter()
        .map(|p| InputItem {
            name: Some(
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("entry")
                    .to_string(),
            ),
            import: p.to_string_lossy().to_string(),
        })
        .collect();

    // Determine output format
    let format = match opts.format.as_str() {
        "cjs" => OutputFormat::Cjs,
        "iife" => OutputFormat::Iife,
        "umd" => OutputFormat::Umd,
        _ => OutputFormat::Esm,
    };

    // Configure bundler options with TypeScript support
    let bundler_options = BundlerOptions {
        input: Some(input_items),
        cwd: Some(cwd.to_path_buf()),
        dir: output.parent().map(|p| p.to_string_lossy().to_string()),
        format: Some(format),
        minify: if opts.minify {
            Some(RawMinifyOptions::Bool(true))
        } else {
            None
        },
        sourcemap: if opts.sourcemap {
            Some(SourceMapType::File)
        } else {
            Some(SourceMapType::Hidden)
        },
        treeshake: brk_rolldown::TreeshakeOptions::Boolean(opts.treeshake),
        // Enable TypeScript/TSX/JSX resolution
        resolve: Some(ResolveOptions {
            extensions: Some(vec![
                ".ts".into(),
                ".tsx".into(),
                ".mts".into(),
                ".cts".into(),
                ".js".into(),
                ".jsx".into(),
                ".mjs".into(),
                ".cjs".into(),
                ".json".into(),
            ]),
            ..Default::default()
        }),
        ..Default::default()
    };

    // Create plugins: file tracking for hot reload + CDN resolver
    let tracking_plugin: SharedPluginable = Arc::new(FileTrackingPlugin::new(tracker.clone()));
    let cdn_plugin: SharedPluginable = Arc::new(CdnResolverPlugin::new());

    // Create and run bundler with plugins
    let mut bundler = Bundler::with_plugins(bundler_options, vec![tracking_plugin, cdn_plugin])
        .map_err(|e| format!("Failed to create bundler: {:?}", e))?;

    let result = bundler
        .generate()
        .await
        .map_err(|e| format!("Bundling failed: {:?}", e))?;

    // Write output (merge all chunks into single file for bundle())
    // Filter out sourcemap files (.map) - only include JS code
    let mut combined_code = String::new();
    for asset in &result.assets {
        let filename = asset.filename();
        // Skip sourcemap files
        if filename.ends_with(".map") {
            continue;
        }
        let code = std::str::from_utf8(asset.content_as_bytes())
            .map_err(|e| format!("Invalid UTF-8 in bundle output: {}", e))?;
        combined_code.push_str(code);
        combined_code.push('\n');
    }

    tokio::fs::write(output, &combined_code)
        .await
        .map_err(|e| format!("Failed to write output: {}", e))?;

    // Track the write
    let canonical = output
        .canonicalize()
        .unwrap_or_else(|_| output.to_path_buf());
    tracker.record_write(canonical, combined_code.as_bytes());

    // Merge thread-local tracking data from this tokio thread
    // (record_read/record_write use thread-local storage that needs to be merged)
    tracker.merge_thread_locals();

    Ok(())
}

/// Bundle files with Rolldown (multiple outputs)
async fn bundle_many_with_rolldown(
    entries: &[PathBuf],
    output_dir: &Path,
    cwd: &Path,
    opts: &JsBundleOptions,
    tracker: &SharedTracker,
) -> std::result::Result<(), String> {
    // Create output directory
    tokio::fs::create_dir_all(output_dir)
        .await
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // Build input items
    let input_items: Vec<InputItem> = entries
        .iter()
        .map(|p| InputItem {
            name: Some(
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("entry")
                    .to_string(),
            ),
            import: p.to_string_lossy().to_string(),
        })
        .collect();

    // Determine output format
    let format = match opts.format.as_str() {
        "cjs" => OutputFormat::Cjs,
        "iife" => OutputFormat::Iife,
        "umd" => OutputFormat::Umd,
        _ => OutputFormat::Esm,
    };

    // Configure bundler options with TypeScript support
    let bundler_options = BundlerOptions {
        input: Some(input_items),
        cwd: Some(cwd.to_path_buf()),
        dir: Some(output_dir.to_string_lossy().to_string()),
        format: Some(format),
        minify: if opts.minify {
            Some(RawMinifyOptions::Bool(true))
        } else {
            None
        },
        sourcemap: if opts.sourcemap {
            Some(SourceMapType::File)
        } else {
            Some(SourceMapType::Hidden)
        },
        treeshake: brk_rolldown::TreeshakeOptions::Boolean(opts.treeshake),
        // Enable TypeScript/TSX/JSX resolution
        resolve: Some(ResolveOptions {
            extensions: Some(vec![
                ".ts".into(),
                ".tsx".into(),
                ".mts".into(),
                ".cts".into(),
                ".js".into(),
                ".jsx".into(),
                ".mjs".into(),
                ".cjs".into(),
                ".json".into(),
            ]),
            ..Default::default()
        }),
        ..Default::default()
    };

    // Create plugins: file tracking for hot reload + CDN resolver
    let tracking_plugin: SharedPluginable = Arc::new(FileTrackingPlugin::new(tracker.clone()));
    let cdn_plugin: SharedPluginable = Arc::new(CdnResolverPlugin::new());

    // Create and run bundler with plugins
    let mut bundler = Bundler::with_plugins(bundler_options, vec![tracking_plugin, cdn_plugin])
        .map_err(|e| format!("Failed to create bundler: {:?}", e))?;

    let result = bundler
        .write()
        .await
        .map_err(|e| format!("Bundling failed: {:?}", e))?;

    // Track written files
    for asset in &result.assets {
        let file_path = output_dir.join(asset.filename());
        let canonical = file_path.canonicalize().unwrap_or(file_path);
        tracker.record_write(canonical, asset.content_as_bytes());
    }

    // Merge thread-local tracking data from this tokio thread
    tracker.merge_thread_locals();

    Ok(())
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
            "js.concat: first argument must be glob pattern or array of paths".to_string(),
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

/// Minify JavaScript using OXC (with dead code elimination)
fn minify_js(source: &str) -> std::result::Result<String, String> {
    use oxc_allocator::Allocator;
    use oxc_codegen::{Codegen, CodegenOptions};
    use oxc_minifier::{CompressOptions, MangleOptions, Minifier, MinifierOptions};
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    let allocator = Allocator::default();
    let source_type = SourceType::mjs();
    let ret = Parser::new(&allocator, source, source_type).parse();

    if !ret.errors.is_empty() {
        return Err(ret
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; "));
    }

    let mut program = ret.program;
    let options = MinifierOptions {
        mangle: Some(MangleOptions::default()),
        compress: Some(CompressOptions::default()),
    };

    Minifier::new(options).minify(&allocator, &mut program);

    let code = Codegen::new()
        .with_options(CodegenOptions::minify())
        .build(&program)
        .code;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minify_js_basic() {
        let input = r#"
            function hello(name) {
                console.log("Hello, " + name);
            }
        "#;
        let result = minify_js(input).unwrap();
        assert!(!result.contains('\n'));
        assert!(result.len() < input.len());
    }

    #[test]
    fn test_minify_js_removes_whitespace() {
        let input = "const   x   =   1;";
        let result = minify_js(input).unwrap();
        assert!(!result.contains("   "));
    }

    #[test]
    fn test_minify_js_boolean_compression() {
        let input = "console.log(true, false);";
        let result = minify_js(input).unwrap();
        assert!(
            result.contains("!0") || result.contains("!1") || result.contains("true"),
            "Result: {}",
            result
        );
    }

    #[test]
    fn test_minify_js_dead_code_elimination() {
        let input = r#"
            function used() { return 1; }
            function unused() { return 2; }
            console.log(used());
        "#;
        let result = minify_js(input).unwrap();
        assert!(
            !result.contains("unused"),
            "Result should not contain 'unused': {}",
            result
        );
    }

    #[test]
    fn test_minify_js_parse_error() {
        let input = "function { invalid syntax";
        let result = minify_js(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_minify_js_preserves_strings() {
        let input = r#"console.log("Hello World");"#;
        let result = minify_js(input).unwrap();
        assert!(
            result.contains("Hello World"),
            "Expected 'Hello World' in result: {}",
            result
        );
    }

    #[test]
    fn test_minify_js_template_literals() {
        let input = "console.log(`hello world`);";
        let result = minify_js(input).unwrap();
        assert!(
            result.contains("hello") && result.contains("world"),
            "Result: {}",
            result
        );
    }
}
