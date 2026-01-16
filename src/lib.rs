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
//!       { path = "/about/", template = "page.html", title = "About" },
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
//! - `paths` - styles, static_files, templates
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
//! - `render_markdown(content, transform_fn?)` - Convert markdown to HTML
//! - `html_to_text(html)` - Convert HTML to plain text
//! - `rss_date(date_string)` - Format date for RSS (RFC 2822)
//!
//! **Image Processing:**
//! - `image_dimensions(path)` - Get image width and height
//! - `image_resize(input, output, options)` - Resize image
//! - `image_convert(input, output, options?)` - Convert image format
//! - `image_optimize(input, output, options?)` - Optimize/compress image
//!
//! **Asset Building:**
//! - `build_css(pattern, output, options?)` - Build and concatenate CSS files
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
//!
//! **Async I/O (rs.async):**
//! - `fetch(url, options?)` - HTTP fetch (blocking)
//! - `fetch_json(url, options?)` - Fetch and parse JSON
//! - `fetch_all(requests)` - Fetch multiple URLs concurrently
//! - `spawn(url, options?)` - Spawn async fetch task
//! - `await(task)` - Await spawned task
//! - `await_all(tasks)` - Await multiple tasks
//! - `read(path)` - Read file as binary
//! - `read_file(path)` - Read file as text
//! - `read_files(paths)` - Read multiple files concurrently
//! - `write_file(path, content)` - Write file
//! - `copy_file(src, dst)` - Copy file
//! - `rename(src, dst)` - Rename/move file or directory
//! - `remove_file(path)` - Remove file
//! - `remove_dir(path)` - Remove directory recursively
//! - `create_dir(path)` - Create directory (including parents)
//! - `exists(path)` - Check if path exists
//! - `metadata(path)` - Get file metadata
//! - `read_dir(path)` - List directory contents
//! - `canonicalize(path)` - Get canonical/absolute path
//!
//! **Encryption (rs.crypt):**
//! - `encrypt(content, password?)` - Encrypt content (AES-256-GCM)
//! - `decrypt(data, password?)` - Decrypt content
//! - `encrypt_html(content, options?)` - Generate encrypted HTML block for browser decryption
//!
//! All file operations respect the sandbox setting and are tracked for incremental builds.
//! Encryption uses SITE_PASSWORD environment variable if password is not provided.
//!
//! ## Root Pages
//!
//! Markdown files at the content root (besides the home page) are processed as
//! standalone pages. For example, `404.md` becomes `404.html`:
//!
//! ```yaml
//! ---
//! title: "404 - Page Not Found"
//! template: "error.html"
//! ---
//!
//! # Page Not Found
//! The page you're looking for doesn't exist.
//! ```
//!
//! Default excluded files (disable with `exclude_defaults = false`):
//! - README.md, LICENSE.md, CHANGELOG.md, CONTRIBUTING.md, CODE_OF_CONDUCT.md
//! - Hidden files (starting with `.`)
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
//! ## Encryption (via Lua)
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
//! - [`markdown`] - Markdown processing pipeline
//! - [`templates`] - Tera template rendering
//! - [`encryption`] - AES-256-GCM encryption utilities (used by `rs.crypt` Lua module)
//! - [`build`] - Main build orchestrator

pub mod assets;
pub mod build;
pub mod config;
pub mod data;
pub mod encryption;
pub mod git;
pub mod lua;
pub mod markdown;
pub mod server;
pub mod templates;
pub mod text;
pub mod watch;
