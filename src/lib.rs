//! # rs-web - Static Site Generator
//!
//! A fast, opinionated static site generator built in Rust with support for:
//!
//! - **Markdown processing** with syntax highlighting, external link handling
//! - **Content encryption** (full post or partial `:::encrypted` blocks)
//! - **Link graph** with backlinks and visualization (Obsidian-style)
//! - **RSS feed** generation with section filtering
//! - **Parallel processing** for fast builds
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
//! # Watch for changes and rebuild incrementally
//! rs-web build --watch
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
//!     minify_css = true,
//!   },
//!
//!   -- Section configuration with custom sort
//!   sections = {
//!     blog = {
//!       iterate = "files",
//!       sort_by = function(a, b)
//!         -- C-style comparator: return -1, 0, or 1
//!         if a.date < b.date then return -1
//!         elseif a.date > b.date then return 1
//!         else return 0 end
//!       end,
//!     },
//!   },
//!
//!   -- Computed data available in templates as {{ computed.tags }}
//!   computed = {
//!     tags = function(sections) return {...} end,
//!   },
//!
//!   -- Generate dynamic pages
//!   computed_pages = function(sections)
//!     return {{ path = "/tags/array/", template = "tag.html", title = "Array", data = {...} }}
//!   end,
//!
//!   -- Custom Tera filters: {{ value | my_filter }}
//!   filters = {
//!     shout = function(s) return s:upper() .. "!" end,
//!   },
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
//! - `build` - output_dir, minify_css (default: true)
//! - `images` - quality (default: 85.0), scale_factor (default: 1.0)
//! - `paths` - content, styles, static_files, templates, home, exclude
//! - `sections` - Per-section config with iterate ("files"/"directories") and sort_by function
//! - `templates` - Section -> template file mapping
//! - `permalinks` - Section -> URL pattern (`:year`, `:month`, `:slug`, `:title`, `:section`)
//! - `encryption` - password_command or password (SITE_PASSWORD env takes priority)
//! - `graph` - enabled, template, path
//! - `rss` - enabled, filename, sections, limit
//! - `text` - enabled, sections, exclude_encrypted, include_home
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
//! - `read_file(path)` - Read file contents
//! - `write_file(path, content)` - Write content to file
//! - `file_exists(path)` - Check if file exists
//! - `list_files(path, pattern?)` - List files matching pattern
//! - `list_dirs(path)` - List subdirectories
//! - `load_json(path)` - Load and parse JSON file
//! - `env(name)` - Get environment variable
//! - `print(...)` - Log output to build log
//!
//! All file operations respect the sandbox setting.
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
//! - [`content`] - Content discovery and post/page models
//! - [`markdown`] - Markdown processing pipeline
//! - [`templates`] - Tera template rendering
//! - [`encryption`] - AES-GCM encryption for protected content
//! - [`links`] - Link graph and backlink generation
//! - [`rss`] - RSS feed generation
//! - [`text`] - Plain text output for curl-friendly access
//! - [`assets`] - CSS building and image optimization
//! - [`build`] - Main build orchestrator

pub mod assets;
pub mod build;
pub mod config;
pub mod content;
pub mod data;
pub mod encryption;
pub mod git;
pub mod links;
pub mod markdown;
pub mod rss;
pub mod templates;
pub mod text;
pub mod watch;
