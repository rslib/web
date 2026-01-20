//! # rs-web - Static Site Generator
//!
//! A fast, opinionated static site generator built in Rust with support for:
//!
//! - **Markdown processing** with syntax highlighting, external link handling
//! - **Content encryption** via Lua (`rs.crypt` module)
//! - **Link graph** with backlinks and visualization (Obsidian-style)
//! - **RSS feed** generation with section filtering
//! - **Parallel processing** for fast builds
//! - **Live reload** with automatic browser refresh during watch mode
//! - **Asset minification** for CSS, JS (with dead code elimination), and HTML
//!
//! ## Quick Start
//!
//! ```bash
//! # Build the site
//! rs-web build
//!
//! # Build to custom output directory
//! rs-web build --output public
//!
//! # Watch for changes and rebuild incrementally with live reload
//! rs-web build --watch
//!
//! # Watch mode with custom port
//! rs-web build --watch --port 8080
//!
//! # Enable debug logging
//! rs-web --debug build
//!
//! # Set specific log level (trace, debug, info, warning, error)
//! rs-web --log-level trace build
//!
//! # Or use environment variable
//! RS_WEB_LOG_LEVEL=debug rs-web build
//! ```
//!
//! ## Configuration
//!
//! Configure via `config.lua`:
//!
//! ```lua
//! return {
//!   site = {
//!     title = "My Site",
//!     description = "Site description",
//!     base_url = "https://example.com",
//!     author = "Your Name",
//!   },
//!
//!   build = {
//!     output_dir = "dist",
//!   },
//!
//!   -- Generate pages via Lua
//!   pages = function()
//!     return {
//!       { path = "/", template = "home.html", title = "Home" },
//!       { path = "/about/", template = "page.html", title = "About", minify = true },
//!     }
//!   end,
//!
//!   -- Build hooks
//!   hooks = {
//!     before_build = function() print("Starting...") end,
//!     after_build = function() print("Done!") end,
//!   },
//! }
//! ```
//!
//! ### Configuration Sections
//!
//! - `site` - Required: title, description, base_url, author
//! - `seo` - twitter_handle, default_og_image
//! - `build` - output_dir
//! - `paths` - templates
//!
//! ### Lua Sandbox
//!
//! By default, file operations are sandboxed to the project directory.
//! To disable (use with caution):
//!
//! ```lua
//! return {
//!   lua = { sandbox = false },
//!   site = { ... },
//! }
//! ```
//!
//! ### Lua API Functions
//!
//! **File Operations:**
//! - `read_file(path)` - Read file contents
//! - `write_file(path, content)` - Write content to file
//! - `copy_file(src, dest)` - Copy file (binary-safe)
//! - `file_exists(path)` - Check if file exists
//! - `list_files(path, pattern?)` - List files matching pattern
//! - `list_dirs(path)` - List subdirectories
//! - `load_json(path)` - Load and parse JSON file
//! - `load_yaml(path)` - Load and parse YAML file
//! - `load_toml(path)` - Load and parse TOML file
//! - `read_frontmatter(path)` - Extract frontmatter and content from markdown
//!
//! **Content Processing:**
//! - `html_to_text(html)` - Convert HTML to plain text
//! - `rss_date(date_string)` - Format date for RSS (RFC 2822)
//!
//! **Markdown (rs.markdown):**
//! - `render(content, opts?)` - Render markdown to HTML with optional plugins
//! - `plugins(...)` - Combine/flatten plugins
//! - `plugins.default(opts?)` - Get default plugins (lazy_images, heading_anchors, external_links)
//! - `plugins.lazy_images(opts?)` - Plugin: add loading="lazy" decoding="async" to images
//! - `plugins.heading_anchors(opts?)` - Plugin: add id="slug" to headings
//! - `plugins.external_links(opts?)` - Plugin: add target="_blank" rel="noopener" to external links
//!
//! **Image Processing:**
//! - `image_dimensions(path)` - Get image width and height
//! - `image_resize(input, output, options)` - Resize image
//! - `image_convert(input, output, options?)` - Convert image format
//! - `image_optimize(input, output, options?)` - Optimize/compress image
//!
//! **Fonts (rs.fonts):**
//! - `download_google_font(family, options)` - Download Google Font with optional CSS minification (async)
//!
//! **JS Module (rs.js):**
//! - `concat(paths, output, options?)` - Concatenate JS files with optional minification (async)
//! - `bundle(entry, output, options?)` - Bundle JS with imports via Rolldown (async)
//! - `bundle_many(entries, output_dir, options?)` - Bundle multiple JS entries to separate files (async)
//!
//! **CSS Module (rs.css):**
//! - `concat(paths, output, options?)` - Concatenate CSS files with optional minification (async)
//! - `bundle(paths, output, options?)` - Bundle CSS with @import resolution via LightningCSS (async)
//! - `bundle_many(paths, output_dir, options?)` - Bundle multiple CSS entries to separate files (async)
//! - `purge(css_path, options?)` - Remove unused CSS rules based on HTML/JS output (async, call in after_build)
//! - `critical(html, css_path, options?)` - Extract critical CSS for a specific HTML page (async)
//! - `inline_critical(html_path, css_path, options?)` - Inline critical CSS into HTML with async loading (async)
//!
//! **Asset Hashing (rs.assets):**
//! - `hash(content, length?)` - Compute SHA256 hash of content (async)
//! - `hash_sync(content, length?)` - Compute hash synchronously
//! - `write_hashed(content, path, options?)` - Write file with hashed filename (async)
//! - `register(original, hashed)` - Register asset path mapping
//! - `get_path(path)` - Get hashed path for original
//! - `manifest()` - Get all path mappings
//! - `clear()` - Clear the manifest
//!
//! **PWA (rs.pwa):**
//! - `manifest(options)` - Generate web app manifest.json (async)
//! - `service_worker(options)` - Generate service worker sw.js (async)
//!
//! **SEO (rs.seo):**
//! - `sitemap(options)` - Generate XML sitemap (async)
//! - `robots(options)` - Generate robots.txt (async)
//!
//! **Text Processing:**
//! - `slugify(text)` - Convert text to URL-friendly slug
//! - `word_count(text)` - Count words in text
//! - `reading_time(text, wpm?)` - Calculate reading time in minutes
//! - `truncate(text, len, suffix?)` - Truncate text with optional suffix
//! - `strip_tags(html)` - Remove HTML tags
//! - `format_date(date, format)` - Format a date string
//! - `parse_date(str)` - Parse date string to table {year, month, day}
//! - `hash(content)` - Hash content (xxHash64)
//! - `hash_file(path)` - Hash file contents
//! - `url_encode(str)` - URL encode a string
//! - `url_decode(str)` - URL decode a string
//!
//! **Path Utilities:**
//! - `join_path(...)` - Join path segments
//! - `basename(path)` - Get file name from path
//! - `dirname(path)` - Get directory from path
//! - `extension(path)` - Get file extension
//!
//! **Collections:**
//! - `filter(items, fn)` - Filter items where fn returns true
//! - `sort(items, fn)` - Sort items using comparator
//! - `map(items, fn)` - Transform each item
//! - `find(items, fn)` - Find first item where fn returns true
//! - `group_by(items, key_fn)` - Group items by key
//! - `unique(items)` - Remove duplicates
//! - `reverse(items)` - Reverse array order
//! - `take(items, n)` - Take first n items
//! - `skip(items, n)` - Skip first n items
//! - `keys(table)` - Get all keys from a table
//! - `values(table)` - Get all values from a table
//!
//! **Environment:**
//! - `env(name)` - Get environment variable
//! - `print(...)` - Log output to build log
//! - `git_info(path?)` - Get git info (hash, branch, author, date, dirty)
//!
//! **Parallel (rs.parallel):** Rayon-backed true parallel operations
//! - `load_json(paths)` / `load_yaml(paths)` - Load multiple files in parallel
//! - `read_files(paths)` / `read_frontmatter(paths)` - Read multiple files
//! - `create_dirs(paths)` / `copy_files(sources, dests)` - Parallel file operations
//! - `image_convert(sources, dests, opts?)` - Convert images in parallel
//! - `map(items, fn, ctx?)` / `filter(items, fn, ctx?)` - Parallel map/filter with explicit context
//! - `map_seq(items, fn)` / `filter_seq(items, fn)` - Sequential fallbacks for non-serializable items
//!
//! **Async I/O (rs.async):** All return handles, await with `rs.async.await(task)` or `rs.async.await_all(tasks)`
//! - `fetch(url, opts?)` / `fetch_bytes(url, opts?)` - HTTP fetch (text/binary)
//! - `fetch_sync(url, opts?)` - Blocking fetch (returns response directly)
//! - `write_file(path, content)` / `write(path, bytes)` - Write text/binary
//! - `copy_file(src, dst)` / `create_dir(path)` - File/dir operations
//! - `exists(path)` / `read_file(path)` - Check existence / read file
//!
//! **Encryption (rs.crypt):**
//! - `encrypt(content, password?)` - Encrypt content (AES-256-GCM)
//! - `decrypt(data, password?)` - Decrypt content
//! - `encrypt_html(content, options?)` - Generate encrypted HTML block for browser decryption
//!
//! All file operations respect the sandbox setting and are tracked for incremental builds.
//! Encryption uses SITE_PASSWORD environment variable if password is not provided.
//!
//! ## Frontmatter
//!
//! Post frontmatter options (YAML or TOML):
//!
//! ```yaml
//! ---
//! title: "Post Title"           # Required
//! description: "Description"    # Optional
//! date: 2024-01-15              # Optional (YAML date or string)
//! tags: ["tag1", "tag2"]        # Optional
//! draft: false                  # Optional (default: false, excluded from build)
//! image: "/static/post.png"     # Optional: OG image
//! template: "custom.html"       # Optional: Override template
//! slug: "custom-slug"           # Optional: Override URL slug
//! permalink: "/custom/url/"     # Optional: Full URL override
//! ---
//! ```
//!
//! ## Encryption
//!
//! Encryption is handled via the `rs.crypt` module in Lua. Use `SITE_PASSWORD`
//! environment variable or pass password explicitly:
//!
//! ```lua
//! -- Encrypt content
//! local encrypted = rs.crypt.encrypt("secret content")
//! -- Returns: { ciphertext, salt, nonce }
//!
//! -- Generate encrypted HTML block for browser decryption
//! local html = rs.crypt.encrypt_html("secret content", {
//!   slug = "post-slug",
//!   block_id = "secret-1",
//! })
//!
//! -- Decrypt content
//! local plaintext = rs.crypt.decrypt(encrypted)
//! ```
//!
//! ## Template Variables
//!
//! ### Home Template (`home.html`)
//! - `site` - Site config (title, description, base_url, author)
//! - `page` - Page info (title, description, url, image)
//! - `sections` - All sections with posts (`sections.blog.posts`)
//! - `content` - Rendered markdown content
//!
//! ### Post Template (`post.html`)
//! - `site` - Site config
//! - `post` - Post info (title, url, date, tags, reading_time, etc.)
//! - `page` - Page info for head.html compatibility
//! - `content` - Rendered markdown content
//! - `backlinks` - Posts linking to this post (url, title, section)
//! - `graph` - Local graph data (nodes, edges) for visualization
//!
//! ### Graph Template (`graph.html`)
//! - `site` - Site config
//! - `page` - Page info
//! - `graph` - Full graph data (nodes, edges)
//!
//! ## Modules
//!
//! - [`config`] - Configuration loading and structures
//! - [`lua`] - Lua API including `rs.markdown` for markdown processing
//! - [`templates`] - Tera template rendering
//! - [`encryption`] - AES-256-GCM encryption utilities (used by `rs.crypt` Lua module)
//! - [`build`] - Main build orchestrator

/// Print output at info level (always shown regardless of log level)
#[macro_export]
macro_rules! rs_print {
    ($($arg:tt)*) => {
        log::info!(target: "rs_print", "{}", format!($($arg)*))
    };
}

pub mod assets;
pub mod build;
pub mod config;
pub mod data;
pub mod encryption;
pub mod git;
pub mod lua;
pub mod server;
pub mod templates;
pub mod text;
pub mod tracker;
pub mod watch;
