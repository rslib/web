//! Asset processing functions for Lua API
//!
//! Functions: build_css, download_google_font

use crate::tracker::SharedTracker;
use mlua::{Lua, Result, Table, Value};
use regex::Regex;
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

    // build_css(paths_or_pattern, output_path, options?) - Build and concatenate CSS files
    // paths_or_pattern: glob pattern like "styles/*.css" OR array of paths {"a.css", "b.css"}
    // options: { minify?: boolean }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let build_css_fn = lua.create_function(
        move |_lua, (input, output_path, options): (Value, String, Option<Table>)| {
            let minify: bool = options
                .as_ref()
                .and_then(|t| t.get::<bool>("minify").ok())
                .unwrap_or(false);

            // Collect CSS files based on input type
            let css_files: Vec<PathBuf> = match input {
                // Array of paths
                Value::Table(table) => {
                    let mut files = Vec::new();
                    for pair in table.pairs::<i64, String>() {
                        let (_, path_str) = pair.map_err(|e| {
                            mlua::Error::external(format!("Invalid path in array: {}", e))
                        })?;
                        let path = if Path::new(&path_str).is_absolute() {
                            PathBuf::from(&path_str)
                        } else {
                            root_clone.join(&path_str)
                        };
                        if path.exists() && path.is_file() {
                            files.push(path);
                        } else {
                            return Err(mlua::Error::external(format!(
                                "CSS file not found: {}",
                                path_str
                            )));
                        }
                    }
                    files
                }
                // Glob pattern
                Value::String(pattern_str) => {
                    let pattern = pattern_str
                        .to_str()
                        .map_err(|e| {
                            mlua::Error::external(format!("Invalid pattern string: {}", e))
                        })?
                        .to_string();
                    let glob_pattern = if Path::new(&pattern).is_absolute() {
                        pattern.clone()
                    } else {
                        root_clone.join(&pattern).to_string_lossy().to_string()
                    };

                    let mut files: Vec<PathBuf> = glob::glob(&glob_pattern)
                        .map_err(|e| mlua::Error::external(format!("Invalid glob pattern: {}", e)))?
                        .filter_map(|entry| entry.ok())
                        .filter(|path| path.is_file())
                        .collect();

                    // Sort alphabetically for consistent ordering (only for glob)
                    files.sort();
                    files
                }
                _ => {
                    return Err(mlua::Error::external(
                        "build_css: first argument must be a glob pattern string or array of paths",
                    ));
                }
            };

            if css_files.is_empty() {
                return Err(mlua::Error::external("No CSS files found"));
            }

            // Read and concatenate files, tracking each read
            let mut css_buffer = String::new();
            for path in &css_files {
                let content = fs::read_to_string(path).map_err(|e| {
                    mlua::Error::external(format!("Failed to read {:?}: {}", path, e))
                })?;

                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                tracker_clone.record_read(canonical, content.as_bytes());

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

            // Track the write (canonicalize for consistent path matching)
            let canonical = output.canonicalize().unwrap_or(output);
            tracker_clone.record_write(canonical, output_content.as_bytes());

            Ok(Value::Boolean(true))
        },
    )?;
    module.set("build_css", build_css_fn)?;

    // download_google_font(family, options) - Download Google Font files
    // options: { fonts_dir, css_path, css_prefix?, weights?, display? }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let download_google_font_fn =
        lua.create_function(move |_lua, (family, options): (String, Table)| {
            // Required: fonts_dir and css_path
            let fonts_dir_str: String = options.get("fonts_dir").map_err(|_| {
                mlua::Error::external("download_google_font: 'fonts_dir' is required")
            })?;
            let css_path_str: String = options.get("css_path").map_err(|_| {
                mlua::Error::external("download_google_font: 'css_path' is required")
            })?;

            // Optional: css_prefix (URL prefix for fonts in CSS), defaults to "/fonts"
            let css_prefix: String = options
                .get::<String>("css_prefix")
                .unwrap_or_else(|_| "/fonts".to_string());

            // Optional: weights
            let weights: Vec<u32> = options
                .get::<Table>("weights")
                .ok()
                .map(|table| {
                    table
                        .pairs::<i64, u32>()
                        .filter_map(|pair| pair.ok().map(|(_, v)| v))
                        .collect()
                })
                .unwrap_or_else(|| vec![400]);

            // Optional: display
            let display: String = options
                .get::<String>("display")
                .unwrap_or_else(|_| "swap".to_string());

            // Resolve paths
            let fonts_dir = if Path::new(&fonts_dir_str).is_absolute() {
                PathBuf::from(&fonts_dir_str)
            } else {
                root_clone.join(&fonts_dir_str)
            };

            let css_path = if Path::new(&css_path_str).is_absolute() {
                PathBuf::from(&css_path_str)
            } else {
                root_clone.join(&css_path_str)
            };

            // Build Google Fonts CSS URL
            let weight_str = weights
                .iter()
                .map(|w| w.to_string())
                .collect::<Vec<_>>()
                .join(";");
            let family_encoded = family.replace(' ', "+");
            let url = format!(
                "https://fonts.googleapis.com/css2?family={}:wght@{}&display={}",
                family_encoded, weight_str, display
            );

            // Fetch CSS (Chrome UA gets woff2 format)
            let css = fetch_google_fonts_css(&url)
                .map_err(|e| mlua::Error::external(format!("Failed to fetch font CSS: {}", e)))?;

            // Create fonts directory
            fs::create_dir_all(&fonts_dir).map_err(|e| {
                mlua::Error::external(format!("Failed to create fonts directory: {}", e))
            })?;

            // Parse CSS for font URLs and download each font
            let url_regex = Regex::new(r"url\(([^)]+)\)").unwrap();
            let mut local_css = css.clone();

            for cap in url_regex.captures_iter(&css) {
                let font_url = &cap[1];

                // Extract filename from URL (woff2 or ttf)
                let filename = extract_font_filename(font_url);
                if let Some(filename) = filename {
                    // Download the font file
                    match fetch_binary(font_url) {
                        Ok(font_data) => {
                            let font_path = fonts_dir.join(&filename);
                            fs::write(&font_path, &font_data).map_err(|e| {
                                mlua::Error::external(format!(
                                    "Failed to write font {}: {}",
                                    filename, e
                                ))
                            })?;

                            // Track the write
                            let canonical = font_path
                                .canonicalize()
                                .unwrap_or_else(|_| font_path.clone());
                            tracker_clone.record_write(canonical, &font_data);

                            // Update CSS to use local path with user-specified prefix
                            let css_prefix_clean = css_prefix.trim_end_matches('/');
                            local_css = local_css
                                .replace(font_url, &format!("{}/{}", css_prefix_clean, filename));

                            log::info!("Downloaded font: {}", filename);
                        }
                        Err(e) => {
                            log::warn!("Failed to download font {}: {}", font_url, e);
                        }
                    }
                }
            }

            // Create CSS parent directory if needed
            if let Some(parent) = css_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    mlua::Error::external(format!("Failed to create CSS directory: {}", e))
                })?;
            }

            // Save the CSS
            fs::write(&css_path, &local_css)
                .map_err(|e| mlua::Error::external(format!("Failed to write CSS: {}", e)))?;

            // Track the CSS write
            let canonical = css_path.canonicalize().unwrap_or_else(|_| css_path.clone());
            tracker_clone.record_write(canonical, local_css.as_bytes());

            log::info!("Saved CSS for {} to {:?}", family, css_path);

            Ok(Value::Boolean(true))
        })?;
    module.set("download_google_font", download_google_font_fn)?;

    Ok(())
}

/// Fetch Google Fonts CSS with Chrome User-Agent
fn fetch_google_fonts_css(url: &str) -> std::result::Result<String, Box<dyn std::error::Error>> {
    use crate::lua::async_io::block_on;

    let client = reqwest::Client::new();
    let resp = block_on(
        client
            .get(url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .send(),
    )?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()).into());
    }

    let text = block_on(resp.text())?;
    Ok(text)
}

/// Fetch binary data from URL
fn fetch_binary(url: &str) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
    use crate::lua::async_io::block_on;

    let client = reqwest::Client::new();
    let resp = block_on(client.get(url).send())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()).into());
    }

    let bytes = block_on(resp.bytes())?;
    Ok(bytes.to_vec())
}

/// Extract filename from Google Fonts URL
fn extract_font_filename(url: &str) -> Option<String> {
    // Try woff2 first, then ttf
    if let Some(pos) = url.rfind('/') {
        let path = &url[pos + 1..];
        // Find .woff2 or .ttf extension
        if let Some(ext_pos) = path.find(".woff2") {
            return Some(path[..ext_pos + 6].to_string());
        }
        if let Some(ext_pos) = path.find(".ttf") {
            return Some(path[..ext_pos + 4].to_string());
        }
        if let Some(ext_pos) = path.find(".woff") {
            return Some(path[..ext_pos + 5].to_string());
        }
    }
    None
}
