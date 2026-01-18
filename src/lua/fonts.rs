//! Font processing module (rs.fonts)
//!
//! Provides:
//! - rs.fonts.download_google_font(family, options) - Download Google Font files

use crate::assets::minify_css;
use crate::lua::async_io::{AsyncIOTask, CacheOption, runtime};
use crate::tracker::SharedTracker;
use mlua::{Lua, Result, Table, Value};
use regex::Regex;
use std::path::{Path, PathBuf};

/// Create the rs.fonts module table
pub fn create_module(lua: &Lua, project_root: &Path, tracker: SharedTracker) -> Result<Table> {
    let fonts_module = lua.create_table()?;
    let root = project_root.to_path_buf();

    // rs.fonts.download_google_font(family, options) - Download Google Font files (async)
    // options: { fonts_dir, css_path, css_prefix?, weights?, display?, cache?, minify? }
    // Returns: AsyncIOTask handle - call rs.async.await() to wait for completion
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

            // Optional: minify CSS output (default: true)
            let minify: bool = options
                .get::<Option<bool>>("minify")
                .ok()
                .flatten()
                .unwrap_or(true);

            // Optional: cache (bool or string path), defaults to true
            let cache_option = match options.get::<Value>("cache") {
                Ok(Value::Boolean(b)) => {
                    if b {
                        CacheOption::Auto
                    } else {
                        CacheOption::Disabled
                    }
                }
                Ok(Value::String(s)) => match s.to_str() {
                    Ok(borrowed) if !borrowed.is_empty() => {
                        let path_str = borrowed.to_string();
                        let path = if Path::new(&path_str).is_absolute() {
                            PathBuf::from(&path_str)
                        } else {
                            root_clone.join(&path_str)
                        };
                        CacheOption::Path(path)
                    }
                    _ => CacheOption::Auto,
                },
                _ => CacheOption::Auto,
            };

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
            let css_url = format!(
                "https://fonts.googleapis.com/css2?family={}:wght@{}&display={}",
                family_encoded, weight_str, display
            );

            let project_root = root_clone.clone();
            let tracker = tracker_clone.clone();
            let minify_opt = minify;

            // Spawn async task
            let handle = runtime().spawn(async move {
                // Fetch CSS with Chrome UA
                let css =
                    fetch_google_fonts_css_async(&css_url, &project_root, &cache_option).await?;

                // Create fonts directory
                tokio::fs::create_dir_all(&fonts_dir)
                    .await
                    .map_err(|e| format!("Failed to create fonts directory: {}", e))?;

                // Parse CSS for font URLs
                let url_regex = Regex::new(r"url\(([^)]+)\)").unwrap();
                let mut local_css = css.clone();

                // Collect all font URLs and filenames
                let font_entries: Vec<(String, String)> = url_regex
                    .captures_iter(&css)
                    .filter_map(|cap| {
                        let font_url = cap[1].to_string();
                        extract_font_filename(&font_url).map(|filename| (font_url, filename))
                    })
                    .collect();

                // Fetch all fonts concurrently
                let client = reqwest::Client::new();
                let font_futures: Vec<_> = font_entries
                    .iter()
                    .map(|(url, filename)| {
                        let client = client.clone();
                        let url = url.clone();
                        let filename = filename.clone();
                        let cache_path = get_cache_path(&url, &project_root, &cache_option);
                        async move {
                            // Check cache first
                            if let Some(ref path) = cache_path
                                && let Ok(cached) = tokio::fs::read(path).await
                            {
                                return (url, filename, Ok(cached));
                            }

                            // Fetch from network
                            let result = match client.get(&url).send().await {
                                Ok(resp) if resp.status().is_success() => {
                                    match resp.bytes().await {
                                        Ok(bytes) => {
                                            let data = bytes.to_vec();
                                            // Write to cache
                                            if let Some(ref path) = cache_path {
                                                if let Some(parent) = path.parent() {
                                                    let _ = tokio::fs::create_dir_all(parent).await;
                                                }
                                                let _ = tokio::fs::write(path, &data).await;
                                            }
                                            Ok(data)
                                        }
                                        Err(e) => Err(e.to_string()),
                                    }
                                }
                                Ok(resp) => Err(format!("HTTP {}", resp.status())),
                                Err(e) => Err(e.to_string()),
                            };
                            (url, filename, result)
                        }
                    })
                    .collect();

                let font_results = futures::future::join_all(font_futures).await;

                // Write fonts and update CSS
                let mut write_tasks: Vec<(PathBuf, Vec<u8>)> = Vec::new();
                for (font_url, filename, result) in font_results {
                    match result {
                        Ok(font_data) => {
                            let font_path = fonts_dir.join(&filename);
                            write_tasks.push((font_path, font_data));

                            // Update CSS to use local path
                            let css_prefix_clean = css_prefix.trim_end_matches('/');
                            local_css = local_css
                                .replace(&font_url, &format!("{}/{}", css_prefix_clean, filename));

                            log::info!("Downloaded font: {}", filename);
                        }
                        Err(e) => {
                            log::warn!("Failed to download font {}: {}", font_url, e);
                        }
                    }
                }

                // Write all font files concurrently
                let write_futures: Vec<_> = write_tasks
                    .iter()
                    .map(|(path, data)| tokio::fs::write(path, data))
                    .collect();
                let write_results = futures::future::join_all(write_futures).await;
                for (i, result) in write_results.into_iter().enumerate() {
                    if let Err(e) = result {
                        log::warn!("Failed to write font file: {}", e);
                    } else {
                        // Track the write
                        let (path, data) = &write_tasks[i];
                        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                        tracker.record_write(canonical, data);
                    }
                }

                // Create CSS parent directory if needed
                if let Some(parent) = css_path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| format!("Failed to create CSS directory: {}", e))?;
                }

                // Minify CSS if requested (default: true)
                let final_css = if minify_opt {
                    minify_css(&local_css).unwrap_or_else(|_| local_css.clone())
                } else {
                    local_css
                };

                // Write CSS file
                tokio::fs::write(&css_path, &final_css)
                    .await
                    .map_err(|e| format!("Failed to write CSS: {}", e))?;

                // Track the CSS write
                let canonical = css_path.canonicalize().unwrap_or(css_path.clone());
                tracker.record_write(canonical, final_css.as_bytes());

                log::info!("Saved CSS for {} to {:?}", family, css_path);

                Ok(())
            });

            Ok(AsyncIOTask::new(handle))
        })?;
    fonts_module.set("download_google_font", download_google_font_fn)?;

    Ok(fonts_module)
}

/// Generate cache path from URL and cache option
fn get_cache_path(url: &str, project_root: &Path, cache_option: &CacheOption) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    use twox_hash::XxHash64;

    match cache_option {
        CacheOption::Disabled => None,
        CacheOption::Auto => {
            let mut hasher = XxHash64::with_seed(0);
            url.hash(&mut hasher);
            let hash = hasher.finish();
            Some(
                project_root
                    .join(".rs-web-cache/downloads")
                    .join(format!("{:016x}", hash)),
            )
        }
        CacheOption::Path(base_path) => {
            let mut hasher = XxHash64::with_seed(0);
            url.hash(&mut hasher);
            let hash = hasher.finish();
            Some(base_path.join(format!("{:016x}", hash)))
        }
    }
}

/// Fetch Google Fonts CSS with Chrome User-Agent (async with caching)
async fn fetch_google_fonts_css_async(
    url: &str,
    project_root: &Path,
    cache_option: &CacheOption,
) -> std::result::Result<String, String> {
    // Check cache first
    let cache_path = get_cache_path(url, project_root, cache_option);
    if let Some(ref path) = cache_path
        && let Ok(cached) = tokio::fs::read_to_string(path).await
    {
        log::debug!("Using cached Google Fonts CSS: {:?}", path);
        return Ok(cached);
    }

    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let text = resp.text().await.map_err(|e| e.to_string())?;

    // Write to cache if enabled
    if let Some(ref path) = cache_path {
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let _ = tokio::fs::write(path, &text).await;
        log::debug!("Cached Google Fonts CSS: {:?}", path);
    }

    Ok(text)
}

/// Extract filename from Google Fonts URL
fn extract_font_filename(url: &str) -> Option<String> {
    if let Some(pos) = url.rfind('/') {
        let path = &url[pos + 1..];
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
