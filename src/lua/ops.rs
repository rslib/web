//! Collection operations module (rs.ops)
//!
//! Sequential and parallel collection operations:
//! - rs.ops.map, rs.ops.filter, rs.ops.sort, etc. (sequential)
//! - rs.ops.par.map, rs.ops.par.filter (parallel with context)

use super::portable::LuaPortable;
use mlua::{Function, Lua, Result, Table, Value};
use rayon::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

// Thread-local Lua VM for parallel execution
thread_local! {
    static WORKER_LUA: RefCell<Option<Lua>> = const { RefCell::new(None) };
}

/// Get or initialize the thread-local Lua VM
fn get_or_init_worker_lua() -> &'static Lua {
    WORKER_LUA.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if borrow.is_none() {
            let lua = Lua::new();
            lua.load_std_libs(mlua::StdLib::ALL_SAFE).ok();
            *borrow = Some(lua);
        }
        // SAFETY: The Lua VM is thread-local and lives for the duration of the thread
        unsafe { &*(borrow.as_ref().unwrap() as *const Lua) }
    })
}

/// Create the ops module table
pub fn create_module(lua: &Lua) -> Result<Table> {
    let ops = lua.create_table()?;

    // ========================================================================
    // Sequential Operations
    // ========================================================================

    // map(items, fn) - Transform each item
    let map_fn = lua.create_function(|lua, (items, func): (Table, Function)| {
        let result = lua.create_table()?;
        let mut i = 1;
        for v in items.sequence_values::<Value>().flatten() {
            let transformed: Value = func.call(v)?;
            result.set(i, transformed)?;
            i += 1;
        }
        Ok(result)
    })?;
    ops.set("map", map_fn)?;

    // filter(items, fn) - Filter items where fn returns true
    let filter_fn = lua.create_function(|lua, (items, func): (Table, Function)| {
        let result = lua.create_table()?;
        let mut i = 1;
        for v in items.sequence_values::<Value>().flatten() {
            let keep: bool = func.call(v.clone())?;
            if keep {
                result.set(i, v)?;
                i += 1;
            }
        }
        Ok(result)
    })?;
    ops.set("filter", filter_fn)?;

    // sort(items, fn) - Sort items using comparator (returns true if a < b)
    let sort_fn = lua.create_function(|lua, (items, func): (Table, Function)| {
        let mut vec: Vec<Value> = items.sequence_values::<Value>().flatten().collect();

        vec.sort_by(|a, b| {
            let result: bool = func.call((a.clone(), b.clone())).unwrap_or(false);
            if result {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });

        let result = lua.create_table()?;
        for (i, v) in vec.into_iter().enumerate() {
            result.set(i + 1, v)?;
        }
        Ok(result)
    })?;
    ops.set("sort", sort_fn)?;

    // find(items, fn) - Find first item where fn returns true
    let find_fn = lua.create_function(|_, (items, func): (Table, Function)| {
        for v in items.sequence_values::<Value>().flatten() {
            let matches: bool = func.call(v.clone())?;
            if matches {
                return Ok(v);
            }
        }
        Ok(Value::Nil)
    })?;
    ops.set("find", find_fn)?;

    // group_by(items, key_fn) - Group items by key returned by key_fn
    let group_by_fn = lua.create_function(|lua, (items, func): (Table, Function)| {
        let mut groups: HashMap<String, Vec<Value>> = HashMap::new();

        for v in items.sequence_values::<Value>().flatten() {
            let key: String = func.call(v.clone())?;
            groups.entry(key).or_default().push(v);
        }

        let result = lua.create_table()?;
        for (key, values) in groups {
            let group_table = lua.create_table()?;
            for (i, v) in values.into_iter().enumerate() {
                group_table.set(i + 1, v)?;
            }
            result.set(key, group_table)?;
        }
        Ok(result)
    })?;
    ops.set("group_by", group_by_fn)?;

    // unique(items) - Remove duplicates (by string representation)
    let unique_fn = lua.create_function(|lua, items: Table| {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let result = lua.create_table()?;
        let mut i = 1;

        for v in items.sequence_values::<Value>().flatten() {
            let key = format!("{:?}", v);
            if seen.insert(key) {
                result.set(i, v)?;
                i += 1;
            }
        }
        Ok(result)
    })?;
    ops.set("unique", unique_fn)?;

    // reverse(items) - Reverse array order
    let reverse_fn = lua.create_function(|lua, items: Table| {
        let vec: Vec<Value> = items.sequence_values::<Value>().flatten().collect();
        let result = lua.create_table()?;
        for (i, v) in vec.into_iter().rev().enumerate() {
            result.set(i + 1, v)?;
        }
        Ok(result)
    })?;
    ops.set("reverse", reverse_fn)?;

    // take(items, n) - Take first n items
    let take_fn = lua.create_function(|lua, (items, n): (Table, usize)| {
        let result = lua.create_table()?;
        for (i, v) in items
            .sequence_values::<Value>()
            .flatten()
            .take(n)
            .enumerate()
        {
            result.set(i + 1, v)?;
        }
        Ok(result)
    })?;
    ops.set("take", take_fn)?;

    // skip(items, n) - Skip first n items
    let skip_fn = lua.create_function(|lua, (items, n): (Table, usize)| {
        let result = lua.create_table()?;
        for (i, v) in items
            .sequence_values::<Value>()
            .flatten()
            .skip(n)
            .enumerate()
        {
            result.set(i + 1, v)?;
        }
        Ok(result)
    })?;
    ops.set("skip", skip_fn)?;

    // keys(table) - Get all keys from a table
    let keys_fn = lua.create_function(|lua, table: Table| {
        let result = lua.create_table()?;
        let mut i = 1;
        for (k, _) in table.pairs::<Value, Value>().flatten() {
            result.set(i, k)?;
            i += 1;
        }
        Ok(result)
    })?;
    ops.set("keys", keys_fn)?;

    // values(table) - Get all values from a table
    let values_fn = lua.create_function(|lua, table: Table| {
        let result = lua.create_table()?;
        let mut i = 1;
        for (_, v) in table.pairs::<Value, Value>().flatten() {
            result.set(i, v)?;
            i += 1;
        }
        Ok(result)
    })?;
    ops.set("values", values_fn)?;

    // reduce(items, initial, fn) - Reduce items to single value
    let reduce_fn =
        lua.create_function(|_, (items, initial, func): (Table, Value, Function)| {
            let mut acc = initial;
            for v in items.sequence_values::<Value>().flatten() {
                acc = func.call((acc, v))?;
            }
            Ok(acc)
        })?;
    ops.set("reduce", reduce_fn)?;

    // ========================================================================
    // Parallel Operations (rs.ops.par.*)
    // ========================================================================

    let par = lua.create_table()?;

    // par.map(items, fn, ctx?) - Parallel map with optional context
    let par_map_fn = lua.create_function(
        |lua, (items, func, ctx): (Table, Function, Option<Table>)| {
            let portable_func = LuaPortable::from_lua(&Value::Function(func), lua)?;

            let portable_ctx = match ctx {
                Some(t) => Some(LuaPortable::from_lua(&Value::Table(t), lua)?),
                None => None,
            };

            let portable_items: Vec<LuaPortable> = items
                .sequence_values::<Value>()
                .filter_map(|v| v.ok())
                .map(|v| LuaPortable::from_lua(&v, lua))
                .collect::<Result<Vec<_>>>()?;

            let results: Vec<std::result::Result<LuaPortable, String>> = portable_items
                .par_iter()
                .map(|item| {
                    let worker_lua = get_or_init_worker_lua();

                    let func_value = portable_func
                        .to_lua(worker_lua)
                        .map_err(|e| format!("Failed to load function: {}", e))?;
                    let func = func_value
                        .as_function()
                        .ok_or_else(|| "Expected function".to_string())?;

                    let lua_item = item
                        .to_lua(worker_lua)
                        .map_err(|e| format!("Failed to convert item: {}", e))?;

                    let lua_ctx = match &portable_ctx {
                        Some(ctx) => ctx
                            .to_lua(worker_lua)
                            .map_err(|e| format!("Failed to convert context: {}", e))?,
                        None => Value::Nil,
                    };

                    let result: Value = func
                        .call((lua_item, lua_ctx))
                        .map_err(|e| format!("Function call failed: {}", e))?;

                    LuaPortable::from_lua(&result, worker_lua)
                        .map_err(|e| format!("Failed to serialize result: {}", e))
                })
                .collect();

            let result_table = lua.create_table()?;
            for (i, result) in results.into_iter().enumerate() {
                match result {
                    Ok(portable) => {
                        let value = portable.to_lua(lua)?;
                        result_table.set(i + 1, value)?;
                    }
                    Err(e) => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "ops.par.map failed at index {}: {}",
                            i + 1,
                            e
                        )));
                    }
                }
            }
            Ok(result_table)
        },
    )?;
    par.set("map", par_map_fn)?;

    // par.filter(items, fn, ctx?) - Parallel filter with optional context
    let par_filter_fn = lua.create_function(
        |lua, (items, func, ctx): (Table, Function, Option<Table>)| {
            let portable_func = LuaPortable::from_lua(&Value::Function(func), lua)?;

            let portable_ctx = match ctx {
                Some(t) => Some(LuaPortable::from_lua(&Value::Table(t), lua)?),
                None => None,
            };

            let portable_items: Vec<(usize, LuaPortable)> = items
                .sequence_values::<Value>()
                .enumerate()
                .filter_map(|(i, v)| v.ok().map(|v| (i, v)))
                .map(|(i, v)| LuaPortable::from_lua(&v, lua).map(|p| (i, p)))
                .collect::<Result<Vec<_>>>()?;

            let results: Vec<std::result::Result<Option<LuaPortable>, String>> = portable_items
                .par_iter()
                .map(|(_, item)| {
                    let worker_lua = get_or_init_worker_lua();

                    let func_value = portable_func
                        .to_lua(worker_lua)
                        .map_err(|e| format!("Failed to load function: {}", e))?;
                    let func = func_value
                        .as_function()
                        .ok_or_else(|| "Expected function".to_string())?;

                    let lua_item = item
                        .to_lua(worker_lua)
                        .map_err(|e| format!("Failed to convert item: {}", e))?;

                    let lua_ctx = match &portable_ctx {
                        Some(ctx) => ctx
                            .to_lua(worker_lua)
                            .map_err(|e| format!("Failed to convert context: {}", e))?,
                        None => Value::Nil,
                    };

                    let keep: bool = func
                        .call((lua_item, lua_ctx))
                        .map_err(|e| format!("Filter predicate failed: {}", e))?;

                    if keep {
                        Ok(Some(item.clone()))
                    } else {
                        Ok(None)
                    }
                })
                .collect();

            let result_table = lua.create_table()?;
            let mut i = 1;
            for result in results.into_iter() {
                match result {
                    Ok(Some(portable)) => {
                        let value = portable.to_lua(lua)?;
                        result_table.set(i, value)?;
                        i += 1;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "ops.par.filter failed: {}",
                            e
                        )));
                    }
                }
            }
            Ok(result_table)
        },
    )?;
    par.set("filter", par_filter_fn)?;

    ops.set("par", par)?;

    Ok(ops)
}
