//! Asset processing functions for Lua API
//!
//! Functions: build_css

use super::tracker::SharedTracker;
use mlua::{Lua, Result, Table, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// Register asset processing functions on the module table
pub fn register(
    lua: &Lua,
    module: &Table,
    project_root: &Path,
    tracker: SharedTracker,
) -> Result<()> {
    let root = project_root.to_path_buf();

    // build_css(pattern, output_path, options?) - Build and concatenate CSS files
    // pattern: glob pattern like "styles/*.css" or "**/*.css"
    // options: { minify?: boolean }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let build_css_fn = lua.create_function(
        move |_lua, (pattern, output_path, options): (String, String, Option<Table>)| {
            let minify: bool = options
                .as_ref()
                .and_then(|t| t.get::<bool>("minify").ok())
                .unwrap_or(false);

            // Resolve the glob pattern relative to project root
            let glob_pattern = if Path::new(&pattern).is_absolute() {
                pattern.clone()
            } else {
                root_clone.join(&pattern).to_string_lossy().to_string()
            };

            // Collect matching CSS files
            let mut css_files: Vec<PathBuf> = glob::glob(&glob_pattern)
                .map_err(|e| mlua::Error::external(format!("Invalid glob pattern: {}", e)))?
                .filter_map(|entry| entry.ok())
                .filter(|path| path.is_file())
                .collect();

            if css_files.is_empty() {
                return Err(mlua::Error::external(format!(
                    "No CSS files found matching pattern: {}",
                    pattern
                )));
            }

            // Sort alphabetically for consistent ordering
            css_files.sort();

            // Read and concatenate files, tracking each read
            let mut css_buffer = String::new();
            for path in &css_files {
                let content = fs::read_to_string(path).map_err(|e| {
                    mlua::Error::external(format!("Failed to read {:?}: {}", path, e))
                })?;

                // Track each CSS file read
                tracker_clone.record_read(path.clone(), content.as_bytes());

                if !css_buffer.is_empty() {
                    css_buffer.push('\n');
                }
                css_buffer.push_str(&content);
            }

            // Minify if requested
            let output_content = if minify {
                minifier::css::minify(&css_buffer)
                    .map_err(|e| mlua::Error::external(format!("CSS minification failed: {}", e)))?
                    .to_string()
            } else {
                css_buffer
            };

            // Resolve output path
            let output = if Path::new(&output_path).is_absolute() {
                PathBuf::from(&output_path)
            } else {
                root_clone.join(&output_path)
            };

            // Ensure parent directory exists
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    mlua::Error::external(format!("Failed to create directory: {}", e))
                })?;
            }

            fs::write(&output, &output_content)
                .map_err(|e| mlua::Error::external(format!("Failed to write CSS: {}", e)))?;

            // Track the write
            tracker_clone.record_write(output, output_content.as_bytes());

            Ok(Value::Boolean(true))
        },
    )?;
    module.set("build_css", build_css_fn)?;

    Ok(())
}
