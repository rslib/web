//! Data loading module (rs.data)
//!
//! Data loading operations:
//! - rs.data.load_json, rs.data.load_yaml, rs.data.load_toml, rs.data.load_frontmatter (sequential)
//! - rs.data.par.load_json, rs.data.par.load_yaml, rs.data.par.load_frontmatter (parallel)

use super::helpers::{is_path_within_root, parse_frontmatter_content, resolve_path};
use crate::tracker::SharedTracker;
use mlua::{Lua, LuaSerdeExt, Result, Table, Value};
use rayon::prelude::*;
use std::path::Path;

/// Create the data module table
pub fn create_module(
    lua: &Lua,
    project_root: &Path,
    sandbox: bool,
    tracker: SharedTracker,
) -> Result<Table> {
    let data = lua.create_table()?;
    let root = project_root.to_path_buf();

    // ========================================================================
    // Sequential Operations
    // ========================================================================

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

        let canonical = resolved.canonicalize().unwrap_or(resolved);
        tracker_clone.record_read(canonical, content.as_bytes());

        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(v) => lua.to_value(&v),
            Err(_) => Ok(Value::Nil),
        }
    })?;
    data.set("load_json", load_json)?;

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

        let canonical = resolved.canonicalize().unwrap_or(resolved);
        tracker_clone.record_read(canonical, content.as_bytes());

        match serde_yaml::from_str::<serde_json::Value>(&content) {
            Ok(v) => lua.to_value(&v),
            Err(_) => Ok(Value::Nil),
        }
    })?;
    data.set("load_yaml", load_yaml)?;

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

        let canonical = resolved.canonicalize().unwrap_or(resolved);
        tracker_clone.record_read(canonical, content.as_bytes());

        match toml::from_str::<toml::Value>(&content) {
            Ok(v) => {
                // Convert TOML to JSON for consistent Lua representation
                let json_str = serde_json::to_string(&v).unwrap_or_default();
                match serde_json::from_str::<serde_json::Value>(&json_str) {
                    Ok(jv) => lua.to_value(&jv),
                    Err(_) => Ok(Value::Nil),
                }
            }
            Err(_) => Ok(Value::Nil),
        }
    })?;
    data.set("load_toml", load_toml)?;

    // from_json(string) - Parse JSON string to Lua value
    let from_json = lua.create_function(|lua, s: String| {
        match serde_json::from_str::<serde_json::Value>(&s) {
            Ok(v) => lua.to_value(&v),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "JSON parse error: {}",
                e
            ))),
        }
    })?;
    data.set("from_json", from_json)?;

    // to_json(value, pretty?) - Serialize Lua value to JSON string
    let to_json = lua.create_function(|lua, (value, pretty): (Value, Option<bool>)| {
        let json_value: serde_json::Value = lua.from_value(value)?;
        let result = if pretty.unwrap_or(false) {
            serde_json::to_string_pretty(&json_value)
        } else {
            serde_json::to_string(&json_value)
        };
        match result {
            Ok(s) => Ok(s),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "JSON serialize error: {}",
                e
            ))),
        }
    })?;
    data.set("to_json", to_json)?;

    // from_yaml(string) - Parse YAML string to Lua value
    let from_yaml = lua.create_function(|lua, s: String| {
        match serde_yaml::from_str::<serde_json::Value>(&s) {
            Ok(v) => lua.to_value(&v),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "YAML parse error: {}",
                e
            ))),
        }
    })?;
    data.set("from_yaml", from_yaml)?;

    // to_yaml(value) - Serialize Lua value to YAML string
    let to_yaml = lua.create_function(|lua, value: Value| {
        let json_value: serde_json::Value = lua.from_value(value)?;
        match serde_yaml::to_string(&json_value) {
            Ok(s) => Ok(s),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "YAML serialize error: {}",
                e
            ))),
        }
    })?;
    data.set("to_yaml", to_yaml)?;

    // from_toml(string) - Parse TOML string to Lua value
    let from_toml =
        lua.create_function(|lua, s: String| match toml::from_str::<toml::Value>(&s) {
            Ok(v) => {
                let json_str = serde_json::to_string(&v).unwrap_or_default();
                match serde_json::from_str::<serde_json::Value>(&json_str) {
                    Ok(jv) => lua.to_value(&jv),
                    Err(e) => Err(mlua::Error::RuntimeError(format!(
                        "TOML parse error: {}",
                        e
                    ))),
                }
            }
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "TOML parse error: {}",
                e
            ))),
        })?;
    data.set("from_toml", from_toml)?;

    // to_toml(value) - Serialize Lua value to TOML string
    let to_toml = lua.create_function(|lua, value: Value| {
        let json_value: serde_json::Value = lua.from_value(value)?;
        let toml_value: toml::Value = serde_json::from_value(json_value)
            .map_err(|e| mlua::Error::RuntimeError(format!("TOML conversion error: {}", e)))?;
        match toml::to_string_pretty(&toml_value) {
            Ok(s) => Ok(s),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "TOML serialize error: {}",
                e
            ))),
        }
    })?;
    data.set("to_toml", to_toml)?;

    // load_frontmatter(path) - Extract frontmatter and content from markdown file
    // Returns: { raw = "...", content = "...", ...frontmatter_fields } or nil
    // Frontmatter fields are merged to top level for convenience
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let load_frontmatter = lua.create_function(move |lua, path: String| {
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

        let canonical = resolved.canonicalize().unwrap_or(resolved);
        tracker_clone.record_read(canonical, raw.as_bytes());

        // Parse frontmatter (YAML between --- or TOML between +++)
        let (frontmatter, content) = parse_frontmatter_content(&raw);

        let result = lua.create_table()?;
        result.set("raw", raw)?;
        result.set("content", content)?;

        // Merge frontmatter fields to top level for convenience
        if let Some(fm) = frontmatter
            && let serde_json::Value::Object(map) = fm
        {
            for (key, value) in map {
                result.set(key, lua.to_value(&value)?)?;
            }
        }

        Ok(Value::Table(result))
    })?;
    data.set("load_frontmatter", load_frontmatter)?;

    // ========================================================================
    // Parallel Operations (rs.data.par.*)
    // ========================================================================

    let par = lua.create_table()?;

    // par.load_json(paths) - Load multiple JSON files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let par_load_json = lua.create_function(move |lua, paths: Table| {
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
                    let canonical = resolved.canonicalize().unwrap_or(resolved);
                    tracker_ref.record_read(canonical, content.as_bytes());
                    serde_json::from_str(&content).ok()
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
    par.set("load_json", par_load_json)?;

    // par.load_yaml(paths) - Load multiple YAML files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let par_load_yaml = lua.create_function(move |lua, paths: Table| {
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
                    let canonical = resolved.canonicalize().unwrap_or(resolved);
                    tracker_ref.record_read(canonical, content.as_bytes());
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
    par.set("load_yaml", par_load_yaml)?;

    // par.load_frontmatter(paths) - Parse frontmatter from multiple files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let par_load_frontmatter = lua.create_function(move |lua, paths: Table| {
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
                    let canonical = resolved.canonicalize().unwrap_or(resolved);
                    tracker_ref.record_read(canonical, raw.as_bytes());
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
                    // Merge frontmatter to top level
                    if let Some(serde_json::Value::Object(map)) = fm {
                        for (key, value) in map {
                            entry.set(key.clone(), lua.to_value(&value)?)?;
                        }
                    }
                    result_table.set(i + 1, entry)?;
                }
                None => result_table.set(i + 1, Value::Nil)?,
            }
        }
        Ok(result_table)
    })?;
    par.set("load_frontmatter", par_load_frontmatter)?;

    data.set("par", par)?;

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lua::assets::create_manifest;
    use crate::tracker::BuildTracker;
    use std::sync::Arc;

    fn setup_lua() -> Lua {
        let lua = Lua::new();
        let root = std::env::current_dir().unwrap();
        let tracker = Arc::new(BuildTracker::disabled());
        crate::lua::register(&lua, &root, false, tracker, None, create_manifest())
            .expect("Failed to register Lua functions");
        lua
    }

    #[test]
    fn test_from_json() {
        let lua = setup_lua();
        let result: i64 = lua
            .load(r#"local rs = require("rs-web"); return rs.data.from_json('{"x": 42}').x"#)
            .call(())
            .expect("from_json failed");
        assert_eq!(result, 42);
    }

    #[test]
    fn test_from_json_array() {
        let lua = setup_lua();
        let result: i64 = lua
            .load(r#"local rs = require("rs-web"); return rs.data.from_json('[1, 2, 3]')[2]"#)
            .call(())
            .expect("from_json array failed");
        assert_eq!(result, 2);
    }

    #[test]
    fn test_to_json() {
        let lua = setup_lua();
        let result: String = lua
            .load(r#"local rs = require("rs-web"); return rs.data.to_json({ x = 42 })"#)
            .call(())
            .expect("to_json failed");
        assert!(result.contains("\"x\""));
        assert!(result.contains("42"));
    }

    #[test]
    fn test_to_json_pretty() {
        let lua = setup_lua();
        let result: String = lua
            .load(r#"local rs = require("rs-web"); return rs.data.to_json({ x = 42 }, true)"#)
            .call(())
            .expect("to_json pretty failed");
        assert!(result.contains('\n'));
    }

    #[test]
    fn test_json_roundtrip() {
        let lua = setup_lua();
        let result: bool = lua
            .load(
                r#"
                local rs = require("rs-web")
                local original = { name = "test", count = 123, active = true }
                local json_str = rs.data.to_json(original)
                local parsed = rs.data.from_json(json_str)
                return parsed.name == "test" and parsed.count == 123 and parsed.active == true
            "#,
            )
            .call(())
            .expect("JSON roundtrip failed");
        assert!(result);
    }

    #[test]
    fn test_from_yaml() {
        let lua = setup_lua();
        let result: i64 = lua
            .load(r#"local rs = require("rs-web"); return rs.data.from_yaml("x: 42").x"#)
            .call(())
            .expect("from_yaml failed");
        assert_eq!(result, 42);
    }

    #[test]
    fn test_to_yaml() {
        let lua = setup_lua();
        let result: String = lua
            .load(r#"local rs = require("rs-web"); return rs.data.to_yaml({ x = 42 })"#)
            .call(())
            .expect("to_yaml failed");
        assert!(result.contains("x:"));
        assert!(result.contains("42"));
    }

    #[test]
    fn test_yaml_roundtrip() {
        let lua = setup_lua();
        let result: bool = lua
            .load(
                r#"
                local rs = require("rs-web")
                local original = { name = "test", count = 123 }
                local yaml_str = rs.data.to_yaml(original)
                local parsed = rs.data.from_yaml(yaml_str)
                return parsed.name == "test" and parsed.count == 123
            "#,
            )
            .call(())
            .expect("YAML roundtrip failed");
        assert!(result);
    }

    #[test]
    fn test_from_toml() {
        let lua = setup_lua();
        let result: i64 = lua
            .load(r#"local rs = require("rs-web"); return rs.data.from_toml("x = 42").x"#)
            .call(())
            .expect("from_toml failed");
        assert_eq!(result, 42);
    }

    #[test]
    fn test_to_toml() {
        let lua = setup_lua();
        let result: String = lua
            .load(r#"local rs = require("rs-web"); return rs.data.to_toml({ x = 42 })"#)
            .call(())
            .expect("to_toml failed");
        assert!(result.contains("x = 42"));
    }

    #[test]
    fn test_toml_roundtrip() {
        let lua = setup_lua();
        let result: bool = lua
            .load(
                r#"
                local rs = require("rs-web")
                local original = { name = "test", count = 123 }
                local toml_str = rs.data.to_toml(original)
                local parsed = rs.data.from_toml(toml_str)
                return parsed.name == "test" and parsed.count == 123
            "#,
            )
            .call(())
            .expect("TOML roundtrip failed");
        assert!(result);
    }

    #[test]
    fn test_from_json_error() {
        let lua = setup_lua();
        let result = lua
            .load(r#"local rs = require("rs-web"); return rs.data.from_json("invalid json")"#)
            .call::<Value>(());
        assert!(result.is_err());
    }

    #[test]
    fn test_from_yaml_error() {
        let lua = setup_lua();
        let result = lua
            .load(r#"local rs = require("rs-web"); return rs.data.from_yaml("invalid: yaml: : :")"#)
            .call::<Value>(());
        assert!(result.is_err());
    }

    #[test]
    fn test_from_toml_error() {
        let lua = setup_lua();
        let result = lua
            .load(r#"local rs = require("rs-web"); return rs.data.from_toml("invalid = = toml")"#)
            .call::<Value>(());
        assert!(result.is_err());
    }
}
