//! Search functions - glob, scan

use super::helpers::{is_path_within_root, resolve_path};
use mlua::{Lua, Result, Table};
use std::path::Path;

/// Register search functions on the module table
pub fn register(lua: &Lua, module: &Table, project_root: &Path, sandbox: bool) -> Result<()> {
    let root = project_root.to_path_buf();

    // glob(pattern) - Find files matching glob pattern, returns FileInfo[]
    let root_clone = root.clone();
    let glob_fn = lua.create_function(move |lua, pattern: String| {
        let glob_pattern = if pattern.starts_with('/') || pattern.contains(':') {
            pattern.clone()
        } else {
            format!("{}/{}", root_clone.display(), pattern)
        };

        let mut files = Vec::new();
        if let Ok(entries) = glob::glob(&glob_pattern) {
            for entry in entries.flatten() {
                if sandbox && !is_path_within_root(&entry, &root_clone) {
                    continue;
                }
                if entry.is_file() {
                    let table = lua.create_table()?;
                    table.set("path", entry.to_string_lossy().to_string())?;
                    table.set(
                        "name",
                        entry
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    )?;
                    table.set(
                        "stem",
                        entry
                            .file_stem()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    )?;
                    table.set(
                        "ext",
                        entry
                            .extension()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    )?;
                    table.set("is_dir", false)?;
                    // Get modified time
                    if let Ok(metadata) = entry.metadata()
                        && let Ok(modified) = metadata.modified()
                        && let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
                    {
                        table.set("modified", duration.as_secs() as i64)?;
                    }
                    files.push(table);
                }
            }
        }

        let result = lua.create_table()?;
        for (i, file) in files.into_iter().enumerate() {
            result.set(i + 1, file)?;
        }
        Ok(result)
    })?;
    module.set("glob", glob_fn)?;

    // scan(path) - List directories in path, returns FileInfo[]
    let root_clone = root.clone();
    let scan_fn = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory.",
                path
            )));
        }

        let mut dirs = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&resolved) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if sandbox && !is_path_within_root(&entry_path, &root_clone) {
                    continue;
                }
                if entry_path.is_dir()
                    && let Some(name) = entry_path.file_name().and_then(|n| n.to_str())
                    && !name.starts_with('.')
                {
                    let table = lua.create_table()?;
                    table.set("path", entry_path.to_string_lossy().to_string())?;
                    table.set("name", name.to_string())?;
                    table.set("stem", name.to_string())?;
                    table.set("ext", "")?;
                    table.set("is_dir", true)?;
                    // Get modified time
                    if let Ok(metadata) = entry_path.metadata()
                        && let Ok(modified) = metadata.modified()
                        && let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
                    {
                        table.set("modified", duration.as_secs() as i64)?;
                    }
                    dirs.push(table);
                }
            }
        }

        let result = lua.create_table()?;
        for (i, dir) in dirs.into_iter().enumerate() {
            result.set(i + 1, dir)?;
        }
        Ok(result)
    })?;
    module.set("scan", scan_fn)?;

    Ok(())
}
