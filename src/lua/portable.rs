//! Portable Lua values for cross-thread communication
//!
//! This module provides `LuaPortable`, a serializable representation of Lua values
//! that can be safely sent across thread boundaries for parallel execution.
//!
//! ## Limitations
//!
//! - Functions are serialized as bytecode only (upvalues are NOT captured)
//! - To pass context to functions, use the explicit context parameter in parallel.map:
//!   ```lua
//!   local multiplier = 10
//!   rs.parallel.map(items, function(x, ctx)
//!       return x * ctx.multiplier
//!   end, {multiplier = multiplier})
//!   ```

use mlua::{Function, Lua, Result, Value};
use std::collections::HashSet;

/// A Lua value that can be sent across threads
#[derive(Clone, Debug)]
pub enum LuaPortable {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Vec<u8>),
    Table(Vec<(LuaPortable, LuaPortable)>),
    /// Function bytecode (upvalues are NOT preserved - use context parameter instead)
    Function(Vec<u8>),
}

/// Error context for serialization failures
#[derive(Debug)]
pub struct PortableError {
    pub message: String,
    pub path: Vec<String>,
}

impl PortableError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            path: Vec::new(),
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.path.insert(0, context.into());
        self
    }

    pub fn format_message(&self) -> String {
        if self.path.is_empty() {
            self.message.clone()
        } else {
            format!("at {}: {}", self.path.join("."), self.message)
        }
    }
}

impl std::fmt::Display for PortableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_message())
    }
}

impl From<PortableError> for mlua::Error {
    fn from(err: PortableError) -> Self {
        mlua::Error::RuntimeError(err.format_message())
    }
}

/// Context for tracking circular references during serialization
pub struct SerializeContext {
    /// Set of table pointer addresses we've seen
    seen_tables: HashSet<usize>,
    /// Set of function pointer addresses we've seen
    seen_functions: HashSet<usize>,
    /// Current path for error messages
    path: Vec<String>,
    /// Maximum recursion depth
    max_depth: usize,
    /// Current depth
    current_depth: usize,
}

impl SerializeContext {
    pub fn new() -> Self {
        Self {
            seen_tables: HashSet::new(),
            seen_functions: HashSet::new(),
            path: Vec::new(),
            max_depth: 100,
            current_depth: 0,
        }
    }

    fn push_path(&mut self, segment: impl Into<String>) {
        self.path.push(segment.into());
        self.current_depth += 1;
    }

    fn pop_path(&mut self) {
        self.path.pop();
        self.current_depth = self.current_depth.saturating_sub(1);
    }

    fn check_depth(&self) -> std::result::Result<(), PortableError> {
        if self.current_depth > self.max_depth {
            Err(PortableError {
                message: format!(
                    "Maximum recursion depth ({}) exceeded. \
                    This may indicate circular references or deeply nested structures.",
                    self.max_depth
                ),
                path: self.path.clone(),
            })
        } else {
            Ok(())
        }
    }

    fn check_table_cycle(&mut self, ptr: usize) -> std::result::Result<(), PortableError> {
        if !self.seen_tables.insert(ptr) {
            Err(PortableError {
                message: "Circular reference detected in table. \
                    Tables that reference themselves cannot be passed to parallel functions."
                    .to_string(),
                path: self.path.clone(),
            })
        } else {
            Ok(())
        }
    }

    fn remove_table(&mut self, ptr: usize) {
        self.seen_tables.remove(&ptr);
    }

    fn check_function_cycle(&mut self, ptr: usize) -> std::result::Result<(), PortableError> {
        if !self.seen_functions.insert(ptr) {
            Err(PortableError {
                message: "Circular reference detected in function. \
                    Functions that reference themselves cannot be passed to parallel functions."
                    .to_string(),
                path: self.path.clone(),
            })
        } else {
            Ok(())
        }
    }

    fn remove_function(&mut self, ptr: usize) {
        self.seen_functions.remove(&ptr);
    }

    fn current_path(&self) -> String {
        if self.path.is_empty() {
            "<root>".to_string()
        } else {
            self.path.join(".")
        }
    }
}

impl Default for SerializeContext {
    fn default() -> Self {
        Self::new()
    }
}

impl LuaPortable {
    /// Convert a Lua value to a portable representation
    pub fn from_lua(value: &Value, lua: &Lua) -> Result<Self> {
        let mut ctx = SerializeContext::new();
        Self::from_lua_with_context(value, lua, &mut ctx).map_err(|e| e.into())
    }

    /// Convert with explicit context (for nested calls)
    #[allow(clippy::only_used_in_recursion)]
    fn from_lua_with_context(
        value: &Value,
        lua: &Lua,
        ctx: &mut SerializeContext,
    ) -> std::result::Result<Self, PortableError> {
        ctx.check_depth()?;

        match value {
            Value::Nil => Ok(LuaPortable::Nil),
            Value::Boolean(b) => Ok(LuaPortable::Bool(*b)),
            Value::Integer(i) => Ok(LuaPortable::Int(*i)),
            Value::Number(n) => Ok(LuaPortable::Float(*n)),
            Value::String(s) => Ok(LuaPortable::String(s.as_bytes().to_vec())),

            Value::Table(t) => {
                // Get table pointer for cycle detection
                let ptr = t.to_pointer() as usize;
                ctx.check_table_cycle(ptr)?;

                let mut entries = Vec::new();
                for pair in t.pairs::<Value, Value>() {
                    let (k, v) = pair.map_err(|e| PortableError::new(e.to_string()))?;

                    // Create path context for key
                    let key_repr = value_short_repr(&k);
                    ctx.push_path(format!("[{}]", key_repr));

                    let portable_key = Self::from_lua_with_context(&k, lua, ctx)
                        .map_err(|e| e.with_context(format!("in table key [{}]", key_repr)))?;

                    let portable_val = Self::from_lua_with_context(&v, lua, ctx)
                        .map_err(|e| e.with_context(format!("in table value at [{}]", key_repr)))?;

                    ctx.pop_path();
                    entries.push((portable_key, portable_val));
                }

                ctx.remove_table(ptr);
                Ok(LuaPortable::Table(entries))
            }

            Value::Function(f) => {
                let ptr = f.to_pointer() as usize;
                ctx.check_function_cycle(ptr)?;

                // Dump function to bytecode
                // Note: Upvalues are NOT captured. Use the context parameter in parallel.map instead.
                let bytecode = f.dump(true);

                ctx.remove_function(ptr);
                Ok(LuaPortable::Function(bytecode))
            }

            Value::UserData(ud) => {
                // Try to get type name from metatable for better error message
                let type_name = ud
                    .metatable()
                    .ok()
                    .and_then(|mt| mt.get::<String>("__name").ok())
                    .unwrap_or_else(|| "unknown".to_string());

                Err(PortableError::new(format!(
                    "UserData of type '{}' cannot be passed to parallel functions. \
                    UserData represents Rust objects that cannot be safely shared across threads. \
                    Consider extracting the data you need into a plain Lua table first.",
                    type_name
                )))
            }

            Value::Thread(_) => Err(PortableError::new(
                "Lua threads (coroutines) cannot be passed to parallel functions. \
                Coroutines have execution state that cannot be serialized.",
            )),

            Value::LightUserData(_) => Err(PortableError::new(
                "Light userdata (raw pointers) cannot be passed to parallel functions. \
                Pointers are not valid across thread boundaries.",
            )),

            Value::Error(e) => Err(PortableError::new(format!(
                "Lua error values cannot be passed to parallel functions: {}",
                e
            ))),

            _ => Err(PortableError::new(format!(
                "Unsupported Lua value type at {}. \
                Only nil, boolean, number, string, table, and function values are supported.",
                ctx.current_path()
            ))),
        }
    }

    /// Convert back to a Lua value
    pub fn to_lua(&self, lua: &Lua) -> Result<Value> {
        match self {
            LuaPortable::Nil => Ok(Value::Nil),
            LuaPortable::Bool(b) => Ok(Value::Boolean(*b)),
            LuaPortable::Int(i) => Ok(Value::Integer(*i)),
            LuaPortable::Float(n) => Ok(Value::Number(*n)),
            LuaPortable::String(s) => Ok(Value::String(lua.create_string(s)?)),

            LuaPortable::Table(entries) => {
                let t = lua.create_table()?;
                for (k, v) in entries {
                    t.set(k.to_lua(lua)?, v.to_lua(lua)?)?;
                }
                Ok(Value::Table(t))
            }

            LuaPortable::Function(bytecode) => {
                let func: Function = lua.load(bytecode).into_function()?;
                Ok(Value::Function(func))
            }
        }
    }

    /// Check if this portable value contains any functions
    #[allow(dead_code)]
    pub fn contains_function(&self) -> bool {
        match self {
            LuaPortable::Function(_) => true,
            LuaPortable::Table(entries) => entries
                .iter()
                .any(|(k, v)| k.contains_function() || v.contains_function()),
            _ => false,
        }
    }

    /// Get a human-readable type name
    #[allow(dead_code)]
    pub fn type_name(&self) -> &'static str {
        match self {
            LuaPortable::Nil => "nil",
            LuaPortable::Bool(_) => "boolean",
            LuaPortable::Int(_) => "integer",
            LuaPortable::Float(_) => "number",
            LuaPortable::String(_) => "string",
            LuaPortable::Table(_) => "table",
            LuaPortable::Function(_) => "function",
        }
    }
}

/// Get a short string representation of a Lua value for error messages
fn value_short_repr(value: &Value) -> String {
    match value {
        Value::Nil => "nil".to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => format!("{:.4}", n),
        Value::String(s) => {
            let bytes = s.as_bytes();
            let s_str = std::str::from_utf8(&bytes).unwrap_or("<invalid utf8>");
            if s_str.len() > 20 {
                format!("\"{}...\"", &s_str[..17])
            } else {
                format!("\"{}\"", s_str)
            }
        }
        Value::Table(_) => "<table>".to_string(),
        Value::Function(_) => "<function>".to_string(),
        Value::Thread(_) => "<thread>".to_string(),
        Value::UserData(_) => "<userdata>".to_string(),
        Value::LightUserData(_) => "<lightuserdata>".to_string(),
        Value::Error(e) => format!("<error: {}>", e),
        _ => "<unknown>".to_string(),
    }
}
