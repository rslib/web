//! Environment module (rs.env)

use mlua::{Lua, Result, Table, Value};

pub fn create_module(lua: &Lua) -> Result<Table> {
    let env = lua.create_table()?;

    // get(name) - Get environment variable
    let get_fn = lua.create_function(|lua, name: String| match std::env::var(&name) {
        Ok(val) => Ok(Value::String(lua.create_string(&val)?)),
        Err(_) => Ok(Value::Nil),
    })?;
    env.set("get", get_fn)?;

    Ok(env)
}
