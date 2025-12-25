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
//! ```
//!
//! ## Configuration
//!
//! Configure via `config.toml`:
//!
//! ### Required Settings
//!
//! ```toml
//! [site]
//! title = "My Site"                    # Site title
//! description = "Site description"     # Site description
//! base_url = "https://example.com"     # Base URL (no trailing slash)
//! author = "Your Name"                 # Author name
//!
//! [seo]
//! twitter_handle = "@username"         # Optional: Twitter handle
//! default_og_image = "/static/og.png"  # Optional: Default OG image
//!
//! [build]
//! output_dir = "dist"                  # Output directory
//! minify_css = true                    # Default: true
//!
//! [images]
//! quality = 85.0                       # WebP quality (default: 85.0)
//! scale_factor = 1.0                   # Image scale (default: 1.0)
//! ```
//!
//! ### Optional Settings (have defaults)
//!
//! ```toml
//! [paths]
//! content = "content"                  # Content directory (default: "content")
//! styles = "styles"                    # Styles directory (default: "styles")
//! static_files = "static"              # Static files (default: "static")
//! templates = "templates"              # Templates (default: "templates")
//! home = "index.md"                    # Home page file (default: "index.md")
//! exclude = ["drafts", "private"]      # Directories to exclude (default: [])
//!
//! [highlight]
//! names = ["John Doe", "Jane Doe"]     # Names to highlight (default: [])
//! class = "me"                         # CSS class for highlights (default: "me")
//!
//! [templates]
//! blog = "post.html"                   # Section -> template mapping
//! projects = "project.html"            # (default: uses {section}.html or post.html)
//!
//! [permalinks]
//! blog = "/:year/:month/:slug/"        # Section -> URL pattern
//! projects = "/:slug/"                 # Placeholders: :year :month :day :slug :title :section
//!
//! [encryption]
//! password_command = "pass show site"  # Command to get password (optional)
//! password = "secret"                  # Raw password (optional, less secure)
//!                                      # Priority: SITE_PASSWORD env > command > password
//!
//! [graph]
//! enabled = true                       # Enable graph generation (default: true)
//! template = "graph.html"              # Graph page template (default: "graph.html")
//! path = "graph"                       # URL path (default: "graph" -> /graph/)
//!
//! [rss]
//! enabled = true                       # Enable RSS generation (default: true)
//! filename = "rss.xml"                 # Output filename (default: "rss.xml")
//! sections = ["blog"]                  # Sections to include (default: [] = all)
//! limit = 20                           # Max items (default: 20)
//! exclude_encrypted_blocks = false     # Exclude posts with :::encrypted (default: false)
//!
//! [text]
//! enabled = false                      # Enable plain text generation (default: false)
//! sections = ["blog"]                  # Sections to include (default: [] = all)
//! exclude_encrypted = false            # Exclude encrypted posts (default: false)
//! include_home = true                  # Include home page as index.txt (default: true)
//! ```
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
pub mod encryption;
pub mod links;
pub mod markdown;
pub mod rss;
pub mod templates;
pub mod text;
