//! Environment functions for Lua API
//!
//! Functions: env, print, is_gitignored

use mlua::{Lua, Result, Table, Value};
use std::path::{Path, PathBuf};

/// Register environment functions on the module table
pub fn register(lua: &Lua, module: &Table, project_root: &Path) -> Result<()> {
    let root = project_root.to_path_buf();

    // env(name) - Get environment variable
    let env_fn = lua.create_function(|lua, name: String| match std::env::var(&name) {
        Ok(val) => Ok(Value::String(lua.create_string(&val)?)),
        Err(_) => Ok(Value::Nil),
    })?;
    module.set("env", env_fn)?;

    // print(...) - Log to build output
    let print_fn = lua.create_function(|_, args: mlua::Variadic<String>| {
        let msg = args
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\t");
        log::info!("[Lua] {}", msg);
        Ok(())
    })?;
    module.set("print", print_fn)?;

    // is_gitignored(path) - Check if path is ignored by .gitignore
    let root_clone = root.clone();
    let is_gitignored_fn = lua.create_function(move |_, path: String| {
        let full_path = if Path::new(&path).is_absolute() {
            PathBuf::from(&path)
        } else {
            root_clone.join(&path)
        };

        // Build gitignore matcher from root
        let mut builder = ignore::gitignore::GitignoreBuilder::new(&root_clone);
        let gitignore_path = root_clone.join(".gitignore");
        if gitignore_path.exists() {
            let _ = builder.add(&gitignore_path);
        }

        match builder.build() {
            Ok(gitignore) => {
                let is_dir = full_path.is_dir();
                Ok(gitignore.matched(&full_path, is_dir).is_ignore())
            }
            Err(_) => Ok(false),
        }
    })?;
    module.set("is_gitignored", is_gitignored_fn)?;

    Ok(())
}
