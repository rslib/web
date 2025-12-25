use anyhow::{Context, Result};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

/// Build and minify CSS files
pub fn build_css<P: AsRef<Path>>(styles_dir: P, output_path: P, minify: bool) -> Result<()> {
    let styles_dir = styles_dir.as_ref();
    let output_path = output_path.as_ref();

    // Collect all CSS files and sort alphabetically
    let mut css_files: Vec<PathBuf> = fs::read_dir(styles_dir)
        .with_context(|| format!("Failed to read styles directory: {:?}", styles_dir))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().map_or(false, |ext| ext == "css"))
        .collect();

    css_files.sort();

    // Read files in parallel
    let contents: Result<Vec<(PathBuf, String)>> = css_files
        .par_iter()
        .map(|path| {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read CSS file: {:?}", path))?;
            Ok((path.clone(), content))
        })
        .collect();

    // Concatenate in order
    let css_buffer: String = {
        let mut contents = contents?;
        contents.sort_by(|a, b| a.0.cmp(&b.0));
        contents
            .into_iter()
            .map(|(_, c)| c)
            .collect::<Vec<_>>()
            .join("\n")
    };

    // Minify if requested
    let output = if minify {
        minifier::css::minify(&css_buffer)
            .map_err(|e| anyhow::anyhow!("CSS minification failed: {}", e))?
            .to_string()
    } else {
        css_buffer
    };

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(output_path, output)
        .with_context(|| format!("Failed to write CSS to {:?}", output_path))?;

    Ok(())
}
