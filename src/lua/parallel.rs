//! Parallel processing module (rs.parallel) - rayon-backed parallel operations

use super::helpers::{is_path_within_root, parse_frontmatter_content, resolve_path};
use super::portable::LuaPortable;
use crate::tracker::SharedTracker;
use image::DynamicImage;
use mlua::{Function, Lua, LuaSerdeExt, Result, Table, Value};
use rayon::prelude::*;
use std::cell::RefCell;
use std::path::Path;

// Thread-local Lua VM for parallel execution
// Each rayon thread gets its own Lua state
thread_local! {
    static WORKER_LUA: RefCell<Option<Lua>> = const { RefCell::new(None) };
}

/// Get or initialize the thread-local Lua VM
fn get_or_init_worker_lua() -> &'static Lua {
    WORKER_LUA.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if borrow.is_none() {
            let lua = Lua::new();
            // Load standard safe libraries (no debug needed - we use explicit context)
            lua.load_std_libs(mlua::StdLib::ALL_SAFE).ok();
            *borrow = Some(lua);
        }
        // SAFETY: The Lua VM is thread-local and lives for the duration of the thread
        unsafe { &*(borrow.as_ref().unwrap() as *const Lua) }
    })
}

/// Create the parallel module table
pub fn create_module(
    lua: &Lua,
    project_root: &Path,
    sandbox: bool,
    tracker: SharedTracker,
) -> Result<Table> {
    let parallel = lua.create_table()?;
    let root = project_root.to_path_buf();

    // parallel.load_json(paths) - Load multiple JSON files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let load_json_parallel = lua.create_function(move |lua, paths: Table| {
        // Collect paths from Lua table
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        // Process in parallel
        let tracker_ref = &tracker_clone;
        let results: Vec<Option<serde_json::Value>> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, &root_clone);
                if sandbox && !is_path_within_root(&resolved, &root_clone) {
                    return None;
                }
                std::fs::read_to_string(&resolved).ok().and_then(|content| {
                    // Canonicalize path for consistent tracking
                    let canonical = resolved.canonicalize().unwrap_or(resolved);
                    tracker_ref.record_read(canonical, content.as_bytes());
                    serde_json::from_str(&content).ok()
                })
            })
            .collect();

        // Convert results back to Lua table
        let result_table = lua.create_table()?;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Some(v) => result_table.set(i + 1, lua.to_value(&v)?)?,
                None => result_table.set(i + 1, Value::Nil)?,
            }
        }
        Ok(result_table)
    })?;
    parallel.set("load_json", load_json_parallel)?;

    // parallel.read_files(paths) - Read multiple files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let read_files_parallel = lua.create_function(move |lua, paths: Table| {
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        let tracker_ref = &tracker_clone;
        let results: Vec<Option<String>> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, &root_clone);
                if sandbox && !is_path_within_root(&resolved, &root_clone) {
                    return None;
                }
                std::fs::read_to_string(&resolved).ok().inspect(|content| {
                    let canonical = resolved.canonicalize().unwrap_or(resolved);
                    tracker_ref.record_read(canonical, content.as_bytes());
                })
            })
            .collect();

        let result_table = lua.create_table()?;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Some(content) => result_table.set(i + 1, lua.create_string(&content)?)?,
                None => result_table.set(i + 1, Value::Nil)?,
            }
        }
        Ok(result_table)
    })?;
    parallel.set("read_files", read_files_parallel)?;

    // parallel.file_exists(paths) - Check multiple files exist in parallel
    let root_clone = root.clone();
    let file_exists_parallel = lua.create_function(move |lua, paths: Table| {
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        let results: Vec<bool> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, &root_clone);
                if sandbox && !is_path_within_root(&resolved, &root_clone) {
                    return false;
                }
                resolved.exists()
            })
            .collect();

        let result_table = lua.create_table()?;
        for (i, exists) in results.into_iter().enumerate() {
            result_table.set(i + 1, exists)?;
        }
        Ok(result_table)
    })?;
    parallel.set("file_exists", file_exists_parallel)?;

    // parallel.load_yaml(paths) - Load multiple YAML files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let load_yaml_parallel = lua.create_function(move |lua, paths: Table| {
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        let tracker_ref = &tracker_clone;
        let results: Vec<Option<serde_json::Value>> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, &root_clone);
                if sandbox && !is_path_within_root(&resolved, &root_clone) {
                    return None;
                }
                std::fs::read_to_string(&resolved).ok().and_then(|content| {
                    let canonical = resolved.canonicalize().unwrap_or(resolved);
                    tracker_ref.record_read(canonical, content.as_bytes());
                    serde_yaml::from_str(&content).ok()
                })
            })
            .collect();

        let result_table = lua.create_table()?;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Some(v) => result_table.set(i + 1, lua.to_value(&v)?)?,
                None => result_table.set(i + 1, Value::Nil)?,
            }
        }
        Ok(result_table)
    })?;
    parallel.set("load_yaml", load_yaml_parallel)?;

    // parallel.read_frontmatter(paths) - Parse frontmatter from multiple files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let read_frontmatter_parallel = lua.create_function(move |lua, paths: Table| {
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        let tracker_ref = &tracker_clone;
        let results: Vec<Option<(Option<serde_json::Value>, String, String)>> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, &root_clone);
                if sandbox && !is_path_within_root(&resolved, &root_clone) {
                    return None;
                }
                std::fs::read_to_string(&resolved).ok().map(|raw| {
                    let canonical = resolved.canonicalize().unwrap_or(resolved);
                    tracker_ref.record_read(canonical, raw.as_bytes());
                    let (fm, content) = parse_frontmatter_content(&raw);
                    (fm, content, raw)
                })
            })
            .collect();

        let result_table = lua.create_table()?;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Some((fm, content, raw)) => {
                    let entry = lua.create_table()?;
                    entry.set("raw", raw)?;
                    entry.set("content", content)?;
                    match fm {
                        Some(v) => entry.set("frontmatter", lua.to_value(&v)?)?,
                        None => entry.set("frontmatter", lua.create_table()?)?,
                    }
                    result_table.set(i + 1, entry)?;
                }
                None => result_table.set(i + 1, Value::Nil)?,
            }
        }
        Ok(result_table)
    })?;
    parallel.set("read_frontmatter", read_frontmatter_parallel)?;

    // parallel.map(items, fn, ctx?) - True parallel map using thread-local Lua VMs
    // Items, function, and optional context are serialized, then executed in parallel.
    // Usage: rs.parallel.map(items, function(item, ctx) ... end, {key = value})
    // Note: Function upvalues are NOT captured. Pass any needed context explicitly.
    let map_fn = lua.create_function(
        |lua, (items, func, ctx): (Table, Function, Option<Table>)| {
            // Serialize the function to bytecode
            let portable_func = LuaPortable::from_lua(&Value::Function(func), lua)?;

            // Serialize the context if provided
            let portable_ctx = match ctx {
                Some(t) => Some(LuaPortable::from_lua(&Value::Table(t), lua)?),
                None => None,
            };

            // Serialize all items
            let portable_items: Vec<LuaPortable> = items
                .sequence_values::<Value>()
                .filter_map(|v| v.ok())
                .map(|v| LuaPortable::from_lua(&v, lua))
                .collect::<Result<Vec<_>>>()?;

            // Execute in parallel using rayon
            let results: Vec<std::result::Result<LuaPortable, String>> = portable_items
                .par_iter()
                .map(|item| {
                    let worker_lua = get_or_init_worker_lua();

                    // Load function from bytecode in this thread's VM
                    let func_value = portable_func
                        .to_lua(worker_lua)
                        .map_err(|e| format!("Failed to load function: {}", e))?;
                    let func = func_value
                        .as_function()
                        .ok_or_else(|| "Expected function".to_string())?;

                    // Convert item to Lua value
                    let lua_item = item
                        .to_lua(worker_lua)
                        .map_err(|e| format!("Failed to convert item: {}", e))?;

                    // Convert context to Lua value (or nil)
                    let lua_ctx = match &portable_ctx {
                        Some(ctx) => ctx
                            .to_lua(worker_lua)
                            .map_err(|e| format!("Failed to convert context: {}", e))?,
                        None => Value::Nil,
                    };

                    // Call function with (item, ctx)
                    let result: Value = func
                        .call((lua_item, lua_ctx))
                        .map_err(|e| format!("Function call failed: {}", e))?;

                    // Convert result back to portable
                    LuaPortable::from_lua(&result, worker_lua)
                        .map_err(|e| format!("Failed to serialize result: {}", e))
                })
                .collect();

            // Convert back to Lua table
            let result_table = lua.create_table()?;
            for (i, result) in results.into_iter().enumerate() {
                match result {
                    Ok(portable) => {
                        let value = portable.to_lua(lua)?;
                        result_table.set(i + 1, value)?;
                    }
                    Err(e) => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "parallel.map failed at index {}: {}",
                            i + 1,
                            e
                        )));
                    }
                }
            }
            Ok(result_table)
        },
    )?;
    parallel.set("map", map_fn)?;

    // parallel.map_seq(items, fn) - Sequential map (for non-serializable items)
    // Use this when items contain userdata or other non-portable values
    let map_seq_fn = lua.create_function(|lua, (items, func): (Table, Function)| {
        let result_table = lua.create_table()?;
        let mut i = 1;
        for v in items.sequence_values::<Value>().flatten() {
            let res: Value = func.call(v)?;
            result_table.set(i, res)?;
            i += 1;
        }
        Ok(result_table)
    })?;
    parallel.set("map_seq", map_seq_fn)?;

    // parallel.filter(items, fn, ctx?) - True parallel filter using thread-local Lua VMs
    // Usage: rs.parallel.filter(items, function(item, ctx) ... end, {key = value})
    // Note: Function upvalues are NOT captured. Pass any needed context explicitly.
    let filter_fn = lua.create_function(
        |lua, (items, func, ctx): (Table, Function, Option<Table>)| {
            // Serialize the function to bytecode
            let portable_func = LuaPortable::from_lua(&Value::Function(func), lua)?;

            // Serialize the context if provided
            let portable_ctx = match ctx {
                Some(t) => Some(LuaPortable::from_lua(&Value::Table(t), lua)?),
                None => None,
            };

            // Serialize all items with their original indices
            let portable_items: Vec<(usize, LuaPortable)> = items
                .sequence_values::<Value>()
                .enumerate()
                .filter_map(|(i, v)| v.ok().map(|v| (i, v)))
                .map(|(i, v)| LuaPortable::from_lua(&v, lua).map(|p| (i, p)))
                .collect::<Result<Vec<_>>>()?;

            // Execute filter in parallel
            let results: Vec<std::result::Result<Option<LuaPortable>, String>> = portable_items
                .par_iter()
                .map(|(_, item)| {
                    let worker_lua = get_or_init_worker_lua();

                    // Load function
                    let func_value = portable_func
                        .to_lua(worker_lua)
                        .map_err(|e| format!("Failed to load function: {}", e))?;
                    let func = func_value
                        .as_function()
                        .ok_or_else(|| "Expected function".to_string())?;

                    // Convert item
                    let lua_item = item
                        .to_lua(worker_lua)
                        .map_err(|e| format!("Failed to convert item: {}", e))?;

                    // Convert context to Lua value (or nil)
                    let lua_ctx = match &portable_ctx {
                        Some(ctx) => ctx
                            .to_lua(worker_lua)
                            .map_err(|e| format!("Failed to convert context: {}", e))?,
                        None => Value::Nil,
                    };

                    // Call predicate with (item, ctx)
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

            // Collect kept items maintaining order
            let result_table = lua.create_table()?;
            let mut i = 1;
            for result in results.into_iter() {
                match result {
                    Ok(Some(portable)) => {
                        let value = portable.to_lua(lua)?;
                        result_table.set(i, value)?;
                        i += 1;
                    }
                    Ok(None) => {} // Filtered out
                    Err(e) => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "parallel.filter failed: {}",
                            e
                        )));
                    }
                }
            }
            Ok(result_table)
        },
    )?;
    parallel.set("filter", filter_fn)?;

    // parallel.filter_seq(items, fn) - Sequential filter (for non-serializable items)
    let filter_seq_fn = lua.create_function(|lua, (items, func): (Table, Function)| {
        let result_table = lua.create_table()?;
        let mut i = 1;
        for v in items.sequence_values::<Value>().flatten() {
            let keep: bool = func.call(v.clone())?;
            if keep {
                result_table.set(i, v)?;
                i += 1;
            }
        }
        Ok(result_table)
    })?;
    parallel.set("filter_seq", filter_seq_fn)?;

    // parallel.reduce(items, initial, fn) - Reduce items to single value
    let reduce_fn =
        lua.create_function(|_, (items, initial, func): (Table, Value, Function)| {
            let mut acc = initial;
            for v in items.sequence_values::<Value>().flatten() {
                acc = func.call((acc, v))?;
            }
            Ok(acc)
        })?;
    parallel.set("reduce", reduce_fn)?;

    // parallel.create_dirs(paths) - Create multiple directories in parallel
    let root_clone = root.clone();
    let create_dirs_parallel = lua.create_function(move |lua, paths: Table| {
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        let root_ref = &root_clone;
        let results: Vec<std::result::Result<(), String>> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, root_ref);
                std::fs::create_dir_all(&resolved)
                    .map_err(|e| format!("Failed to create {}: {}", path, e))
            })
            .collect();

        let result_table = lua.create_table()?;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(()) => result_table.set(i + 1, true)?,
                Err(e) => result_table.set(i + 1, e)?,
            }
        }
        Ok(result_table)
    })?;
    parallel.set("create_dirs", create_dirs_parallel)?;

    // parallel.copy_files(sources, destinations) - Copy multiple files in parallel
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let copy_files_parallel =
        lua.create_function(move |lua, (sources, dests): (Table, Table)| {
            let source_list: Vec<String> = sources
                .sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect();
            let dest_list: Vec<String> = dests
                .sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect();

            if source_list.len() != dest_list.len() {
                return Err(mlua::Error::external(
                    "parallel.copy_files: sources and destinations must have same length",
                ));
            }

            let pairs: Vec<_> = source_list.into_iter().zip(dest_list).collect();
            let tracker_ref = &tracker_clone;
            let root_ref = &root_clone;

            let results: Vec<std::result::Result<(), String>> = pairs
                .par_iter()
                .map(|(src, dest)| {
                    let src_path = resolve_path(src, root_ref);
                    let dest_path = resolve_path(dest, root_ref);

                    if sandbox && !is_path_within_root(&src_path, root_ref) {
                        return Err(format!("Source path outside sandbox: {}", src));
                    }

                    // Create parent directory
                    if let Some(parent) = dest_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("Failed to create directory: {}", e))?;
                    }

                    // Read and copy
                    let content = std::fs::read(&src_path)
                        .map_err(|e| format!("Failed to read {}: {}", src, e))?;

                    let src_canonical = src_path.canonicalize().unwrap_or(src_path);
                    tracker_ref.record_read(src_canonical, &content);

                    std::fs::write(&dest_path, &content)
                        .map_err(|e| format!("Failed to write {}: {}", dest, e))?;

                    let dest_canonical = dest_path.canonicalize().unwrap_or(dest_path);
                    tracker_ref.record_write(dest_canonical, &content);

                    Ok(())
                })
                .collect();

            // Return table of results (true for success, error string for failure)
            let result_table = lua.create_table()?;
            for (i, result) in results.into_iter().enumerate() {
                match result {
                    Ok(()) => result_table.set(i + 1, true)?,
                    Err(e) => result_table.set(i + 1, e)?,
                }
            }
            Ok(result_table)
        })?;
    parallel.set("copy_files", copy_files_parallel)?;

    // parallel.image_convert(sources, destinations, options?) - Convert multiple images in parallel
    // options: { quality?: number }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let image_convert_parallel = lua.create_function(
        move |lua, (sources, dests, options): (Table, Table, Option<Table>)| {
            let source_list: Vec<String> = sources
                .sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect();
            let dest_list: Vec<String> = dests
                .sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect();

            if source_list.len() != dest_list.len() {
                return Err(mlua::Error::external(
                    "parallel.image_convert: sources and destinations must have same length",
                ));
            }

            let quality: f32 = options
                .as_ref()
                .and_then(|t| t.get("quality").ok())
                .unwrap_or(85.0);

            let pairs: Vec<_> = source_list.into_iter().zip(dest_list).collect();
            let tracker_ref = &tracker_clone;
            let root_ref = &root_clone;

            let results: Vec<std::result::Result<(), String>> = pairs
                .par_iter()
                .map(|(src, dest)| {
                    let src_path = resolve_path(src, root_ref);
                    let dest_path = resolve_path(dest, root_ref);

                    if sandbox && !is_path_within_root(&src_path, root_ref) {
                        return Err(format!("Source path outside sandbox: {}", src));
                    }

                    // Read source
                    let content = std::fs::read(&src_path)
                        .map_err(|e| format!("Failed to read {}: {}", src, e))?;

                    let src_canonical = src_path.canonicalize().unwrap_or(src_path.clone());
                    tracker_ref.record_read(src_canonical, &content);

                    // Open and apply EXIF orientation
                    let img = image::open(&src_path)
                        .map_err(|e| format!("Failed to open image {}: {}", src, e))?;
                    let img = apply_exif_orientation(img, &src_path);

                    // Create parent directory
                    if let Some(parent) = dest_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("Failed to create directory: {}", e))?;
                    }

                    // Determine format and save
                    let ext = dest_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_else(|| "png".to_string());

                    let output_bytes = match ext.as_str() {
                        "webp" => {
                            let encoder = webp::Encoder::from_image(&img)
                                .map_err(|e| format!("WebP encoder error: {}", e))?;
                            encoder.encode(quality).to_vec()
                        }
                        "jpg" | "jpeg" => {
                            let mut buf = Vec::new();
                            let mut cursor = std::io::Cursor::new(&mut buf);
                            img.write_to(&mut cursor, image::ImageFormat::Jpeg)
                                .map_err(|e| format!("JPEG encode error: {}", e))?;
                            buf
                        }
                        "png" => {
                            let mut buf = Vec::new();
                            let mut cursor = std::io::Cursor::new(&mut buf);
                            img.write_to(&mut cursor, image::ImageFormat::Png)
                                .map_err(|e| format!("PNG encode error: {}", e))?;
                            buf
                        }
                        _ => {
                            return Err(format!("Unsupported output format: {}", ext));
                        }
                    };

                    std::fs::write(&dest_path, &output_bytes)
                        .map_err(|e| format!("Failed to write {}: {}", dest, e))?;

                    let dest_canonical = dest_path.canonicalize().unwrap_or(dest_path);
                    tracker_ref.record_write(dest_canonical, &output_bytes);

                    Ok(())
                })
                .collect();

            // Return table of results
            let result_table = lua.create_table()?;
            for (i, result) in results.into_iter().enumerate() {
                match result {
                    Ok(()) => result_table.set(i + 1, true)?,
                    Err(e) => result_table.set(i + 1, e)?,
                }
            }
            Ok(result_table)
        },
    )?;
    parallel.set("image_convert", image_convert_parallel)?;

    Ok(parallel)
}

/// Apply EXIF orientation to an image
fn apply_exif_orientation(img: DynamicImage, path: &Path) -> DynamicImage {
    use std::fs::File;
    use std::io::BufReader;

    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return img,
    };

    let exif = match exif::Reader::new().read_from_container(&mut BufReader::new(file)) {
        Ok(e) => e,
        Err(_) => return img,
    };

    let orientation = exif
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .unwrap_or(1);

    match orientation {
        1 => img,
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}
