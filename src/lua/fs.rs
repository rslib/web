//! File system operations module (rs.fs)
//!
//! File system operations:
//! - rs.fs.read, rs.fs.write, rs.fs.copy, rs.fs.exists, etc. (sequential)
//! - rs.fs.par.read, rs.fs.par.copy, etc. (parallel)

use super::helpers::{is_path_within_root, resolve_path};
use crate::tracker::SharedTracker;
use mlua::{Lua, Result, Table, Value};
use rayon::prelude::*;
use std::path::Path;

/// Create the fs module table
pub fn create_module(
    lua: &Lua,
    project_root: &Path,
    sandbox: bool,
    tracker: SharedTracker,
) -> Result<Table> {
    let fs = lua.create_table()?;
    let root = project_root.to_path_buf();

    // ========================================================================
    // Sequential Operations
    // ========================================================================

    // read(path) - Read a file as text
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let read_fn = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        match std::fs::read_to_string(&resolved) {
            Ok(content) => {
                let canonical = resolved.canonicalize().unwrap_or(resolved);
                tracker_clone.record_read(canonical, content.as_bytes());
                Ok(Value::String(lua.create_string(&content)?))
            }
            Err(_) => Ok(Value::Nil),
        }
    })?;
    fs.set("read", read_fn)?;

    // write(path, content) - Write content to a file
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let write_fn = lua.create_function(move |_, (path, content): (String, String)| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot write '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        // Create parent directories if needed
        if let Some(parent) = resolved.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&resolved, &content) {
            Ok(_) => {
                let canonical = resolved.canonicalize().unwrap_or(resolved);
                tracker_clone.record_write(canonical, content.as_bytes());
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    })?;
    fs.set("write", write_fn)?;

    // copy(src, dst) - Copy a file (works with binary files)
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let copy_fn = lua.create_function(move |_, (src, dest): (String, String)| {
        let src_resolved = resolve_path(&src, &root_clone);
        let dest_resolved = resolve_path(&dest, &root_clone);

        if sandbox && !is_path_within_root(&src_resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot read '{}' outside project directory. Set lua.sandbox = false to disable.",
                src
            )));
        }
        if sandbox && !is_path_within_root(&dest_resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot write '{}' outside project directory. Set lua.sandbox = false to disable.",
                dest
            )));
        }

        // Read source for tracking
        let content = std::fs::read(&src_resolved).unwrap_or_default();

        // Create parent directories if needed
        if let Some(parent) = dest_resolved.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match std::fs::copy(&src_resolved, &dest_resolved) {
            Ok(_) => {
                let src_canonical = src_resolved.canonicalize().unwrap_or(src_resolved);
                let dest_canonical = dest_resolved.canonicalize().unwrap_or(dest_resolved);
                tracker_clone.record_read(src_canonical, &content);
                tracker_clone.record_write(dest_canonical, &content);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    })?;
    fs.set("copy", copy_fn)?;

    // exists(path) - Check if a file exists
    let root_clone = root.clone();
    let exists_fn = lua.create_function(move |_, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }
        Ok(resolved.exists())
    })?;
    fs.set("exists", exists_fn)?;

    // list(path, pattern?) - List files in directory
    let root_clone = root.clone();
    let list_fn = lua.create_function(move |lua, (path, pattern): (String, Option<String>)| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        let pattern = pattern.unwrap_or_else(|| "*".to_string());
        let glob_pattern = format!("{}/{}", resolved.display(), pattern);

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
    fs.set("list", list_fn)?;

    // list_dirs(path) - List subdirectories
    let root_clone = root.clone();
    let list_dirs_fn = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
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
                    dirs.push(name.to_string());
                }
            }
        }
        dirs.sort();

        let result = lua.create_table()?;
        for (i, dir) in dirs.into_iter().enumerate() {
            result.set(i + 1, dir)?;
        }
        Ok(result)
    })?;
    fs.set("list_dirs", list_dirs_fn)?;

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
    fs.set("glob", glob_fn)?;

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
    fs.set("scan", scan_fn)?;

    // ========================================================================
    // Parallel Operations (rs.fs.par.*)
    // ========================================================================

    let par = lua.create_table()?;

    // par.read(paths) - Read multiple files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let par_read_fn = lua.create_function(move |lua, paths: Table| {
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        let tracker_ref = &tracker_clone;
        let results: Vec<Option<String>> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, &root_clone);
                if sandbox && !is_path_within_root(&resolved, &root_clone) {
                    return None;
                }
                std::fs::read_to_string(&resolved).ok().inspect(|content| {
                    let canonical = resolved.canonicalize().unwrap_or(resolved);
                    tracker_ref.record_read(canonical, content.as_bytes());
                })
            })
            .collect();

        let result_table = lua.create_table()?;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Some(content) => result_table.set(i + 1, lua.create_string(&content)?)?,
                None => result_table.set(i + 1, Value::Nil)?,
            }
        }
        Ok(result_table)
    })?;
    par.set("read", par_read_fn)?;

    // par.exists(paths) - Check multiple files exist in parallel
    let root_clone = root.clone();
    let par_exists_fn = lua.create_function(move |lua, paths: Table| {
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        let results: Vec<bool> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, &root_clone);
                if sandbox && !is_path_within_root(&resolved, &root_clone) {
                    return false;
                }
                resolved.exists()
            })
            .collect();

        let result_table = lua.create_table()?;
        for (i, exists) in results.into_iter().enumerate() {
            result_table.set(i + 1, exists)?;
        }
        Ok(result_table)
    })?;
    par.set("exists", par_exists_fn)?;

    // par.copy(sources, dests) - Copy multiple files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let par_copy_fn = lua.create_function(move |lua, (sources, dests): (Table, Table)| {
        let source_list: Vec<String> = sources
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();
        let dest_list: Vec<String> = dests
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        if source_list.len() != dest_list.len() {
            return Err(mlua::Error::external(
                "fs.par.copy: sources and destinations must have same length",
            ));
        }

        let pairs: Vec<_> = source_list.into_iter().zip(dest_list).collect();
        let tracker_ref = &tracker_clone;
        let root_ref = &root_clone;

        let results: Vec<std::result::Result<(), String>> = pairs
            .par_iter()
            .map(|(src, dest)| {
                let src_path = resolve_path(src, root_ref);
                let dest_path = resolve_path(dest, root_ref);

                if sandbox && !is_path_within_root(&src_path, root_ref) {
                    return Err(format!("Source path outside sandbox: {}", src));
                }

                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create directory: {}", e))?;
                }

                let content = std::fs::read(&src_path)
                    .map_err(|e| format!("Failed to read {}: {}", src, e))?;

                let src_canonical = src_path.canonicalize().unwrap_or(src_path);
                tracker_ref.record_read(src_canonical, &content);

                std::fs::write(&dest_path, &content)
                    .map_err(|e| format!("Failed to write {}: {}", dest, e))?;

                let dest_canonical = dest_path.canonicalize().unwrap_or(dest_path);
                tracker_ref.record_write(dest_canonical, &content);

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
    })?;
    par.set("copy", par_copy_fn)?;

    // par.create_dirs(paths) - Create multiple directories in parallel
    let root_clone = root.clone();
    let par_create_dirs_fn = lua.create_function(move |lua, paths: Table| {
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        let root_ref = &root_clone;
        let results: Vec<std::result::Result<(), String>> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, root_ref);
                std::fs::create_dir_all(&resolved)
                    .map_err(|e| format!("Failed to create {}: {}", path, e))
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
    })?;
    par.set("create_dirs", par_create_dirs_fn)?;

    fs.set("par", par)?;

    Ok(fs)
}
