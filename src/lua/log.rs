//! Log module (rs.log)

use mlua::{Lua, Result, Table};

pub fn create_module(lua: &Lua) -> Result<Table> {
    let log_mod = lua.create_table()?;

    // trace(...) - Log at trace level
    let trace_fn = lua.create_function(|_, args: mlua::Variadic<String>| {
        let msg = args
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\t");
        log::trace!(target: "lua", "{}", msg);
        Ok(())
    })?;
    log_mod.set("trace", trace_fn)?;

    // debug(...) - Log at debug level
    let debug_fn = lua.create_function(|_, args: mlua::Variadic<String>| {
        let msg = args
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\t");
        log::debug!(target: "lua", "{}", msg);
        Ok(())
    })?;
    log_mod.set("debug", debug_fn)?;

    // info(...) - Log at info level
    let info_fn = lua.create_function(|_, args: mlua::Variadic<String>| {
        let msg = args
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\t");
        log::info!(target: "lua", "{}", msg);
        Ok(())
    })?;
    log_mod.set("info", info_fn)?;

    // warn(...) - Log at warn level
    let warn_fn = lua.create_function(|_, args: mlua::Variadic<String>| {
        let msg = args
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\t");
        log::warn!(target: "lua", "{}", msg);
        Ok(())
    })?;
    log_mod.set("warn", warn_fn)?;

    // error(...) - Log at error level
    let error_fn = lua.create_function(|_, args: mlua::Variadic<String>| {
        let msg = args
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\t");
        log::error!(target: "lua", "{}", msg);
        Ok(())
    })?;
    log_mod.set("error", error_fn)?;

    // print(...) - Always visible (uses special target)
    let print_fn = lua.create_function(|_, args: mlua::Variadic<String>| {
        let msg = args
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\t");
        log::info!(target: "lua_print", "{}", msg);
        Ok(())
    })?;
    log_mod.set("print", print_fn)?;

    Ok(log_mod)
}
