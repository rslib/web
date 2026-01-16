//! Async I/O functions for Lua API (tokio-backed)
//!
//! HTTP Functions: async.fetch, async.fetch_json, async.fetch_all, async.spawn, async.await, async.await_all
//!
//! File Functions: async.read, async.read_file, async.read_files, async.write_file, async.load_json,
//!                 async.copy_file, async.rename, async.remove_file, async.remove_dir,
//!                 async.create_dir, async.exists, async.metadata, async.read_dir, async.canonicalize
//!
//! These functions use tokio for async I/O, allowing non-blocking
//! HTTP requests and file operations from Lua scripts.

use mlua::{Lua, LuaSerdeExt, Result, Table, UserData, UserDataMethods, Value};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

/// Global tokio runtime for async operations
fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime")
    })
}

/// Block on a future, handling the case where we're already in a runtime
pub fn block_on<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // We're in a runtime - use block_in_place to allow blocking
            tokio::task::block_in_place(|| handle.block_on(future))
        }
        Err(_) => {
            // Not in a runtime, use our global one directly
            runtime().block_on(future)
        }
    }
}

/// HTTP method enum
#[derive(Debug, Clone, Copy, Default)]
enum Method {
    #[default]
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
}

impl Method {
    fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "DELETE" => Method::Delete,
            "PATCH" => Method::Patch,
            "HEAD" => Method::Head,
            _ => Method::Get,
        }
    }
}

/// Fetch options
#[derive(Debug, Default)]
struct FetchOptions {
    method: Method,
    headers: HashMap<String, String>,
    body: Option<String>,
    timeout_secs: Option<u64>,
}

impl FetchOptions {
    fn from_lua_table(lua: &Lua, table: &Table) -> Result<Self> {
        let mut opts = FetchOptions::default();

        if let Ok(method) = table.get::<String>("method") {
            opts.method = Method::from_str(&method);
        }

        if let Ok(headers) = table.get::<Table>("headers") {
            for (k, v) in headers.pairs::<String, String>().flatten() {
                opts.headers.insert(k, v);
            }
        }

        if let Ok(body) = table.get::<String>("body") {
            opts.body = Some(body);
        } else if let Ok(body_table) = table.get::<Table>("body") {
            // JSON encode table body
            if let Ok(json_val) = lua.from_value::<serde_json::Value>(Value::Table(body_table)) {
                opts.body = Some(json_val.to_string());
                opts.headers
                    .entry("Content-Type".to_string())
                    .or_insert_with(|| "application/json".to_string());
            }
        }

        if let Ok(timeout) = table.get::<u64>("timeout") {
            opts.timeout_secs = Some(timeout);
        }

        Ok(opts)
    }
}

/// Result type for fetch operations
type FetchResult = std::result::Result<FetchResponse, String>;

/// Async task handle that can be awaited
/// Wraps a tokio JoinHandle for deferred execution
struct AsyncTask {
    handle: Arc<Mutex<Option<JoinHandle<FetchResult>>>>,
    result: Arc<Mutex<Option<FetchResult>>>,
    completed: Arc<Mutex<bool>>,
}

impl Clone for AsyncTask {
    fn clone(&self) -> Self {
        Self {
            handle: Arc::clone(&self.handle),
            result: Arc::clone(&self.result),
            completed: Arc::clone(&self.completed),
        }
    }
}

impl UserData for AsyncTask {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // Check if task is completed
        methods.add_method("is_completed", |_, this, _: ()| Ok(*this.completed.lock()));
    }
}

/// Response structure returned to Lua
#[derive(Clone)]
struct FetchResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: String,
    ok: bool,
}

impl FetchResponse {
    fn to_lua_table(&self, lua: &Lua) -> Result<Table> {
        let table = lua.create_table()?;
        table.set("status", self.status)?;
        table.set("body", self.body.clone())?;
        table.set("ok", self.ok)?;

        let headers_table = lua.create_table()?;
        for (k, v) in &self.headers {
            headers_table.set(k.clone(), v.clone())?;
        }
        table.set("headers", headers_table)?;

        // Add json() method to parse body as JSON
        let body_clone = self.body.clone();
        let json_fn = lua.create_function(move |lua, _: ()| {
            match serde_json::from_str::<serde_json::Value>(&body_clone) {
                Ok(v) => lua.to_value(&v),
                Err(e) => Err(mlua::Error::RuntimeError(format!(
                    "Failed to parse JSON: {}",
                    e
                ))),
            }
        })?;
        table.set("json", json_fn)?;

        Ok(table)
    }
}

/// Perform async fetch using reqwest
async fn do_fetch(url: &str, opts: &FetchOptions) -> FetchResult {
    let client = reqwest::Client::new();

    let mut builder = match opts.method {
        Method::Get => client.get(url),
        Method::Post => client.post(url),
        Method::Put => client.put(url),
        Method::Delete => client.delete(url),
        Method::Patch => client.patch(url),
        Method::Head => client.head(url),
    };

    // Add headers
    for (k, v) in &opts.headers {
        builder = builder.header(k, v);
    }

    // Add body
    if let Some(body) = &opts.body {
        builder = builder.body(body.clone());
    }

    // Set timeout
    if let Some(timeout) = opts.timeout_secs {
        builder = builder.timeout(std::time::Duration::from_secs(timeout));
    }

    let response = builder.send().await.map_err(|e| e.to_string())?;

    let status = response.status().as_u16();
    let ok = response.status().is_success();

    let mut headers = HashMap::new();
    for (k, v) in response.headers() {
        if let Ok(v_str) = v.to_str() {
            headers.insert(k.to_string(), v_str.to_string());
        }
    }

    let body = response.text().await.map_err(|e| e.to_string())?;

    Ok(FetchResponse {
        status,
        headers,
        body,
        ok,
    })
}

/// Perform async fetch returning raw bytes (for binary data like fonts, images)
async fn do_fetch_bytes(
    url: &str,
    opts: &FetchOptions,
) -> std::result::Result<(u16, bool, Vec<u8>), String> {
    let client = reqwest::Client::new();

    let mut builder = match opts.method {
        Method::Get => client.get(url),
        Method::Post => client.post(url),
        Method::Put => client.put(url),
        Method::Delete => client.delete(url),
        Method::Patch => client.patch(url),
        Method::Head => client.head(url),
    };

    for (k, v) in &opts.headers {
        builder = builder.header(k, v);
    }

    if let Some(body) = &opts.body {
        builder = builder.body(body.clone());
    }

    if let Some(timeout) = opts.timeout_secs {
        builder = builder.timeout(std::time::Duration::from_secs(timeout));
    }

    let response = builder.send().await.map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let ok = response.status().is_success();
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;

    Ok((status, ok, bytes.to_vec()))
}

use std::path::Path;

use super::helpers::{is_path_within_root, resolve_path};

/// Create the async module table
pub fn create_module(lua: &Lua, project_root: &Path, sandbox: bool) -> Result<Table> {
    let async_module = lua.create_table()?;
    let root = project_root.to_path_buf();

    // async.fetch(url, options?) - Fetch a URL
    let fetch = lua.create_function(|lua, args: (String, Option<Table>)| {
        let (url, opts_table) = args;

        let opts = if let Some(ref t) = opts_table {
            FetchOptions::from_lua_table(lua, t)?
        } else {
            FetchOptions::default()
        };

        let result = block_on(do_fetch(&url, &opts));

        match result {
            Ok(response) => {
                let table = response.to_lua_table(lua)?;
                Ok(Value::Table(table))
            }
            Err(e) => Err(mlua::Error::RuntimeError(format!("Fetch failed: {}", e))),
        }
    })?;
    async_module.set("fetch", fetch)?;

    // async.fetch_json(url, options?) - Fetch and parse as JSON
    let fetch_json = lua.create_function(|lua, args: (String, Option<Table>)| {
        let (url, opts_table) = args;

        let mut opts = if let Some(ref t) = opts_table {
            FetchOptions::from_lua_table(lua, t)?
        } else {
            FetchOptions::default()
        };

        // Add Accept header for JSON
        opts.headers
            .entry("Accept".to_string())
            .or_insert_with(|| "application/json".to_string());

        let result = block_on(do_fetch(&url, &opts));

        match result {
            Ok(response) => {
                if !response.ok {
                    return Err(mlua::Error::RuntimeError(format!(
                        "HTTP {} for {}",
                        response.status, url
                    )));
                }
                match serde_json::from_str::<serde_json::Value>(&response.body) {
                    Ok(v) => lua.to_value(&v),
                    Err(e) => Err(mlua::Error::RuntimeError(format!(
                        "Failed to parse JSON from {}: {}",
                        url, e
                    ))),
                }
            }
            Err(e) => Err(mlua::Error::RuntimeError(format!("Fetch failed: {}", e))),
        }
    })?;
    async_module.set("fetch_json", fetch_json)?;

    // async.fetch_bytes(url, options?) - Fetch binary data (for fonts, images, etc.)
    let fetch_bytes = lua.create_function(|lua, args: (String, Option<Table>)| {
        let (url, opts_table) = args;

        let opts = if let Some(ref t) = opts_table {
            FetchOptions::from_lua_table(lua, t)?
        } else {
            FetchOptions::default()
        };

        let result = block_on(do_fetch_bytes(&url, &opts));

        match result {
            Ok((status, ok, bytes)) => {
                let table = lua.create_table()?;
                table.set("status", status)?;
                table.set("ok", ok)?;
                table.set("body", lua.create_string(&bytes)?)?;
                Ok(Value::Table(table))
            }
            Err(e) => Err(mlua::Error::RuntimeError(format!("Fetch failed: {}", e))),
        }
    })?;
    async_module.set("fetch_bytes", fetch_bytes)?;

    // async.fetch_all(requests) - Fetch multiple URLs concurrently
    // requests is a table of {url, options?} or just strings
    let fetch_all = lua.create_function(|lua, requests: Table| {
        let mut urls_and_opts: Vec<(String, FetchOptions)> = Vec::new();

        for pair in requests.pairs::<Value, Value>() {
            let (_, v) = pair?;
            match v {
                Value::String(s) => {
                    urls_and_opts.push((s.to_str()?.to_string(), FetchOptions::default()));
                }
                Value::Table(t) => {
                    let url: String = t.get("url").or_else(|_| t.get(1))?;
                    let opts = if let Ok(opts_table) = t.get::<Table>("options") {
                        FetchOptions::from_lua_table(lua, &opts_table)?
                    } else {
                        FetchOptions::default()
                    };
                    urls_and_opts.push((url, opts));
                }
                _ => {
                    return Err(mlua::Error::RuntimeError(
                        "fetch_all: expected string or table".to_string(),
                    ));
                }
            }
        }

        // Run all fetches concurrently
        let results = block_on(async {
            let futures: Vec<_> = urls_and_opts
                .iter()
                .map(|(url, opts)| do_fetch(url, opts))
                .collect();
            futures::future::join_all(futures).await
        });

        // Convert to Lua table
        let result_table = lua.create_table()?;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(response) => {
                    let table = response.to_lua_table(lua)?;
                    result_table.set(i + 1, table)?;
                }
                Err(e) => {
                    let err_table = lua.create_table()?;
                    err_table.set("ok", false)?;
                    err_table.set("error", e)?;
                    result_table.set(i + 1, err_table)?;
                }
            }
        }

        Ok(Value::Table(result_table))
    })?;
    async_module.set("fetch_all", fetch_all)?;

    // async.spawn(url, options?) - Spawn a fetch task, returns handle for later await
    let spawn = lua.create_function(|lua, args: (String, Option<Table>)| {
        let (url, opts_table) = args;

        let opts = if let Some(ref t) = opts_table {
            FetchOptions::from_lua_table(lua, t)?
        } else {
            FetchOptions::default()
        };

        // Spawn the task on the runtime
        let handle = runtime().spawn(async move { do_fetch(&url, &opts).await });

        let task = AsyncTask {
            handle: Arc::new(Mutex::new(Some(handle))),
            result: Arc::new(Mutex::new(None)),
            completed: Arc::new(Mutex::new(false)),
        };

        Ok(task)
    })?;
    async_module.set("spawn", spawn)?;

    // async.await(task) - Await a spawned task
    let await_fn = lua.create_function(|lua, ud: mlua::AnyUserData| {
        let task = ud.borrow::<AsyncTask>()?;

        // Check if already completed
        if *task.completed.lock()
            && let Some(ref result) = *task.result.lock()
        {
            return match result {
                Ok(response) => {
                    let table = response.to_lua_table(lua)?;
                    Ok(Value::Table(table))
                }
                Err(e) => Err(mlua::Error::RuntimeError(format!("Fetch failed: {}", e))),
            };
        }

        // Take the handle and block on it
        let handle = task.handle.lock().take();
        drop(task); // Release borrow before blocking

        if let Some(h) = handle {
            let result = block_on(h);
            let task = ud.borrow::<AsyncTask>()?;
            match result {
                Ok(fetch_result) => {
                    *task.completed.lock() = true;
                    *task.result.lock() = Some(fetch_result.clone());
                    match fetch_result {
                        Ok(response) => {
                            let table = response.to_lua_table(lua)?;
                            Ok(Value::Table(table))
                        }
                        Err(e) => Err(mlua::Error::RuntimeError(format!("Fetch failed: {}", e))),
                    }
                }
                Err(e) => Err(mlua::Error::RuntimeError(format!("Task panicked: {}", e))),
            }
        } else {
            Err(mlua::Error::RuntimeError(
                "Task handle already consumed".to_string(),
            ))
        }
    })?;
    // Use raw set to avoid Lua keyword conflict with "await"
    async_module.raw_set("await", await_fn)?;

    // async.await_all(tasks) - Await multiple tasks concurrently
    let await_all = lua.create_function(|lua, tasks_table: Table| {
        let mut handles: Vec<Option<JoinHandle<FetchResult>>> = Vec::new();
        let mut already_completed: Vec<(usize, FetchResult)> = Vec::new();
        let mut task_uds: Vec<mlua::AnyUserData> = Vec::new();

        for pair in tasks_table.pairs::<i64, mlua::AnyUserData>() {
            let (_, ud) = pair?;
            task_uds.push(ud);
        }

        for (i, ud) in task_uds.iter().enumerate() {
            let task = ud.borrow::<AsyncTask>()?;
            if *task.completed.lock()
                && let Some(ref result) = *task.result.lock()
            {
                already_completed.push((i, result.clone()));
                handles.push(None);
                continue;
            }
            handles.push(task.handle.lock().take());
        }

        // Await all pending handles
        let results = block_on(async {
            let mut results: Vec<Option<FetchResult>> = Vec::with_capacity(handles.len());
            for _ in 0..handles.len() {
                results.push(None);
            }

            // Insert already completed results
            for (i, result) in already_completed {
                results[i] = Some(result);
            }

            // Await pending handles
            for (i, handle) in handles.into_iter().enumerate() {
                if let Some(h) = handle {
                    match h.await {
                        Ok(result) => {
                            results[i] = Some(result);
                        }
                        Err(e) => {
                            results[i] = Some(Err(format!("Task panicked: {}", e)));
                        }
                    }
                }
            }

            results
        });

        // Mark tasks as completed and store results
        for (i, ud) in task_uds.iter().enumerate() {
            if let Some(ref result) = results[i] {
                let task = ud.borrow::<AsyncTask>()?;
                *task.completed.lock() = true;
                *task.result.lock() = Some(result.clone());
            }
        }

        // Convert to Lua table
        let result_table = lua.create_table()?;
        for (i, result) in results.into_iter().enumerate() {
            if let Some(r) = result {
                match r {
                    Ok(response) => {
                        let table = response.to_lua_table(lua)?;
                        result_table.set(i + 1, table)?;
                    }
                    Err(e) => {
                        let err_table = lua.create_table()?;
                        err_table.set("ok", false)?;
                        err_table.set("error", e)?;
                        result_table.set(i + 1, err_table)?;
                    }
                }
            }
        }

        Ok(Value::Table(result_table))
    })?;
    async_module.set("await_all", await_all)?;

    // async.read(path) - Async binary file read
    let root_clone = root.clone();
    let read = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory",
                path
            )));
        }

        let result = block_on(async { tokio::fs::read(&resolved).await });

        match result {
            Ok(bytes) => Ok(Value::String(lua.create_string(&bytes)?)),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to read file '{}': {}",
                path, e
            ))),
        }
    })?;
    async_module.set("read", read)?;

    // async.read_file(path) - Async file read (text)
    let root_clone = root.clone();
    let read_file = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory",
                path
            )));
        }

        let result = block_on(async { tokio::fs::read_to_string(&resolved).await });

        match result {
            Ok(content) => Ok(Value::String(lua.create_string(&content)?)),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to read file '{}': {}",
                path, e
            ))),
        }
    })?;
    async_module.set("read_file", read_file)?;

    // async.write_file(path, content) - Async file write
    let root_clone = root.clone();
    let write_file = lua.create_function(move |_, (path, content): (String, String)| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot write '{}' outside project directory",
                path
            )));
        }

        // Ensure parent directory exists
        if let Some(parent) = resolved.parent() {
            let parent = parent.to_path_buf();
            block_on(async {
                tokio::fs::create_dir_all(&parent).await.ok();
            });
        }

        let result = block_on(async { tokio::fs::write(&resolved, &content).await });

        match result {
            Ok(()) => Ok(true),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to write file '{}': {}",
                path, e
            ))),
        }
    })?;
    async_module.set("write_file", write_file)?;

    // async.write(path, data) - Async binary file write
    let root_clone = root.clone();
    let write = lua.create_function(move |_, (path, data): (String, mlua::String)| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot write '{}' outside project directory",
                path
            )));
        }

        // Ensure parent directory exists
        if let Some(parent) = resolved.parent() {
            let parent = parent.to_path_buf();
            block_on(async {
                tokio::fs::create_dir_all(&parent).await.ok();
            });
        }

        let bytes = data.as_bytes().to_vec();
        let result = block_on(async { tokio::fs::write(&resolved, &bytes).await });

        match result {
            Ok(()) => Ok(true),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to write file '{}': {}",
                path, e
            ))),
        }
    })?;
    async_module.set("write", write)?;

    // async.read_files(paths) - Async batch file read
    let root_clone = root.clone();
    let read_files = lua.create_function(move |lua, paths: Vec<String>| {
        let resolved_paths: Vec<PathBuf> =
            paths.iter().map(|p| resolve_path(p, &root_clone)).collect();

        // Check sandbox
        if sandbox {
            for (i, resolved) in resolved_paths.iter().enumerate() {
                if !is_path_within_root(resolved, &root_clone) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Sandbox: cannot access '{}' outside project directory",
                        paths[i]
                    )));
                }
            }
        }

        // Read all files concurrently
        let results = block_on(async {
            let futures: Vec<_> = resolved_paths
                .iter()
                .map(tokio::fs::read_to_string)
                .collect();
            futures::future::join_all(futures).await
        });

        // Convert to Lua table
        let result_table = lua.create_table()?;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(content) => {
                    result_table.set(i + 1, content)?;
                }
                Err(e) => {
                    // Set nil for failed reads but continue
                    result_table.set(i + 1, Value::Nil)?;
                    log::warn!("Failed to read '{}': {}", paths[i], e);
                }
            }
        }

        Ok(Value::Table(result_table))
    })?;
    async_module.set("read_files", read_files)?;

    // async.load_json(path) - Async JSON file load
    let root_clone = root.clone();
    let load_json = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory",
                path
            )));
        }

        let result = block_on(async { tokio::fs::read_to_string(&resolved).await });

        match result {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(v) => lua.to_value(&v),
                Err(e) => Err(mlua::Error::RuntimeError(format!(
                    "Failed to parse JSON from '{}': {}",
                    path, e
                ))),
            },
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to read file '{}': {}",
                path, e
            ))),
        }
    })?;
    async_module.set("load_json", load_json)?;

    // async.copy_file(src, dst) - Async file copy
    let root_clone = root.clone();
    let copy_file = lua.create_function(move |_, (src, dst): (String, String)| {
        let src_resolved = resolve_path(&src, &root_clone);
        let dst_resolved = resolve_path(&dst, &root_clone);

        if sandbox {
            if !is_path_within_root(&src_resolved, &root_clone) {
                return Err(mlua::Error::RuntimeError(format!(
                    "Sandbox: cannot access '{}' outside project directory",
                    src
                )));
            }
            if !is_path_within_root(&dst_resolved, &root_clone) {
                return Err(mlua::Error::RuntimeError(format!(
                    "Sandbox: cannot write '{}' outside project directory",
                    dst
                )));
            }
        }

        // Ensure parent directory exists
        if let Some(parent) = dst_resolved.parent() {
            let parent = parent.to_path_buf();
            block_on(async {
                tokio::fs::create_dir_all(&parent).await.ok();
            });
        }

        let result = block_on(async { tokio::fs::copy(&src_resolved, &dst_resolved).await });

        match result {
            Ok(bytes) => Ok(bytes),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to copy '{}' to '{}': {}",
                src, dst, e
            ))),
        }
    })?;
    async_module.set("copy_file", copy_file)?;

    // async.rename(src, dst) - Async file/dir rename
    let root_clone = root.clone();
    let rename = lua.create_function(move |_, (src, dst): (String, String)| {
        let src_resolved = resolve_path(&src, &root_clone);
        let dst_resolved = resolve_path(&dst, &root_clone);

        if sandbox {
            if !is_path_within_root(&src_resolved, &root_clone) {
                return Err(mlua::Error::RuntimeError(format!(
                    "Sandbox: cannot access '{}' outside project directory",
                    src
                )));
            }
            if !is_path_within_root(&dst_resolved, &root_clone) {
                return Err(mlua::Error::RuntimeError(format!(
                    "Sandbox: cannot write '{}' outside project directory",
                    dst
                )));
            }
        }

        // Ensure parent directory exists
        if let Some(parent) = dst_resolved.parent() {
            let parent = parent.to_path_buf();
            block_on(async {
                tokio::fs::create_dir_all(&parent).await.ok();
            });
        }

        let result = block_on(async { tokio::fs::rename(&src_resolved, &dst_resolved).await });

        match result {
            Ok(()) => Ok(true),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to rename '{}' to '{}': {}",
                src, dst, e
            ))),
        }
    })?;
    async_module.set("rename", rename)?;

    // async.create_dir(path) - Async create directory (including parents)
    let root_clone = root.clone();
    let create_dir = lua.create_function(move |_, path: String| {
        let resolved = resolve_path(&path, &root_clone);

        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot create '{}' outside project directory",
                path
            )));
        }

        let result = block_on(async { tokio::fs::create_dir_all(&resolved).await });

        match result {
            Ok(()) => Ok(true),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to create directory '{}': {}",
                path, e
            ))),
        }
    })?;
    async_module.set("create_dir", create_dir)?;

    // async.remove_file(path) - Async file removal
    let root_clone = root.clone();
    let remove_file = lua.create_function(move |_, path: String| {
        let resolved = resolve_path(&path, &root_clone);

        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot remove '{}' outside project directory",
                path
            )));
        }

        let result = block_on(async { tokio::fs::remove_file(&resolved).await });

        match result {
            Ok(()) => Ok(true),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to remove file '{}': {}",
                path, e
            ))),
        }
    })?;
    async_module.set("remove_file", remove_file)?;

    // async.remove_dir(path) - Async directory removal (recursive)
    let root_clone = root.clone();
    let remove_dir = lua.create_function(move |_, path: String| {
        let resolved = resolve_path(&path, &root_clone);

        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot remove '{}' outside project directory",
                path
            )));
        }

        let result = block_on(async { tokio::fs::remove_dir_all(&resolved).await });

        match result {
            Ok(()) => Ok(true),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to remove directory '{}': {}",
                path, e
            ))),
        }
    })?;
    async_module.set("remove_dir", remove_dir)?;

    // async.exists(path) - Async check if file/dir exists
    let root_clone = root.clone();
    let exists = lua.create_function(move |_, path: String| {
        let resolved = resolve_path(&path, &root_clone);

        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory",
                path
            )));
        }

        let result = block_on(async { tokio::fs::try_exists(&resolved).await });

        match result {
            Ok(exists) => Ok(exists),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to check existence of '{}': {}",
                path, e
            ))),
        }
    })?;
    async_module.set("exists", exists)?;

    // async.metadata(path) - Async get file metadata
    let root_clone = root.clone();
    let metadata = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);

        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory",
                path
            )));
        }

        let result = block_on(async { tokio::fs::metadata(&resolved).await });

        match result {
            Ok(meta) => {
                let table = lua.create_table()?;
                table.set("is_file", meta.is_file())?;
                table.set("is_dir", meta.is_dir())?;
                table.set("len", meta.len())?;
                table.set("readonly", meta.permissions().readonly())?;

                // Add modified time if available
                if let Ok(modified) = meta.modified()
                    && let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
                {
                    table.set("modified", duration.as_secs())?;
                }

                Ok(Value::Table(table))
            }
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to get metadata for '{}': {}",
                path, e
            ))),
        }
    })?;
    async_module.set("metadata", metadata)?;

    // async.read_dir(path) - Async directory listing
    let root_clone = root.clone();
    let read_dir = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);

        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory",
                path
            )));
        }

        // Collect entry info inside async block
        struct EntryInfo {
            path: PathBuf,
            name: String,
            is_file: bool,
            is_dir: bool,
            is_symlink: bool,
        }

        let result = block_on(async {
            let mut entries = Vec::new();
            let mut dir = tokio::fs::read_dir(&resolved).await?;
            while let Some(entry) = dir.next_entry().await? {
                let file_type = entry.file_type().await?;
                entries.push(EntryInfo {
                    path: entry.path(),
                    name: entry.file_name().to_string_lossy().to_string(),
                    is_file: file_type.is_file(),
                    is_dir: file_type.is_dir(),
                    is_symlink: file_type.is_symlink(),
                });
            }
            Ok::<_, std::io::Error>(entries)
        });

        match result {
            Ok(entries) => {
                let result_table = lua.create_table()?;
                let mut idx = 1;
                for entry in entries {
                    // Skip entries outside sandbox
                    if sandbox && !is_path_within_root(&entry.path, &root_clone) {
                        continue;
                    }

                    let entry_table = lua.create_table()?;
                    entry_table.set("path", entry.path.to_string_lossy().to_string())?;
                    entry_table.set("name", entry.name)?;
                    entry_table.set("is_file", entry.is_file)?;
                    entry_table.set("is_dir", entry.is_dir)?;
                    entry_table.set("is_symlink", entry.is_symlink)?;

                    result_table.set(idx, entry_table)?;
                    idx += 1;
                }
                Ok(Value::Table(result_table))
            }
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to read directory '{}': {}",
                path, e
            ))),
        }
    })?;
    async_module.set("read_dir", read_dir)?;

    // async.canonicalize(path) - Async get canonical/absolute path
    let root_clone = root.clone();
    let canonicalize = lua.create_function(move |_, path: String| {
        let resolved = resolve_path(&path, &root_clone);

        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory",
                path
            )));
        }

        let result = block_on(async { tokio::fs::canonicalize(&resolved).await });

        match result {
            Ok(canonical) => Ok(canonical.to_string_lossy().to_string()),
            Err(e) => Err(mlua::Error::RuntimeError(format!(
                "Failed to canonicalize '{}': {}",
                path, e
            ))),
        }
    })?;
    async_module.set("canonicalize", canonicalize)?;

    Ok(async_module)
}
