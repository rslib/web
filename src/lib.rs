//! # rs-web - Static Site Generator
//!
//! A fast, opinionated static site generator built in Rust with support for:
//!
//! - **Markdown processing** with syntax highlighting, external link handling
//! - **Content encryption** (full post or partial `:::encrypted` blocks)
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
//! - `encryption` - password_command or password (SITE_PASSWORD env takes priority)
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
//! All file operations respect the sandbox setting and are tracked for incremental builds.
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
//! encrypted: false              # Optional: Encrypt entire post
//! password: "post-secret"       # Optional: Post-specific password
//! ---
//! ```
//!
//! ## Partial Encryption
//!
//! Use `:::encrypted` blocks for partial content encryption:
//!
//! ```markdown
//! Public content here.
//!
//! :::encrypted
//! This content is encrypted with the global/post password.
//! :::
//!
//! :::encrypted password="custom"
//! This block has its own password.
//! :::
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
//! - [`encryption`] - AES-GCM encryption for protected content
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
