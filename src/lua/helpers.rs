//! Shared helper functions for Lua API

use std::path::{Path, PathBuf};

/// Check if a path is within the project root (for sandbox enforcement)
pub fn is_path_within_root(path: &Path, root: &Path) -> bool {
    let resolved = if path.exists() {
        path.canonicalize().ok()
    } else {
        // For non-existing paths, canonicalize the parent and append the filename
        path.parent()
            .map(|p| {
                if p.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    p.to_path_buf()
                }
            })
            .and_then(|p| p.canonicalize().ok())
            .map(|p| p.join(path.file_name().unwrap_or_default()))
    };

    match resolved {
        Some(abs_path) => abs_path.starts_with(root),
        None => false,
    }
}

/// Resolve a path relative to project root
pub fn resolve_path(path: &str, root: &Path) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

/// Parse frontmatter from content string
/// Supports YAML (---) and TOML (+++) delimiters
/// Returns (frontmatter_value, content_without_frontmatter)
pub fn parse_frontmatter_content(raw: &str) -> (Option<serde_json::Value>, String) {
    let trimmed = raw.trim_start();

    // Check for YAML frontmatter (---)
    if trimmed.starts_with("---")
        && let Some(end_idx) = trimmed[3..].find("\n---")
    {
        let fm_str = &trimmed[3..3 + end_idx].trim();
        let content_start = 3 + end_idx + 4; // Skip past closing ---\n
        let content = trimmed[content_start..].trim_start().to_string();

        match serde_yaml::from_str::<serde_json::Value>(fm_str) {
            Ok(v) => return (Some(v), content),
            Err(_) => return (None, raw.to_string()),
        }
    }

    // Check for TOML frontmatter (+++)
    if trimmed.starts_with("+++")
        && let Some(end_idx) = trimmed[3..].find("\n+++")
    {
        let fm_str = &trimmed[3..3 + end_idx].trim();
        let content_start = 3 + end_idx + 4; // Skip past closing +++\n
        let content = trimmed[content_start..].trim_start().to_string();

        match toml::from_str::<toml::Value>(fm_str) {
            Ok(v) => {
                // Convert TOML to JSON
                let json_str = serde_json::to_string(&v).unwrap_or_default();
                match serde_json::from_str(&json_str) {
                    Ok(jv) => return (Some(jv), content),
                    Err(_) => return (None, raw.to_string()),
                }
            }
            Err(_) => return (None, raw.to_string()),
        }
    }

    // No frontmatter found
    (None, raw.to_string())
}
