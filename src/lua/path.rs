//! Path module (rs.path)

use mlua::{Lua, Result, Table};
use std::path::Path;

pub fn create_module(lua: &Lua) -> Result<Table> {
    let path_mod = lua.create_table()?;

    // join(...) - Join path segments
    let join_fn = lua.create_function(|_, parts: mlua::Variadic<String>| {
        let mut path = std::path::PathBuf::new();
        for part in parts {
            path.push(part);
        }
        Ok(path.to_string_lossy().to_string())
    })?;
    path_mod.set("join", join_fn)?;

    // basename(path) - Get file name from path
    let basename_fn = lua.create_function(|_, path: String| {
        Ok(Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string())
    })?;
    path_mod.set("basename", basename_fn)?;

    // dirname(path) - Get directory from path
    let dirname_fn = lua.create_function(|_, path: String| {
        Ok(Path::new(&path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string())
    })?;
    path_mod.set("dirname", dirname_fn)?;

    // extension(path) - Get file extension
    let extension_fn = lua.create_function(|_, path: String| {
        Ok(Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string())
    })?;
    path_mod.set("extension", extension_fn)?;

    Ok(path_mod)
}
