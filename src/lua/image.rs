//! Image processing module (rs.image)
//!
//! Image operations:
//! - rs.image.dimensions, rs.image.resize, rs.image.convert, rs.image.optimize (sequential)
//! - rs.image.par.resize, rs.image.par.convert, rs.image.par.optimize (parallel)

use crate::tracker::SharedTracker;
use image::DynamicImage;
use mlua::{Lua, Result, Table, Value};
use rayon::prelude::*;
use std::path::{Path, PathBuf};

/// Encode an image as AVIF. Quality 0-100 (higher is better). Speed is fixed at
/// 6 (ravif scale: 1 slowest/best ... 10 fastest/worst); a build pipeline can
/// afford a slower encode than a real-time service, and 6 still beats webp
/// substantially in compression at comparable visual quality.
fn encode_avif(img: &DynamicImage, quality: f32) -> std::result::Result<Vec<u8>, String> {
    use image::codecs::avif::AvifEncoder;
    use image::{ExtendedColorType, ImageEncoder};

    let mut buf = Vec::new();
    let q = quality.round().clamp(0.0, 100.0) as u8;
    let encoder = AvifEncoder::new_with_speed_quality(&mut buf, 6, q);
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    encoder
        .write_image(rgba.as_raw(), w, h, ExtendedColorType::Rgba8)
        .map_err(|e| format!("AVIF encode error: {}", e))?;
    Ok(buf)
}

/// Apply EXIF orientation to an image
fn apply_exif_orientation(img: DynamicImage, path: &Path) -> DynamicImage {
    use std::fs::File;
    use std::io::BufReader;

    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return img,
    };

    let exif = match exif::Reader::new().read_from_container(&mut BufReader::new(file)) {
        Ok(e) => e,
        Err(_) => return img,
    };

    let orientation = exif
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .unwrap_or(1);

    // Apply transformation based on EXIF orientation
    match orientation {
        1 => img,
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

/// Resolve path relative to project root
fn resolve_path(path: &str, root: &Path) -> PathBuf {
    if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    }
}

/// Create the image module table
pub fn create_module(lua: &Lua, project_root: &Path, tracker: SharedTracker) -> Result<Table> {
    let image_mod = lua.create_table()?;
    let root = project_root.to_path_buf();

    // ========================================================================
    // Sequential Operations
    // ========================================================================

    // dimensions(path) - Get image width and height
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let dimensions_fn = lua.create_function(move |lua, path: String| {
        use image::GenericImageView;

        let full_path = resolve_path(&path, &root_clone);

        if let Ok(content) = std::fs::read(&full_path) {
            let canonical = full_path
                .canonicalize()
                .unwrap_or_else(|_| full_path.clone());
            tracker_clone.record_read(canonical, &content);
        }

        match image::open(&full_path) {
            Ok(img) => {
                let img = apply_exif_orientation(img, &full_path);
                let (width, height) = img.dimensions();
                let result = lua.create_table()?;
                result.set("width", width)?;
                result.set("height", height)?;
                Ok(Value::Table(result))
            }
            Err(_) => Ok(Value::Nil),
        }
    })?;
    image_mod.set("dimensions", dimensions_fn)?;

    // resize(input, output, options) - Resize image
    // options: { width: number, height?: number, quality?: number }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let resize_fn = lua.create_function(
        move |_lua, (input, output, options): (String, String, Table)| {
            use image::{GenericImageView, imageops};

            let width: u32 = options
                .get("width")
                .map_err(|_| mlua::Error::external("image.resize requires options.width"))?;
            let height: Option<u32> = options.get("height").ok();
            let quality: f32 = options.get("quality").unwrap_or(85.0);

            let input_path = resolve_path(&input, &root_clone);
            let output_path = resolve_path(&output, &root_clone);

            let input_content = std::fs::read(&input_path)
                .map_err(|e| mlua::Error::external(format!("Failed to read image: {}", e)))?;
            let input_canonical = input_path
                .canonicalize()
                .unwrap_or_else(|_| input_path.clone());
            tracker_clone.record_read(input_canonical, &input_content);

            let img = image::open(&input_path)
                .map_err(|e| mlua::Error::external(format!("Failed to open image: {}", e)))?;
            let img = apply_exif_orientation(img, &input_path);

            let (orig_w, orig_h) = img.dimensions();
            let new_height =
                height.unwrap_or_else(|| (orig_h as f64 * (width as f64 / orig_w as f64)) as u32);

            let resized = DynamicImage::ImageRgba8(imageops::resize(
                &img,
                width,
                new_height,
                imageops::FilterType::Lanczos3,
            ));

            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    mlua::Error::external(format!("Failed to create directory: {}", e))
                })?;
            }

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
                "avif" => {
                    let avif = encode_avif(&resized, quality).map_err(mlua::Error::external)?;
                    std::fs::write(&output_path, &avif)
                        .map_err(|e| mlua::Error::external(format!("Failed to write: {}", e)))?;
                    let output_canonical = output_path.canonicalize().unwrap_or(output_path);
                    tracker_clone.record_write(output_canonical, &avif);
                }
                _ => {
                    resized
                        .save(&output_path)
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
    image_mod.set("resize", resize_fn)?;

    // convert(input, output, options?) - Convert image format
    // options: { format?: string, quality?: number }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let convert_fn = lua.create_function(
        move |_lua, (input, output, options): (String, String, Option<Table>)| {
            let format: Option<String> = options.as_ref().and_then(|t| t.get("format").ok());
            let quality: f32 = options
                .as_ref()
                .and_then(|t| t.get::<f32>("quality").ok())
                .unwrap_or(85.0);

            let input_path = resolve_path(&input, &root_clone);
            let output_path = resolve_path(&output, &root_clone);

            let input_content = std::fs::read(&input_path)
                .map_err(|e| mlua::Error::external(format!("Failed to read image: {}", e)))?;
            let input_canonical = input_path
                .canonicalize()
                .unwrap_or_else(|_| input_path.clone());
            tracker_clone.record_read(input_canonical, &input_content);

            let img = image::open(&input_path)
                .map_err(|e| mlua::Error::external(format!("Failed to open image: {}", e)))?;
            let img = apply_exif_orientation(img, &input_path);

            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    mlua::Error::external(format!("Failed to create directory: {}", e))
                })?;
            }

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
                "avif" => {
                    let avif = encode_avif(&img, quality).map_err(mlua::Error::external)?;
                    std::fs::write(&output_path, &avif)
                        .map_err(|e| mlua::Error::external(format!("Failed to write: {}", e)))?;
                    let output_canonical = output_path.canonicalize().unwrap_or(output_path);
                    tracker_clone.record_write(output_canonical, &avif);
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
    image_mod.set("convert", convert_fn)?;

    // optimize(input, output, options?) - Optimize/compress image
    // options: { quality?: number }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let optimize_fn = lua.create_function(
        move |_lua, (input, output, options): (String, String, Option<Table>)| {
            let quality: f32 = options
                .as_ref()
                .and_then(|t| t.get::<f32>("quality").ok())
                .unwrap_or(85.0);

            let input_path = resolve_path(&input, &root_clone);
            let output_path = resolve_path(&output, &root_clone);

            let input_content = std::fs::read(&input_path)
                .map_err(|e| mlua::Error::external(format!("Failed to read image: {}", e)))?;
            let input_canonical = input_path
                .canonicalize()
                .unwrap_or_else(|_| input_path.clone());
            tracker_clone.record_read(input_canonical, &input_content);

            let img = image::open(&input_path)
                .map_err(|e| mlua::Error::external(format!("Failed to open image: {}", e)))?;
            let img = apply_exif_orientation(img, &input_path);

            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    mlua::Error::external(format!("Failed to create directory: {}", e))
                })?;
            }

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
                "avif" => {
                    let avif = encode_avif(&img, quality).map_err(mlua::Error::external)?;
                    std::fs::write(&output_path, &avif)
                        .map_err(|e| mlua::Error::external(format!("Failed to write: {}", e)))?;
                    let output_canonical = output_path.canonicalize().unwrap_or(output_path);
                    tracker_clone.record_write(output_canonical, &avif);
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
    image_mod.set("optimize", optimize_fn)?;

    // ========================================================================
    // Parallel Operations (rs.image.par.*)
    // ========================================================================

    let par = lua.create_table()?;

    // par.resize(inputs, outputs, options) - Resize multiple images in parallel
    // options: { width: number, height?: number, quality?: number }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let par_resize_fn = lua.create_function(
        move |lua, (inputs, outputs, options): (Table, Table, Table)| {
            use image::{GenericImageView, imageops};

            let input_list: Vec<String> = inputs
                .sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect();
            let output_list: Vec<String> = outputs
                .sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect();

            if input_list.len() != output_list.len() {
                return Err(mlua::Error::external(
                    "image.par.resize: inputs and outputs must have same length",
                ));
            }

            let width: u32 = options
                .get("width")
                .map_err(|_| mlua::Error::external("image.par.resize requires options.width"))?;
            let height: Option<u32> = options.get("height").ok();
            let quality: f32 = options.get("quality").unwrap_or(85.0);

            let pairs: Vec<_> = input_list.into_iter().zip(output_list).collect();
            let tracker_ref = &tracker_clone;
            let root_ref = &root_clone;

            let results: Vec<std::result::Result<(), String>> = pairs
                .par_iter()
                .map(|(input, output)| {
                    let input_path = resolve_path(input, root_ref);
                    let output_path = resolve_path(output, root_ref);

                    let input_content = std::fs::read(&input_path)
                        .map_err(|e| format!("Failed to read {}: {}", input, e))?;
                    let input_canonical = input_path.canonicalize().unwrap_or(input_path.clone());
                    tracker_ref.record_read(input_canonical, &input_content);

                    let img = image::open(&input_path)
                        .map_err(|e| format!("Failed to open {}: {}", input, e))?;
                    let img = apply_exif_orientation(img, &input_path);

                    let (orig_w, orig_h) = img.dimensions();
                    let new_height = height
                        .unwrap_or_else(|| (orig_h as f64 * (width as f64 / orig_w as f64)) as u32);

                    let resized = DynamicImage::ImageRgba8(imageops::resize(
                        &img,
                        width,
                        new_height,
                        imageops::FilterType::Lanczos3,
                    ));

                    if let Some(parent) = output_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("Failed to create directory: {}", e))?;
                    }

                    let ext = output_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_else(|| "png".to_string());

                    let output_bytes = match ext.as_str() {
                        "webp" => {
                            let encoder = webp::Encoder::from_image(&resized)
                                .map_err(|e| format!("WebP encode error: {}", e))?;
                            encoder.encode(quality).to_vec()
                        }
                        "avif" => encode_avif(&resized, quality)?,
                        "jpg" | "jpeg" => {
                            let mut buf = Vec::new();
                            let mut cursor = std::io::Cursor::new(&mut buf);
                            resized
                                .write_to(&mut cursor, image::ImageFormat::Jpeg)
                                .map_err(|e| format!("JPEG encode error: {}", e))?;
                            buf
                        }
                        "png" => {
                            let mut buf = Vec::new();
                            let mut cursor = std::io::Cursor::new(&mut buf);
                            resized
                                .write_to(&mut cursor, image::ImageFormat::Png)
                                .map_err(|e| format!("PNG encode error: {}", e))?;
                            buf
                        }
                        _ => return Err(format!("Unsupported output format: {}", ext)),
                    };

                    std::fs::write(&output_path, &output_bytes)
                        .map_err(|e| format!("Failed to write {}: {}", output, e))?;

                    let output_canonical = output_path.canonicalize().unwrap_or(output_path);
                    tracker_ref.record_write(output_canonical, &output_bytes);

                    Ok(())
                })
                .collect();

            let result_table = lua.create_table()?;
            for (i, result) in results.into_iter().enumerate() {
                match result {
                    Ok(()) => result_table.set(i + 1, true)?,
                    Err(e) => result_table.set(i + 1, e)?,
                }
            }
            Ok(result_table)
        },
    )?;
    par.set("resize", par_resize_fn)?;

    // par.convert(inputs, outputs, options?) - Convert multiple images in parallel
    // options: { quality?: number }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let par_convert_fn = lua.create_function(
        move |lua, (inputs, outputs, options): (Table, Table, Option<Table>)| {
            let input_list: Vec<String> = inputs
                .sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect();
            let output_list: Vec<String> = outputs
                .sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect();

            if input_list.len() != output_list.len() {
                return Err(mlua::Error::external(
                    "image.par.convert: inputs and outputs must have same length",
                ));
            }

            let quality: f32 = options
                .as_ref()
                .and_then(|t| t.get("quality").ok())
                .unwrap_or(85.0);

            let pairs: Vec<_> = input_list.into_iter().zip(output_list).collect();
            let tracker_ref = &tracker_clone;
            let root_ref = &root_clone;

            let results: Vec<std::result::Result<(), String>> = pairs
                .par_iter()
                .map(|(input, output)| {
                    let input_path = resolve_path(input, root_ref);
                    let output_path = resolve_path(output, root_ref);

                    let content = std::fs::read(&input_path)
                        .map_err(|e| format!("Failed to read {}: {}", input, e))?;

                    let input_canonical = input_path.canonicalize().unwrap_or(input_path.clone());
                    tracker_ref.record_read(input_canonical, &content);

                    let img = image::open(&input_path)
                        .map_err(|e| format!("Failed to open image {}: {}", input, e))?;
                    let img = apply_exif_orientation(img, &input_path);

                    if let Some(parent) = output_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("Failed to create directory: {}", e))?;
                    }

                    let ext = output_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_else(|| "png".to_string());

                    let output_bytes = match ext.as_str() {
                        "webp" => {
                            let encoder = webp::Encoder::from_image(&img)
                                .map_err(|e| format!("WebP encoder error: {}", e))?;
                            encoder.encode(quality).to_vec()
                        }
                        "avif" => encode_avif(&img, quality)?,
                        "jpg" | "jpeg" => {
                            let mut buf = Vec::new();
                            let mut cursor = std::io::Cursor::new(&mut buf);
                            img.write_to(&mut cursor, image::ImageFormat::Jpeg)
                                .map_err(|e| format!("JPEG encode error: {}", e))?;
                            buf
                        }
                        "png" => {
                            let mut buf = Vec::new();
                            let mut cursor = std::io::Cursor::new(&mut buf);
                            img.write_to(&mut cursor, image::ImageFormat::Png)
                                .map_err(|e| format!("PNG encode error: {}", e))?;
                            buf
                        }
                        _ => return Err(format!("Unsupported output format: {}", ext)),
                    };

                    std::fs::write(&output_path, &output_bytes)
                        .map_err(|e| format!("Failed to write {}: {}", output, e))?;

                    let output_canonical = output_path.canonicalize().unwrap_or(output_path);
                    tracker_ref.record_write(output_canonical, &output_bytes);

                    Ok(())
                })
                .collect();

            let result_table = lua.create_table()?;
            for (i, result) in results.into_iter().enumerate() {
                match result {
                    Ok(()) => result_table.set(i + 1, true)?,
                    Err(e) => result_table.set(i + 1, e)?,
                }
            }
            Ok(result_table)
        },
    )?;
    par.set("convert", par_convert_fn)?;

    // par.optimize(inputs, outputs, options?) - Optimize multiple images in parallel
    // options: { quality?: number }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let par_optimize_fn = lua.create_function(
        move |lua, (inputs, outputs, options): (Table, Table, Option<Table>)| {
            let input_list: Vec<String> = inputs
                .sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect();
            let output_list: Vec<String> = outputs
                .sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect();

            if input_list.len() != output_list.len() {
                return Err(mlua::Error::external(
                    "image.par.optimize: inputs and outputs must have same length",
                ));
            }

            let quality: f32 = options
                .as_ref()
                .and_then(|t| t.get("quality").ok())
                .unwrap_or(85.0);

            let pairs: Vec<_> = input_list.into_iter().zip(output_list).collect();
            let tracker_ref = &tracker_clone;
            let root_ref = &root_clone;

            let results: Vec<std::result::Result<(), String>> = pairs
                .par_iter()
                .map(|(input, output)| {
                    let input_path = resolve_path(input, root_ref);
                    let output_path = resolve_path(output, root_ref);

                    let content = std::fs::read(&input_path)
                        .map_err(|e| format!("Failed to read {}: {}", input, e))?;

                    let input_canonical = input_path.canonicalize().unwrap_or(input_path.clone());
                    tracker_ref.record_read(input_canonical, &content);

                    let img = image::open(&input_path)
                        .map_err(|e| format!("Failed to open image {}: {}", input, e))?;
                    let img = apply_exif_orientation(img, &input_path);

                    if let Some(parent) = output_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("Failed to create directory: {}", e))?;
                    }

                    let ext = output_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_else(|| "webp".to_string());

                    let output_bytes = match ext.as_str() {
                        "webp" => {
                            let encoder = webp::Encoder::from_image(&img)
                                .map_err(|e| format!("WebP encoder error: {}", e))?;
                            encoder.encode(quality).to_vec()
                        }
                        "avif" => encode_avif(&img, quality)?,
                        "jpg" | "jpeg" => {
                            let mut buf = Vec::new();
                            let mut cursor = std::io::Cursor::new(&mut buf);
                            img.write_to(&mut cursor, image::ImageFormat::Jpeg)
                                .map_err(|e| format!("JPEG encode error: {}", e))?;
                            buf
                        }
                        "png" => {
                            let mut buf = Vec::new();
                            let mut cursor = std::io::Cursor::new(&mut buf);
                            img.write_to(&mut cursor, image::ImageFormat::Png)
                                .map_err(|e| format!("PNG encode error: {}", e))?;
                            buf
                        }
                        _ => return Err(format!("Unsupported output format: {}", ext)),
                    };

                    std::fs::write(&output_path, &output_bytes)
                        .map_err(|e| format!("Failed to write {}: {}", output, e))?;

                    let output_canonical = output_path.canonicalize().unwrap_or(output_path);
                    tracker_ref.record_write(output_canonical, &output_bytes);

                    Ok(())
                })
                .collect();

            let result_table = lua.create_table()?;
            for (i, result) in results.into_iter().enumerate() {
                match result {
                    Ok(()) => result_table.set(i + 1, true)?,
                    Err(e) => result_table.set(i + 1, e)?,
                }
            }
            Ok(result_table)
        },
    )?;
    par.set("optimize", par_optimize_fn)?;

    image_mod.set("par", par)?;

    Ok(image_mod)
}
