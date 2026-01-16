//! File operation functions for Lua API
//!
//! Functions: load_json, load_yaml, load_toml, read_frontmatter, read_file,
//!            file_exists, list_files, list_dirs, write_file, copy_file

use super::helpers::{is_path_within_root, parse_frontmatter_content, resolve_path};
use super::tracker::SharedTracker;
use mlua::{Lua, LuaSerdeExt, Result, Table, Value};
use std::path::Path;

/// Register file operation functions on the module table
pub fn register(
    lua: &Lua,
    module: &Table,
    project_root: &Path,
    sandbox: bool,
    tracker: SharedTracker,
) -> Result<()> {
    let root = project_root.to_path_buf();

    // load_json(path) - Load and parse a JSON file
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let load_json = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        let content = match std::fs::read_to_string(&resolved) {
            Ok(c) => c,
            Err(_) => return Ok(Value::Nil),
        };

        // Track the read (canonicalize for consistent path matching)
        let canonical = resolved.canonicalize().unwrap_or(resolved);
        tracker_clone.record_read(canonical, content.as_bytes());

        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(v) => lua.to_value(&v),
            Err(_) => Ok(Value::Nil),
        }
    })?;
    module.set("load_json", load_json)?;

    // load_yaml(path) - Load and parse a YAML file
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let load_yaml = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        let content = match std::fs::read_to_string(&resolved) {
            Ok(c) => c,
            Err(_) => return Ok(Value::Nil),
        };

        // Track the read (canonicalize for consistent path matching)
        let canonical = resolved.canonicalize().unwrap_or(resolved);
        tracker_clone.record_read(canonical, content.as_bytes());

        match serde_yaml::from_str::<serde_json::Value>(&content) {
            Ok(v) => lua.to_value(&v),
            Err(_) => Ok(Value::Nil),
        }
    })?;
    module.set("load_yaml", load_yaml)?;

    // load_toml(path) - Load and parse a TOML file
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let load_toml = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        let content = match std::fs::read_to_string(&resolved) {
            Ok(c) => c,
            Err(_) => return Ok(Value::Nil),
        };

        // Track the read (canonicalize for consistent path matching)
        let canonical = resolved.canonicalize().unwrap_or(resolved);
        tracker_clone.record_read(canonical, content.as_bytes());

        match toml::from_str::<toml::Value>(&content) {
            Ok(v) => {
                // Convert TOML to JSON
                let json_str = serde_json::to_string(&v).unwrap_or_default();
                match serde_json::from_str::<serde_json::Value>(&json_str) {
                    Ok(jv) => lua.to_value(&jv),
                    Err(_) => Ok(Value::Nil),
                }
            }
            Err(_) => Ok(Value::Nil),
        }
    })?;
    module.set("load_toml", load_toml)?;

    // read_frontmatter(path) - Extract frontmatter and content from markdown file
    // Returns: { raw = "...", content = "...", ...frontmatter_fields } or nil
    // Frontmatter fields are merged to top level for convenience
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let read_frontmatter = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        let raw = match std::fs::read_to_string(&resolved) {
            Ok(c) => c,
            Err(_) => return Ok(Value::Nil),
        };

        // Track the read (canonicalize for consistent path matching)
        let canonical = resolved.canonicalize().unwrap_or(resolved);
        tracker_clone.record_read(canonical, raw.as_bytes());

        // Parse frontmatter (YAML between --- or TOML between +++)
        let (frontmatter, content) = parse_frontmatter_content(&raw);

        let result = lua.create_table()?;
        result.set("raw", raw)?;
        result.set("content", content)?;

        // Merge frontmatter fields to top level for convenience
        if let Some(fm) = frontmatter
            && let serde_json::Value::Object(map) = fm {
                for (key, value) in map {
                    result.set(key, lua.to_value(&value)?)?;
                }
            }

        Ok(Value::Table(result))
    })?;
    module.set("read_frontmatter", read_frontmatter)?;

    // read_file(path) - Read a file as text
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let read_file = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        match std::fs::read_to_string(&resolved) {
            Ok(content) => {
                // Track the read (canonicalize for consistent path matching)
                let canonical = resolved.canonicalize().unwrap_or(resolved);
                tracker_clone.record_read(canonical, content.as_bytes());
                Ok(Value::String(lua.create_string(&content)?))
            }
            Err(_) => Ok(Value::Nil),
        }
    })?;
    module.set("read_file", read_file)?;

    // file_exists(path) - Check if a file exists
    let root_clone = root.clone();
    let file_exists = lua.create_function(move |_, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }
        Ok(resolved.exists())
    })?;
    module.set("file_exists", file_exists)?;

    // list_files(path, pattern?) - List files in directory
    let root_clone = root.clone();
    let list_files = lua.create_function(move |lua, (path, pattern): (String, Option<String>)| {
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
                // Skip files outside sandbox (in case glob pattern escapes)
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
    module.set("list_files", list_files)?;

    // list_dirs(path) - List subdirectories
    let root_clone = root.clone();
    let list_dirs = lua.create_function(move |lua, path: String| {
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
                // Skip directories outside sandbox
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
    module.set("list_dirs", list_dirs)?;

    // write_file(path, content) - Write content to a file
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let write_file = lua.create_function(move |_, (path, content): (String, String)| {
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
                // Track the write (canonicalize for consistent path matching)
                let canonical = resolved.canonicalize().unwrap_or(resolved);
                tracker_clone.record_write(canonical, content.as_bytes());
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    })?;
    module.set("write_file", write_file)?;

    // copy_file(src, dest) - Copy a file (works with binary files)
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let copy_file = lua.create_function(move |_, (src, dest): (String, String)| {
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
                // Track both read and write (canonicalize for consistent path matching)
                let src_canonical = src_resolved.canonicalize().unwrap_or(src_resolved);
                let dest_canonical = dest_resolved.canonicalize().unwrap_or(dest_resolved);
                tracker_clone.record_read(src_canonical, &content);
                tracker_clone.record_write(dest_canonical, &content);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    })?;
    module.set("copy_file", copy_file)?;

    Ok(())
}
