//! Hash module (rs.hash)

use mlua::{Lua, Result, Table, Value};

pub fn create_module(lua: &Lua) -> Result<Table> {
    let hash = lua.create_table()?;

    // content(str) - Hash string content (xxHash64)
    let content_fn = lua.create_function(|_, content: String| {
        use std::hash::{Hash, Hasher};
        let mut hasher = twox_hash::XxHash64::with_seed(0);
        content.hash(&mut hasher);
        Ok(format!("{:x}", hasher.finish()))
    })?;
    hash.set("content", content_fn)?;

    // file(path) - Hash file contents
    let file_fn = lua.create_function(|lua, path: String| {
        use std::hash::Hasher;
        match std::fs::read(&path) {
            Ok(content) => {
                let mut hasher = twox_hash::XxHash64::with_seed(0);
                hasher.write(&content);
                let hash_str = format!("{:x}", hasher.finish());
                Ok(Value::String(lua.create_string(&hash_str)?))
            }
            Err(_) => Ok(Value::Nil),
        }
    })?;
    hash.set("file", file_fn)?;

    Ok(hash)
}
