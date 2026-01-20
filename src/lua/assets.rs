//! Asset hashing module (rs.assets)
//!
//! Provides content-based hashing for cache busting:
//! - rs.assets.hash(content) - Compute hash of content (async)
//! - rs.assets.write_hashed(content, path, options) - Write with hashed filename (async)
//! - rs.assets.get_path(original_path) - Get hashed path for original
//! - rs.assets.register(original_path, hashed_path) - Manually register a mapping
//!
//! Also provides a global manifest accessible via Tera filter.

use dashmap::DashMap;
use mlua::{Lua, Result, Table};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::lua::async_io::{AsyncIOTask, runtime};
use crate::tracker::SharedTracker;

/// Shared asset manifest mapping original paths to hashed paths
pub type AssetManifest = Arc<DashMap<String, String>>;

/// Create a new asset manifest
pub fn create_manifest() -> AssetManifest {
    Arc::new(DashMap::new())
}

/// Compute SHA256 hash of content and return first N characters
pub fn compute_hash(content: &[u8], length: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let result = hasher.finalize();
    hex::encode(result)[..length.min(64)].to_string()
}

/// Generate hashed filename from original path and content
/// e.g., "styles/main.css" + hash -> "styles/main.a1b2c3d4.css"
pub fn hashed_filename(original_path: &Path, hash: &str) -> PathBuf {
    let stem = original_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = original_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let new_filename = if ext.is_empty() {
        format!("{}.{}", stem, hash)
    } else {
        format!("{}.{}.{}", stem, hash, ext)
    };

    original_path.with_file_name(new_filename)
}

/// Write content to a hashed file and update the manifest (async)
pub async fn write_hashed_file(
    content: Vec<u8>,
    original_path: PathBuf,
    output_dir: PathBuf,
    manifest: AssetManifest,
    hash_length: usize,
) -> std::result::Result<(PathBuf, String), String> {
    // Compute hash in blocking task (CPU-bound)
    let content_clone = content.clone();
    let hash = tokio::task::spawn_blocking(move || compute_hash(&content_clone, hash_length))
        .await
        .map_err(|e| format!("Hash computation failed: {}", e))?;

    let hashed_path = hashed_filename(&original_path, &hash);
    let full_path = output_dir.join(&hashed_path);

    // Ensure parent directory exists
    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // Write file
    tokio::fs::write(&full_path, &content)
        .await
        .map_err(|e| format!("Failed to write file: {}", e))?;

    // Update manifest with web path (leading /)
    let original_web_path = format!("/{}", original_path.to_string_lossy());
    let hashed_web_path = format!("/{}", hashed_path.to_string_lossy());
    manifest.insert(original_web_path, hashed_web_path.clone());

    Ok((full_path, hashed_web_path))
}

/// Create the assets Lua module
pub fn create_module(
    lua: &Lua,
    root: &Path,
    manifest: AssetManifest,
    tracker: SharedTracker,
) -> Result<Table> {
    let module = lua.create_table()?;
    let root = root.to_path_buf();

    // rs.assets.hash(content, length?) -> AsyncIOTask<string>
    let hash_fn = lua.create_function(|_, (content, length): (mlua::String, Option<usize>)| {
        let content = content.as_bytes().to_vec();
        let len = length.unwrap_or(8);

        let handle = runtime().spawn(async move {
            let hash = tokio::task::spawn_blocking(move || compute_hash(&content, len))
                .await
                .map_err(|e| format!("Hash computation failed: {}", e))?;
            Ok(hash)
        });

        Ok(AsyncIOTask::from_string_handle(handle))
    })?;
    module.set("hash", hash_fn)?;

    // rs.assets.hash_sync(content, length?) -> string (blocking)
    let hash_sync_fn =
        lua.create_function(|_, (content, length): (mlua::String, Option<usize>)| {
            let bytes = content.as_bytes().to_vec();
            let hash = compute_hash(&bytes, length.unwrap_or(8));
            Ok(hash)
        })?;
    module.set("hash_sync", hash_sync_fn)?;

    // rs.assets.write_hashed(content, path, options?) -> AsyncIOTask<{path, hash_path}>
    let manifest_clone = manifest.clone();
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let write_hashed_fn = lua.create_function(
        move |_, (content, path, options): (mlua::String, String, Option<Table>)| {
            let content = content.as_bytes().to_vec();
            let original_path = PathBuf::from(&path);
            let output_dir = root_clone.clone();
            let manifest = manifest_clone.clone();
            let tracker = tracker_clone.clone();
            let hash_length = options
                .as_ref()
                .and_then(|o| o.get::<i64>("hash_length").ok())
                .unwrap_or(8) as usize;

            let content_for_tracking = content.clone();
            let handle = runtime().spawn(async move {
                let (full_path, hashed_web_path) =
                    write_hashed_file(content, original_path, output_dir, manifest, hash_length)
                        .await?;

                // Track the written file
                tracker.record_write(full_path, &content_for_tracking);
                tracker.merge_thread_locals();

                // Return the hashed web path
                Ok(hashed_web_path)
            });

            Ok(AsyncIOTask::from_string_handle(handle))
        },
    )?;
    module.set("write_hashed", write_hashed_fn)?;

    // rs.assets.register(original_path, hashed_path) - manually register a mapping
    let manifest_clone = manifest.clone();
    let register_fn = lua.create_function(move |_, (original, hashed): (String, String)| {
        let normalized_original = if original.starts_with('/') {
            original.clone()
        } else {
            format!("/{}", original)
        };
        let normalized_hashed = if hashed.starts_with('/') {
            hashed.clone()
        } else {
            format!("/{}", hashed)
        };
        manifest_clone.insert(normalized_original, normalized_hashed);
        Ok(())
    })?;
    module.set("register", register_fn)?;

    // rs.assets.get_path(original_path) -> hashed path or original if not found
    let manifest_clone = manifest.clone();
    let get_path_fn = lua.create_function(move |_, path: String| {
        let normalized = if path.starts_with('/') {
            path.clone()
        } else {
            format!("/{}", path)
        };
        Ok(manifest_clone
            .get(&normalized)
            .map(|v| v.clone())
            .unwrap_or(path))
    })?;
    module.set("get_path", get_path_fn)?;

    // rs.assets.manifest() -> table of all mappings
    let manifest_clone = manifest.clone();
    let manifest_fn = lua.create_function(move |lua, ()| {
        let table = lua.create_table()?;
        for entry in manifest_clone.iter() {
            table.set(entry.key().clone(), entry.value().clone())?;
        }
        Ok(table)
    })?;
    module.set("manifest", manifest_fn)?;

    // rs.assets.clear() -> clear the manifest
    let manifest_clone = manifest.clone();
    let clear_fn = lua.create_function(move |_, ()| {
        manifest_clone.clear();
        Ok(())
    })?;
    module.set("clear", clear_fn)?;

    // rs.assets.check_unused(output_dir) -> list of unused assets
    let root_clone = root.clone();
    let check_unused_fn = lua.create_function(move |lua, output_dir: String| {
        use regex::Regex;
        use std::collections::HashSet;

        let output_path = if Path::new(&output_dir).is_absolute() {
            PathBuf::from(&output_dir)
        } else {
            root_clone.join(&output_dir)
        };

        let mut asset_files: HashSet<String> = HashSet::new();
        for subdir in &["static", "fonts"] {
            let dir = output_path.join(subdir);
            if dir.exists() {
                collect_files_recursive(&dir, subdir, &mut asset_files);
            }
        }

        let mut referenced: HashSet<String> = HashSet::new();
        let patterns = [
            r#"src=["']([^"']+)["']"#,
            r#"href=["']([^"']+)["']"#,
            r#"url\(["']?([^"')]+)["']?\)"#,
            r#"srcset=["']([^"']+)["']"#,
        ];
        let regexes: Vec<Regex> = patterns.iter().filter_map(|p| Regex::new(p).ok()).collect();

        scan_files_for_refs(&output_path, "html", &regexes, &mut referenced);
        scan_files_for_refs(&output_path, "css", &regexes, &mut referenced);

        let mut unused: Vec<String> = asset_files
            .iter()
            .filter(|asset| !is_asset_referenced(asset, &referenced))
            .cloned()
            .collect();
        unused.sort();

        let result = lua.create_table()?;
        for (i, path) in unused.iter().enumerate() {
            result.set(i + 1, path.clone())?;
        }
        Ok(mlua::Value::Table(result))
    })?;
    module.set("check_unused", check_unused_fn)?;

    Ok(module)
}

fn collect_files_recursive(
    dir: &Path,
    prefix: &str,
    files: &mut std::collections::HashSet<String>,
) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && !name.starts_with('.')
            {
                files.insert(format!("/{}/{}", prefix, name));
            } else if path.is_dir()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && !name.starts_with('.')
            {
                let new_prefix = format!("{}/{}", prefix, name);
                collect_files_recursive(&path, &new_prefix, files);
            }
        }
    }
}

fn scan_files_for_refs(
    dir: &Path,
    ext: &str,
    regexes: &[regex::Regex],
    referenced: &mut std::collections::HashSet<String>,
) {
    let pattern = format!("{}/**/*.{}", dir.display(), ext);
    if let Ok(paths) = glob::glob(&pattern) {
        for path in paths.flatten() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                for regex in regexes {
                    for cap in regex.captures_iter(&content) {
                        if let Some(m) = cap.get(1) {
                            let reference = m.as_str();
                            if reference.contains(',') {
                                for part in reference.split(',') {
                                    let url = part.split_whitespace().next().unwrap_or("");
                                    if !url.is_empty() {
                                        referenced.insert(normalize_ref(url));
                                    }
                                }
                            } else {
                                referenced.insert(normalize_ref(reference));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn normalize_ref(reference: &str) -> String {
    if reference.starts_with("http://")
        || reference.starts_with("https://")
        || reference.starts_with("//")
    {
        return reference.to_string();
    }
    let path = if reference.starts_with('/') {
        reference.to_string()
    } else if let Some(stripped) = reference.strip_prefix("./") {
        format!("/{}", stripped)
    } else {
        format!("/{}", reference)
    };
    path.split('?')
        .next()
        .unwrap_or(&path)
        .split('#')
        .next()
        .unwrap_or(&path)
        .to_string()
}

fn is_asset_referenced(asset: &str, referenced: &std::collections::HashSet<String>) -> bool {
    if referenced.contains(asset) {
        return true;
    }
    let without_slash = asset.trim_start_matches('/');
    referenced.iter().any(|r| {
        r == without_slash || r.ends_with(asset) || asset.ends_with(r.trim_start_matches('/'))
    })
}
