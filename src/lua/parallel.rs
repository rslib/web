//! Parallel processing functions for Lua API
//!
//! Functions: parallel.load_json, parallel.read_files, parallel.file_exists,
//!            parallel.load_yaml, parallel.read_frontmatter, parallel.map,
//!            parallel.filter, parallel.reduce

use super::helpers::{is_path_within_root, parse_frontmatter_content, resolve_path};
use super::tracker::SharedTracker;
use mlua::{Function, Lua, LuaSerdeExt, Result, Table, Value};
use rayon::prelude::*;
use std::path::Path;

/// Create the parallel module table
pub fn create_module(
    lua: &Lua,
    project_root: &Path,
    sandbox: bool,
    tracker: SharedTracker,
) -> Result<Table> {
    let parallel = lua.create_table()?;
    let root = project_root.to_path_buf();

    // parallel.load_json(paths) - Load multiple JSON files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let load_json_parallel = lua.create_function(move |lua, paths: Table| {
        // Collect paths from Lua table
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        // Process in parallel
        let tracker_ref = &tracker_clone;
        let results: Vec<Option<serde_json::Value>> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, &root_clone);
                if sandbox && !is_path_within_root(&resolved, &root_clone) {
                    return None;
                }
                std::fs::read_to_string(&resolved).ok().and_then(|content| {
                    tracker_ref.record_read(resolved, content.as_bytes());
                    serde_json::from_str(&content).ok()
                })
            })
            .collect();

        // Convert results back to Lua table
        let result_table = lua.create_table()?;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Some(v) => result_table.set(i + 1, lua.to_value(&v)?)?,
                None => result_table.set(i + 1, Value::Nil)?,
            }
        }
        Ok(result_table)
    })?;
    parallel.set("load_json", load_json_parallel)?;

    // parallel.read_files(paths) - Read multiple files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let read_files_parallel = lua.create_function(move |lua, paths: Table| {
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
                    tracker_ref.record_read(resolved, content.as_bytes());
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
    parallel.set("read_files", read_files_parallel)?;

    // parallel.file_exists(paths) - Check multiple files exist in parallel
    let root_clone = root.clone();
    let file_exists_parallel = lua.create_function(move |lua, paths: Table| {
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
    parallel.set("file_exists", file_exists_parallel)?;

    // parallel.load_yaml(paths) - Load multiple YAML files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let load_yaml_parallel = lua.create_function(move |lua, paths: Table| {
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        let tracker_ref = &tracker_clone;
        let results: Vec<Option<serde_json::Value>> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, &root_clone);
                if sandbox && !is_path_within_root(&resolved, &root_clone) {
                    return None;
                }
                std::fs::read_to_string(&resolved).ok().and_then(|content| {
                    tracker_ref.record_read(resolved, content.as_bytes());
                    serde_yaml::from_str(&content).ok()
                })
            })
            .collect();

        let result_table = lua.create_table()?;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Some(v) => result_table.set(i + 1, lua.to_value(&v)?)?,
                None => result_table.set(i + 1, Value::Nil)?,
            }
        }
        Ok(result_table)
    })?;
    parallel.set("load_yaml", load_yaml_parallel)?;

    // parallel.read_frontmatter(paths) - Parse frontmatter from multiple files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let read_frontmatter_parallel = lua.create_function(move |lua, paths: Table| {
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        let tracker_ref = &tracker_clone;
        let results: Vec<Option<(Option<serde_json::Value>, String, String)>> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, &root_clone);
                if sandbox && !is_path_within_root(&resolved, &root_clone) {
                    return None;
                }
                std::fs::read_to_string(&resolved).ok().map(|raw| {
                    tracker_ref.record_read(resolved, raw.as_bytes());
                    let (fm, content) = parse_frontmatter_content(&raw);
                    (fm, content, raw)
                })
            })
            .collect();

        let result_table = lua.create_table()?;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Some((fm, content, raw)) => {
                    let entry = lua.create_table()?;
                    entry.set("raw", raw)?;
                    entry.set("content", content)?;
                    match fm {
                        Some(v) => entry.set("frontmatter", lua.to_value(&v)?)?,
                        None => entry.set("frontmatter", lua.create_table()?)?,
                    }
                    result_table.set(i + 1, entry)?;
                }
                None => result_table.set(i + 1, Value::Nil)?,
            }
        }
        Ok(result_table)
    })?;
    parallel.set("read_frontmatter", read_frontmatter_parallel)?;

    // parallel.map(items, fn) - Map over items, calling Lua function (sequential fn calls, parallel-ready structure)
    let map_fn = lua.create_function(|lua, (items, func): (Table, Function)| {
        let result_table = lua.create_table()?;
        let mut i = 1;
        for v in items.sequence_values::<Value>().flatten() {
            let res: Value = func.call(v)?;
            result_table.set(i, res)?;
            i += 1;
        }
        Ok(result_table)
    })?;
    parallel.set("map", map_fn)?;

    // parallel.filter(items, fn) - Filter items using predicate function
    let filter_fn = lua.create_function(|lua, (items, func): (Table, Function)| {
        let result_table = lua.create_table()?;
        let mut i = 1;
        for v in items.sequence_values::<Value>().flatten() {
            let keep: bool = func.call(v.clone())?;
            if keep {
                result_table.set(i, v)?;
                i += 1;
            }
        }
        Ok(result_table)
    })?;
    parallel.set("filter", filter_fn)?;

    // parallel.reduce(items, initial, fn) - Reduce items to single value
    let reduce_fn =
        lua.create_function(|_, (items, initial, func): (Table, Value, Function)| {
            let mut acc = initial;
            for v in items.sequence_values::<Value>().flatten() {
                acc = func.call((acc, v))?;
            }
            Ok(acc)
        })?;
    parallel.set("reduce", reduce_fn)?;

    Ok(parallel)
}
