//! Lua API for rs-web
//! Usage in Lua:
//! ```lua
//! local rs = require("rs-web")
//! local content = rs.read_file("path/to/file.md")
//! local html = rs.markdown.render(content)
//! ```

mod assets;
mod async_io;
mod coro;
mod crypt;
mod css;
mod data;
mod date;
mod env;
mod fonts;
mod fs;
mod git;
mod hash;
mod helpers;
mod highlight;
mod html;
mod image;
mod js;
mod log;
mod markdown;
mod ops;
mod parallel;
mod path;
mod portable;
mod pwa;
mod seo;
mod text;
mod types;

pub use assets::{AssetManifest, create_manifest};
pub use helpers::{is_path_within_root, parse_frontmatter_content, resolve_path};
pub use highlight::highlight_code_sync;
pub use types::{LuaClass, LuaField, LuaFunction, LuaParam, generate_emmylua, generate_markdown};

use crate::tracker::SharedTracker;
use mlua::{Lua, Result, Table};
use std::path::Path;

/// Register all Lua functions as a require-able module
///
/// This function registers the rs-web module so it can be used with:
/// ```lua
/// local rs = require("rs-web")
/// ```
///
/// The module provides:
/// - File operations: rs.read_file, rs.write_file, rs.load_json, etc.
/// - Search: rs.glob, rs.scan
/// - Collections: rs.filter, rs.sort, rs.map
/// - Text: rs.slugify, rs.word_count, rs.reading_time
/// - Environment: rs.env, rs.print, rs.is_gitignored
/// - Git: rs.git_info
/// - Content: rs.rss_date, rs.html_to_text, etc.
/// - Markdown: rs.markdown.render, rs.markdown.plugins
/// - Images: rs.image_dimensions, rs.image_resize, rs.image_convert, rs.image_optimize
/// - JS: rs.js.concat, rs.js.bundle
/// - CSS: rs.css.concat, rs.css.bundle
/// - Fonts: rs.fonts.download_google_font
/// - Assets: rs.assets.hash, rs.assets.write_hashed, rs.assets.get_path
/// - PWA: rs.pwa.manifest, rs.pwa.service_worker
/// - SEO: rs.seo.sitemap, rs.seo.robots
/// - Coroutines: rs.coro.task, rs.coro.await, rs.coro.yield, etc.
/// - Parallel (rayon): rs.parallel.load_json, rs.parallel.read_files, etc.
/// - Async I/O (tokio): rs.async.fetch, rs.async.read_file, rs.async.write_file, etc.
/// - Encryption: rs.crypt.encrypt, rs.crypt.decrypt, rs.crypt.encrypt_html
pub fn register(
    lua: &Lua,
    project_root: &Path,
    sandbox: bool,
    tracker: SharedTracker,
    global_password: Option<String>,
    asset_manifest: AssetManifest,
) -> Result<()> {
    let root = project_root.to_path_buf();

    // Create the main module table
    let rs_module = lua.create_table()?;

    // Store metadata
    rs_module.set("_VERSION", env!("CARGO_PKG_VERSION"))?;
    rs_module.set("_SANDBOX", sandbox)?;
    rs_module.set("_PROJECT_ROOT", project_root.to_string_lossy().to_string())?;

    let ops_module = ops::create_module(lua)?;
    rs_module.set("ops", ops_module)?;

    let fs_module = fs::create_module(lua, &root, sandbox, tracker.clone())?;
    rs_module.set("fs", fs_module)?;

    let data_module = data::create_module(lua, &root, sandbox, tracker.clone())?;
    rs_module.set("data", data_module)?;

    let image_module = image::create_module(lua, &root, tracker.clone())?;
    rs_module.set("image", image_module)?;

    let text_module = text::create_module(lua)?;
    rs_module.set("text", text_module)?;

    let date_module = date::create_module(lua)?;
    rs_module.set("date", date_module)?;

    let path_module = path::create_module(lua)?;
    rs_module.set("path", path_module)?;

    let hash_module = hash::create_module(lua)?;
    rs_module.set("hash", hash_module)?;

    let git_module = git::create_module(lua, &root, sandbox)?;
    rs_module.set("git", git_module)?;

    let html_module = html::create_module(lua, tracker.clone())?;
    rs_module.set("html", html_module)?;

    let log_module = log::create_module(lua)?;
    rs_module.set("log", log_module)?;

    let env_module = env::create_module(lua)?;
    rs_module.set("env", env_module)?;

    let js_module = js::create_module(lua, &root, tracker.clone())?;
    rs_module.set("js", js_module)?;

    let css_module = css::create_module(lua, &root, tracker.clone())?;
    rs_module.set("css", css_module)?;

    let fonts_module = fonts::create_module(lua, &root, tracker.clone())?;
    rs_module.set("fonts", fonts_module)?;

    // Register coro and parallel as submodules
    let coro_module = coro::create_module(lua)?;
    rs_module.set("coro", coro_module)?;

    let parallel_module = parallel::create_module(lua, &root, sandbox, tracker.clone())?;
    rs_module.set("parallel", parallel_module)?;

    let async_module = async_io::create_module(lua, &root, sandbox, tracker.clone())?;
    // Use raw_set to avoid "async" being a reserved word in some contexts
    rs_module.raw_set("async", async_module)?;

    let crypt_module = crypt::create_module(lua, global_password)?;
    rs_module.set("crypt", crypt_module)?;

    let highlight_module = highlight::create_module(lua)?;
    rs_module.set("highlight", highlight_module)?;

    let assets_module = assets::create_module(lua, &root, asset_manifest, tracker.clone())?;
    rs_module.set("assets", assets_module)?;

    let pwa_module = pwa::create_module(lua, &root, tracker.clone())?;
    rs_module.set("pwa", pwa_module)?;

    let seo_module = seo::create_module(lua, &root, tracker.clone())?;
    rs_module.set("seo", seo_module)?;

    // Register markdown module (rs.markdown.render, rs.markdown.plugins)
    markdown::register(lua, &rs_module, tracker)?;

    // Register as a preloaded module so require("rs-web") works
    let preload: Table = lua
        .globals()
        .get::<Table>("package")?
        .get::<Table>("preload")?;

    let loader = lua.create_function(move |_, _: ()| Ok(rs_module.clone()))?;
    preload.set("rs-web", loader)?;

    Ok(())
}
