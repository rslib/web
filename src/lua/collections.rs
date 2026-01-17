//! Collection operations - filter, sort, map, find, group_by, etc.

use mlua::{Function, Lua, Result, Table, Value};
use std::collections::HashMap;

/// Register collection operation functions on the module table
pub fn register(lua: &Lua, module: &Table) -> Result<()> {
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
    module.set("filter", filter_fn)?;

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
    module.set("sort", sort_fn)?;

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
    module.set("map", map_fn)?;

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
    module.set("find", find_fn)?;

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
    module.set("group_by", group_by_fn)?;

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
    module.set("unique", unique_fn)?;

    // reverse(items) - Reverse array order
    let reverse_fn = lua.create_function(|lua, items: Table| {
        let vec: Vec<Value> = items.sequence_values::<Value>().flatten().collect();
        let result = lua.create_table()?;
        for (i, v) in vec.into_iter().rev().enumerate() {
            result.set(i + 1, v)?;
        }
        Ok(result)
    })?;
    module.set("reverse", reverse_fn)?;

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
    module.set("take", take_fn)?;

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
    module.set("skip", skip_fn)?;

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
    module.set("keys", keys_fn)?;

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
    module.set("values", values_fn)?;

    Ok(())
}
