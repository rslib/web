//! Environment module (rs.env)

use mlua::{Lua, Result, Table, Value};

pub fn create_module(lua: &Lua) -> Result<Table> {
    let env = lua.create_table()?;

    // get(name, default?) - Get environment variable with optional default
    let get_fn =
        lua.create_function(
            |lua, (name, default): (String, Option<String>)| match std::env::var(&name) {
                Ok(val) => Ok(Value::String(lua.create_string(&val)?)),
                Err(_) => match default {
                    Some(d) => Ok(Value::String(lua.create_string(&d)?)),
                    None => Ok(Value::Nil),
                },
            },
        )?;
    env.set("get", get_fn)?;

    Ok(env)
}
