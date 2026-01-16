//! Image processing functions for Lua API
//!
//! Functions: image_dimensions, image_resize, image_convert, image_optimize

use super::tracker::SharedTracker;
use mlua::{Lua, Result, Table, Value};
use std::path::{Path, PathBuf};

/// Register image processing functions on the module table
pub fn register(
    lua: &Lua,
    module: &Table,
    project_root: &Path,
    tracker: SharedTracker,
) -> Result<()> {
    let root = project_root.to_path_buf();

    // image_dimensions(path) - Get image width and height
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let image_dimensions_fn = lua.create_function(move |lua, path: String| {
        use image::GenericImageView;

        let full_path = if Path::new(&path).is_absolute() {
            PathBuf::from(&path)
        } else {
            root_clone.join(&path)
        };

        // Read file for tracking (canonicalize for consistent path matching)
        if let Ok(content) = std::fs::read(&full_path) {
            let canonical = full_path
                .canonicalize()
                .unwrap_or_else(|_| full_path.clone());
            tracker_clone.record_read(canonical, &content);
        }

        match image::open(&full_path) {
            Ok(img) => {
                let (width, height) = img.dimensions();
                let result = lua.create_table()?;
                result.set("width", width)?;
                result.set("height", height)?;
                Ok(Value::Table(result))
            }
            Err(_) => Ok(Value::Nil),
        }
    })?;
    module.set("image_dimensions", image_dimensions_fn)?;

    // image_resize(input, output, options) - Resize image
    // options: { width: number, height?: number, quality?: number }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let image_resize_fn = lua.create_function(
        move |_lua, (input, output, options): (String, String, Table)| {
            use image::{DynamicImage, GenericImageView, imageops};

            let width: u32 = options
                .get("width")
                .map_err(|_| mlua::Error::external("image_resize requires options.width"))?;
            let height: Option<u32> = options.get("height").ok();
            let quality: f32 = options.get("quality").unwrap_or(85.0);

            let input_path = if Path::new(&input).is_absolute() {
                PathBuf::from(&input)
            } else {
                root_clone.join(&input)
            };

            let output_path = if Path::new(&output).is_absolute() {
                PathBuf::from(&output)
            } else {
                root_clone.join(&output)
            };

            // Read input for tracking (canonicalize for consistent path matching)
            let input_content = std::fs::read(&input_path)
                .map_err(|e| mlua::Error::external(format!("Failed to read image: {}", e)))?;
            let input_canonical = input_path
                .canonicalize()
                .unwrap_or_else(|_| input_path.clone());
            tracker_clone.record_read(input_canonical, &input_content);

            let img = image::open(&input_path)
                .map_err(|e| mlua::Error::external(format!("Failed to open image: {}", e)))?;

            let (orig_w, orig_h) = img.dimensions();
            let new_height =
                height.unwrap_or_else(|| (orig_h as f64 * (width as f64 / orig_w as f64)) as u32);

            let resized = DynamicImage::ImageRgba8(imageops::resize(
                &img,
                width,
                new_height,
                imageops::FilterType::Lanczos3,
            ));

            // Create output directory if needed
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    mlua::Error::external(format!("Failed to create directory: {}", e))
                })?;
            }

            // Determine format from extension
            let ext = output_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_else(|| "png".to_string());

            match ext.as_str() {
                "webp" => {
                    let encoder = webp::Encoder::from_image(&resized)
                        .map_err(|e| mlua::Error::external(format!("WebP encode error: {}", e)))?;
                    let webp = encoder.encode(quality);
                    std::fs::write(&output_path, &*webp)
                        .map_err(|e| mlua::Error::external(format!("Failed to write: {}", e)))?;
                    let output_canonical = output_path.canonicalize().unwrap_or(output_path);
                    tracker_clone.record_write(output_canonical, &webp);
                }
                _ => {
                    resized
                        .save(&output_path)
                        .map_err(|e| mlua::Error::external(format!("Failed to save: {}", e)))?;
                    // Track write by reading back (image crate doesn't give us bytes directly)
                    if let Ok(output_content) = std::fs::read(&output_path) {
                        let output_canonical = output_path.canonicalize().unwrap_or(output_path);
                        tracker_clone.record_write(output_canonical, &output_content);
                    }
                }
            }

            Ok(Value::Boolean(true))
        },
    )?;
    module.set("image_resize", image_resize_fn)?;

    // image_convert(input, output, options?) - Convert image format
    // options: { format?: string, quality?: number }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let image_convert_fn = lua.create_function(
        move |_lua, (input, output, options): (String, String, Option<Table>)| {
            // Extract options
            let format: Option<String> = options.as_ref().and_then(|t| t.get("format").ok());
            let quality: f32 = options
                .as_ref()
                .and_then(|t| t.get::<f32>("quality").ok())
                .unwrap_or(85.0);

            let input_path = if Path::new(&input).is_absolute() {
                PathBuf::from(&input)
            } else {
                root_clone.join(&input)
            };

            let output_path = if Path::new(&output).is_absolute() {
                PathBuf::from(&output)
            } else {
                root_clone.join(&output)
            };

            // Read input for tracking (canonicalize for consistent path matching)
            let input_content = std::fs::read(&input_path)
                .map_err(|e| mlua::Error::external(format!("Failed to read image: {}", e)))?;
            let input_canonical = input_path
                .canonicalize()
                .unwrap_or_else(|_| input_path.clone());
            tracker_clone.record_read(input_canonical, &input_content);

            let img = image::open(&input_path)
                .map_err(|e| mlua::Error::external(format!("Failed to open image: {}", e)))?;

            // Create output directory if needed
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    mlua::Error::external(format!("Failed to create directory: {}", e))
                })?;
            }

            // Determine format from argument or extension
            let ext = format.unwrap_or_else(|| {
                output_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_else(|| "png".to_string())
            });

            match ext.as_str() {
                "webp" => {
                    let encoder = webp::Encoder::from_image(&img)
                        .map_err(|e| mlua::Error::external(format!("WebP encode error: {}", e)))?;
                    let webp = encoder.encode(quality);
                    std::fs::write(&output_path, &*webp)
                        .map_err(|e| mlua::Error::external(format!("Failed to write: {}", e)))?;
                    let output_canonical = output_path.canonicalize().unwrap_or(output_path);
                    tracker_clone.record_write(output_canonical, &webp);
                }
                _ => {
                    img.save(&output_path)
                        .map_err(|e| mlua::Error::external(format!("Failed to save: {}", e)))?;
                    if let Ok(output_content) = std::fs::read(&output_path) {
                        let output_canonical = output_path.canonicalize().unwrap_or(output_path);
                        tracker_clone.record_write(output_canonical, &output_content);
                    }
                }
            }

            Ok(Value::Boolean(true))
        },
    )?;
    module.set("image_convert", image_convert_fn)?;

    // image_optimize(input, output, options?) - Optimize/compress image
    // options: { quality?: number }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let image_optimize_fn = lua.create_function(
        move |_lua, (input, output, options): (String, String, Option<Table>)| {
            let quality: f32 = options
                .as_ref()
                .and_then(|t| t.get::<f32>("quality").ok())
                .unwrap_or(85.0);

            let input_path = if Path::new(&input).is_absolute() {
                PathBuf::from(&input)
            } else {
                root_clone.join(&input)
            };

            let output_path = if Path::new(&output).is_absolute() {
                PathBuf::from(&output)
            } else {
                root_clone.join(&output)
            };

            // Read input for tracking (canonicalize for consistent path matching)
            let input_content = std::fs::read(&input_path)
                .map_err(|e| mlua::Error::external(format!("Failed to read image: {}", e)))?;
            let input_canonical = input_path
                .canonicalize()
                .unwrap_or_else(|_| input_path.clone());
            tracker_clone.record_read(input_canonical, &input_content);

            let img = image::open(&input_path)
                .map_err(|e| mlua::Error::external(format!("Failed to open image: {}", e)))?;

            // Create output directory if needed
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    mlua::Error::external(format!("Failed to create directory: {}", e))
                })?;
            }

            // Determine format from extension
            let ext = output_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_else(|| "webp".to_string());

            match ext.as_str() {
                "webp" => {
                    let encoder = webp::Encoder::from_image(&img)
                        .map_err(|e| mlua::Error::external(format!("WebP encode error: {}", e)))?;
                    let webp = encoder.encode(quality);
                    std::fs::write(&output_path, &*webp)
                        .map_err(|e| mlua::Error::external(format!("Failed to write: {}", e)))?;
                    let output_canonical = output_path.canonicalize().unwrap_or(output_path);
                    tracker_clone.record_write(output_canonical, &webp);
                }
                _ => {
                    // For non-webp, just save (quality not directly controllable for png)
                    img.save(&output_path)
                        .map_err(|e| mlua::Error::external(format!("Failed to save: {}", e)))?;
                    if let Ok(output_content) = std::fs::read(&output_path) {
                        let output_canonical = output_path.canonicalize().unwrap_or(output_path);
                        tracker_clone.record_write(output_canonical, &output_content);
                    }
                }
            }

            Ok(Value::Boolean(true))
        },
    )?;
    module.set("image_optimize", image_optimize_fn)?;

    Ok(())
}
