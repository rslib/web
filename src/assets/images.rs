use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView, imageops};
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use webp::{Encoder, WebPMemory};

/// Image optimization configuration
pub struct ImageConfig {
    pub quality: f32,
    pub scale_factor: f64,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            quality: 85.0,
            scale_factor: 1.0,
        }
    }
}

/// Optimize all images in a directory
pub fn optimize_images<P: AsRef<Path>>(
    input_dir: P,
    output_dir: P,
    config: &ImageConfig,
) -> Result<()> {
    let input_dir = input_dir.as_ref();
    let output_dir = output_dir.as_ref();

    if !input_dir.exists() {
        return Ok(());
    }

    fs::create_dir_all(output_dir)?;

    // Find all image files
    let image_files: Vec<_> = fs::read_dir(input_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let path = entry.path();
            path.extension().is_some_and(|ext| {
                let ext = ext.to_string_lossy().to_lowercase();
                ext == "jpg" || ext == "jpeg" || ext == "png"
            })
        })
        .collect();

    // Process images
    image_files.par_iter().try_for_each(|entry| {
        let path = entry.path();
        optimize_image_to_dir(&path, output_dir, config)
    })?;

    Ok(())
}

/// Optimize a single image (batch version - output to directory)
fn optimize_image_to_dir(input_path: &Path, output_dir: &Path, config: &ImageConfig) -> Result<()> {
    let img = image::open(input_path)
        .with_context(|| format!("Failed to open image: {:?}", input_path))?;

    let (w, h) = img.dimensions();

    // Resize if scale factor is less than 1
    let img = if config.scale_factor < 1.0 {
        let new_w = (w as f64 * config.scale_factor) as u32;
        let new_h = (h as f64 * config.scale_factor) as u32;
        DynamicImage::ImageRgba8(imageops::resize(
            &img,
            new_w,
            new_h,
            imageops::FilterType::Triangle,
        ))
    } else {
        img
    };

    // Get the file stem for output naming
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");

    // Convert to WebP
    let encoder = Encoder::from_image(&img)
        .map_err(|e| anyhow::anyhow!("Failed to create WebP encoder: {}", e))?;
    let webp: WebPMemory = encoder.encode(config.quality);

    let webp_path = output_dir.join(format!("{}.webp", stem));
    fs::write(&webp_path, &*webp)
        .with_context(|| format!("Failed to write WebP: {:?}", webp_path))?;

    // Also copy original (as fallback)
    let file_name = input_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid file path: {:?}", input_path))?;
    let original_path = output_dir.join(file_name);
    fs::copy(input_path, &original_path)
        .with_context(|| format!("Failed to copy original: {:?}", original_path))?;

    Ok(())
}

/// Optimize a single image to a specific output path (for incremental builds)
pub fn optimize_single_image(
    input_path: &Path,
    output_path: &Path,
    config: &ImageConfig,
) -> Result<()> {
    let img = image::open(input_path)
        .with_context(|| format!("Failed to open image: {:?}", input_path))?;

    let (w, h) = img.dimensions();

    // Resize if scale factor is less than 1
    let img = if config.scale_factor < 1.0 {
        let new_w = (w as f64 * config.scale_factor) as u32;
        let new_h = (h as f64 * config.scale_factor) as u32;
        DynamicImage::ImageRgba8(imageops::resize(
            &img,
            new_w,
            new_h,
            imageops::FilterType::Triangle,
        ))
    } else {
        img
    };

    // Get output directory
    let output_dir = output_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid output path: {:?}", output_path))?;

    // Get the file stem for WebP naming
    let stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");

    // Convert to WebP
    let encoder = Encoder::from_image(&img)
        .map_err(|e| anyhow::anyhow!("Failed to create WebP encoder: {}", e))?;
    let webp: WebPMemory = encoder.encode(config.quality);

    let webp_path = output_dir.join(format!("{}.webp", stem));
    fs::write(&webp_path, &*webp)
        .with_context(|| format!("Failed to write WebP: {:?}", webp_path))?;

    // Also copy original (as fallback)
    fs::copy(input_path, output_path)
        .with_context(|| format!("Failed to copy original: {:?}", output_path))?;

    Ok(())
}

/// Copy a single static file (for incremental builds)
pub fn copy_single_static_file(src: &Path, dest: &Path) -> Result<()> {
    fs::copy(src, dest).with_context(|| format!("Failed to copy {:?} to {:?}", src, dest))?;
    Ok(())
}

/// Copy static files (non-images) from source to destination
pub fn copy_static_files<P: AsRef<Path>>(src_dir: P, dest_dir: P) -> Result<()> {
    let src_dir = src_dir.as_ref();
    let dest_dir = dest_dir.as_ref();

    if !src_dir.exists() {
        return Ok(());
    }

    fs::create_dir_all(dest_dir)?;

    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());

            // Skip images (handled separately) and hidden files
            let is_image = ext
                .as_ref()
                .is_some_and(|e| e == "jpg" || e == "jpeg" || e == "png" || e == "webp");

            let is_hidden = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.'));

            if !is_image
                && !is_hidden
                && let Some(file_name) = path.file_name()
            {
                let dest_path = dest_dir.join(file_name);
                fs::copy(&path, &dest_path)
                    .with_context(|| format!("Failed to copy {:?}", path))?;
            }
        }
    }

    Ok(())
}
