//! Lua API type definitions for EmmyLua annotations

/// A Lua function definition
#[derive(Debug, Clone)]
pub struct LuaFunction {
    pub name: &'static str,
    pub module: Option<&'static str>,
    pub description: &'static str,
    pub params: &'static [LuaParam],
    pub returns: &'static str,
}

/// A Lua function parameter
#[derive(Debug, Clone)]
pub struct LuaParam {
    pub name: &'static str,
    pub typ: &'static str,
    pub description: &'static str,
    pub optional: bool,
}

/// A Lua class/table type definition
#[derive(Debug, Clone)]
pub struct LuaClass {
    pub name: &'static str,
    pub description: &'static str,
    pub fields: &'static [LuaField],
}

/// A field in a Lua class
#[derive(Debug, Clone)]
pub struct LuaField {
    pub name: &'static str,
    pub typ: &'static str,
    pub description: &'static str,
}

// ============================================================================
// CLASS DEFINITIONS
// ============================================================================

pub static LUA_CLASSES: &[LuaClass] = &[
    LuaClass {
        name: "FileInfo",
        description: "File metadata",
        fields: &[
            LuaField {
                name: "path",
                typ: "string",
                description: "Absolute path",
            },
            LuaField {
                name: "name",
                typ: "string",
                description: "Full filename with extension",
            },
            LuaField {
                name: "stem",
                typ: "string",
                description: "Filename without extension",
            },
            LuaField {
                name: "ext",
                typ: "string",
                description: "File extension",
            },
            LuaField {
                name: "is_dir",
                typ: "boolean",
                description: "Whether this is a directory",
            },
            LuaField {
                name: "modified",
                typ: "number|nil",
                description: "Unix timestamp of last modification",
            },
        ],
    },
    LuaClass {
        name: "FrontmatterResult",
        description: "Result of reading frontmatter from a file. Frontmatter fields are merged to top level.",
        fields: &[
            LuaField {
                name: "raw",
                typ: "string",
                description: "Original file content",
            },
            LuaField {
                name: "content",
                typ: "string",
                description: "Content after frontmatter",
            },
            LuaField {
                name: "title",
                typ: "string?",
                description: "Common frontmatter field",
            },
            LuaField {
                name: "description",
                typ: "string?",
                description: "Common frontmatter field",
            },
            LuaField {
                name: "date",
                typ: "string?",
                description: "Common frontmatter field",
            },
            LuaField {
                name: "template",
                typ: "string?",
                description: "Common frontmatter field",
            },
            LuaField {
                name: "[string]",
                typ: "string|number|boolean|table|any[]",
                description: "Additional frontmatter fields (supports nested values)",
            },
        ],
    },
    LuaClass {
        name: "GitInfo",
        description: "Git repository information",
        fields: &[
            LuaField {
                name: "hash",
                typ: "string",
                description: "Full commit hash",
            },
            LuaField {
                name: "short_hash",
                typ: "string",
                description: "Short hash (7 chars)",
            },
            LuaField {
                name: "branch",
                typ: "string|nil",
                description: "Current branch name",
            },
            LuaField {
                name: "author",
                typ: "string|nil",
                description: "Commit author",
            },
            LuaField {
                name: "date",
                typ: "string|nil",
                description: "Commit date (YYYY-MM-DD)",
            },
            LuaField {
                name: "dirty",
                typ: "boolean",
                description: "Has uncommitted changes",
            },
        ],
    },
    LuaClass {
        name: "ImageDimensions",
        description: "Image dimensions",
        fields: &[
            LuaField {
                name: "width",
                typ: "number",
                description: "Image width in pixels",
            },
            LuaField {
                name: "height",
                typ: "number",
                description: "Image height in pixels",
            },
        ],
    },
    LuaClass {
        name: "ImageResizeOptions",
        description: "Options for image_resize",
        fields: &[
            LuaField {
                name: "width",
                typ: "number",
                description: "Target width in pixels",
            },
            LuaField {
                name: "height",
                typ: "number?",
                description: "Target height (maintains aspect ratio if omitted)",
            },
            LuaField {
                name: "quality",
                typ: "number?",
                description: "Quality 0-100 for lossy formats (default: 85)",
            },
        ],
    },
    LuaClass {
        name: "ImageConvertOptions",
        description: "Options for image_convert",
        fields: &[
            LuaField {
                name: "format",
                typ: "string?",
                description: "Target format: 'webp'|'png'|'jpg' (default: from extension)",
            },
            LuaField {
                name: "quality",
                typ: "number?",
                description: "Quality 0-100 for lossy formats (default: 85)",
            },
        ],
    },
    LuaClass {
        name: "ImageOptimizeOptions",
        description: "Options for image_optimize",
        fields: &[LuaField {
            name: "quality",
            typ: "number?",
            description: "Quality 0-100 (default: 85)",
        }],
    },
    LuaClass {
        name: "BuildCssOptions",
        description: "Options for build_css",
        fields: &[
            LuaField {
                name: "minify",
                typ: "boolean?",
                description: "Minify output CSS (default: false)",
            },
            LuaField {
                name: "purge",
                typ: "boolean?",
                description: "Remove unused CSS rules based on HTML output (default: false)",
            },
            LuaField {
                name: "safelist",
                typ: "string[]?",
                description: "Regex patterns for selectors to always keep",
            },
        ],
    },
    LuaClass {
        name: "BuildJsOptions",
        description: "Options for build_js",
        fields: &[LuaField {
            name: "minify",
            typ: "boolean?",
            description: "Minify output JS with dead code elimination (default: false)",
        }],
    },
    LuaClass {
        name: "CssPurgeOptions",
        description: "Options for rs.css.purge",
        fields: &[LuaField {
            name: "safelist",
            typ: "string[]?",
            description: "Regex patterns for selectors to always keep",
        }],
    },
    LuaClass {
        name: "GoogleFontOptions",
        description: "Options for download_google_font",
        fields: &[
            LuaField {
                name: "fonts_dir",
                typ: "string",
                description: "Directory to save font files (required)",
            },
            LuaField {
                name: "css_path",
                typ: "string",
                description: "Path to save CSS file (required)",
            },
            LuaField {
                name: "css_prefix",
                typ: "string?",
                description: "URL prefix for fonts in CSS (default: '/fonts')",
            },
            LuaField {
                name: "weights",
                typ: "number[]?",
                description: "Font weights to download (default: {400})",
            },
            LuaField {
                name: "display",
                typ: "string?",
                description: "Font display strategy: 'swap'|'block'|'fallback'|'optional' (default: 'swap')",
            },
            LuaField {
                name: "cache",
                typ: "boolean|string?",
                description: "Enable caching: true (auto), string (explicit path), false (disabled). Default: true",
            },
            LuaField {
                name: "minify",
                typ: "boolean?",
                description: "Minify CSS output (default: true)",
            },
        ],
    },
    LuaClass {
        name: "MarkdownEvent",
        description: "Markdown AST event for transformation",
        fields: &[
            LuaField {
                name: "type",
                typ: "string",
                description: "Event type: 'text'|'html'|'code'|'start'|'end'|'softbreak'|'hardbreak'|'rule'",
            },
            LuaField {
                name: "content",
                typ: "string|nil",
                description: "Text/HTML/code content",
            },
            LuaField {
                name: "tag",
                typ: "string|nil",
                description: "Tag name for start/end events",
            },
            LuaField {
                name: "level",
                typ: "number|nil",
                description: "Heading level (1-6)",
            },
            LuaField {
                name: "url",
                typ: "string|nil",
                description: "Link/image URL",
            },
            LuaField {
                name: "title",
                typ: "string|nil",
                description: "Link/image title",
            },
        ],
    },
    LuaClass {
        name: "MarkdownContext",
        description: "Context information during markdown parsing",
        fields: &[
            LuaField {
                name: "in_paragraph",
                typ: "boolean",
                description: "Inside paragraph",
            },
            LuaField {
                name: "in_heading",
                typ: "boolean",
                description: "Inside heading",
            },
            LuaField {
                name: "in_list",
                typ: "boolean",
                description: "Inside list",
            },
            LuaField {
                name: "in_list_item",
                typ: "boolean",
                description: "Inside list item",
            },
            LuaField {
                name: "in_blockquote",
                typ: "boolean",
                description: "Inside blockquote",
            },
            LuaField {
                name: "in_link",
                typ: "boolean",
                description: "Inside link",
            },
            LuaField {
                name: "in_emphasis",
                typ: "boolean",
                description: "Inside emphasis",
            },
            LuaField {
                name: "in_strong",
                typ: "boolean",
                description: "Inside strong",
            },
            LuaField {
                name: "in_code_block",
                typ: "boolean",
                description: "Inside code block",
            },
            LuaField {
                name: "in_table",
                typ: "boolean",
                description: "Inside table",
            },
            LuaField {
                name: "heading_level",
                typ: "number",
                description: "Current heading level (0 if not in heading)",
            },
            LuaField {
                name: "list_depth",
                typ: "number",
                description: "Nesting depth of lists",
            },
        ],
    },
    LuaClass {
        name: "CoroTask",
        description: "Coroutine task wrapper for cooperative multitasking",
        fields: &[
            LuaField {
                name: "_co",
                typ: "thread",
                description: "Internal coroutine",
            },
            LuaField {
                name: "_completed",
                typ: "boolean",
                description: "Whether task has completed",
            },
            LuaField {
                name: "_result",
                typ: "any",
                description: "Task result",
            },
        ],
    },
    LuaClass {
        name: "AsyncTask",
        description: "Async fetch task handle (await with rs.async.await)",
        fields: &[LuaField {
            name: "is_completed",
            typ: "fun(): boolean",
            description: "Check if task has completed",
        }],
    },
    LuaClass {
        name: "AsyncBytesTask",
        description: "Async bytes fetch task handle (await with rs.async.await)",
        fields: &[LuaField {
            name: "is_completed",
            typ: "fun(): boolean",
            description: "Check if task has completed",
        }],
    },
    LuaClass {
        name: "AsyncIOTask",
        description: "Async I/O task handle (await with rs.async.await)",
        fields: &[LuaField {
            name: "is_completed",
            typ: "fun(): boolean",
            description: "Check if task has completed",
        }],
    },
    LuaClass {
        name: "FetchResponse",
        description: "HTTP response from async.fetch",
        fields: &[
            LuaField {
                name: "status",
                typ: "number",
                description: "HTTP status code",
            },
            LuaField {
                name: "ok",
                typ: "boolean",
                description: "Whether request was successful (2xx)",
            },
            LuaField {
                name: "body",
                typ: "string",
                description: "Response body as string",
            },
            LuaField {
                name: "headers",
                typ: "table<string, string>",
                description: "Response headers",
            },
            LuaField {
                name: "json",
                typ: "fun(): table",
                description: "Parse body as JSON",
            },
        ],
    },
    LuaClass {
        name: "FetchOptions",
        description: "Options for async.fetch",
        fields: &[
            LuaField {
                name: "method",
                typ: "string?",
                description: "HTTP method: GET|POST|PUT|DELETE|PATCH|HEAD (default: GET)",
            },
            LuaField {
                name: "headers",
                typ: "table<string, string>?",
                description: "Request headers",
            },
            LuaField {
                name: "body",
                typ: "string|table?",
                description: "Request body (table will be JSON encoded)",
            },
            LuaField {
                name: "timeout",
                typ: "number?",
                description: "Timeout in seconds",
            },
            LuaField {
                name: "cache",
                typ: "boolean|string?",
                description: "Enable caching: true (auto), string (path), false (disabled)",
            },
        ],
    },
    LuaClass {
        name: "EncryptedData",
        description: "Encrypted content data from crypt.encrypt",
        fields: &[
            LuaField {
                name: "ciphertext",
                typ: "string",
                description: "Base64-encoded encrypted content",
            },
            LuaField {
                name: "salt",
                typ: "string",
                description: "Base64-encoded salt for key derivation",
            },
            LuaField {
                name: "nonce",
                typ: "string",
                description: "Base64-encoded nonce for decryption",
            },
        ],
    },
    LuaClass {
        name: "EncryptHtmlOptions",
        description: "Options for crypt.encrypt_html",
        fields: &[
            LuaField {
                name: "password",
                typ: "string?",
                description: "Encryption password (uses SITE_PASSWORD env if not provided)",
            },
            LuaField {
                name: "slug",
                typ: "string?",
                description: "Page slug for localStorage key (default: 'page')",
            },
            LuaField {
                name: "block_id",
                typ: "string?",
                description: "Unique block ID (auto-generated if not provided)",
            },
            LuaField {
                name: "own_password",
                typ: "boolean?",
                description: "Whether block has its own password (default: false)",
            },
        ],
    },
    LuaClass {
        name: "DirEntry",
        description: "Directory entry from async.read_dir",
        fields: &[
            LuaField {
                name: "path",
                typ: "string",
                description: "Full path to the entry",
            },
            LuaField {
                name: "name",
                typ: "string",
                description: "Entry name (filename or directory name)",
            },
            LuaField {
                name: "is_file",
                typ: "boolean",
                description: "Whether this is a file",
            },
            LuaField {
                name: "is_dir",
                typ: "boolean",
                description: "Whether this is a directory",
            },
            LuaField {
                name: "is_symlink",
                typ: "boolean",
                description: "Whether this is a symbolic link",
            },
        ],
    },
    LuaClass {
        name: "FileMetadata",
        description: "File metadata from async.metadata",
        fields: &[
            LuaField {
                name: "is_file",
                typ: "boolean",
                description: "Whether this is a file",
            },
            LuaField {
                name: "is_dir",
                typ: "boolean",
                description: "Whether this is a directory",
            },
            LuaField {
                name: "len",
                typ: "number",
                description: "File size in bytes",
            },
            LuaField {
                name: "readonly",
                typ: "boolean",
                description: "Whether file is read-only",
            },
            LuaField {
                name: "modified",
                typ: "number?",
                description: "Unix timestamp of last modification",
            },
        ],
    },
    LuaClass {
        name: "DateTable",
        description: "Date as table with year, month, day fields",
        fields: &[
            LuaField {
                name: "year",
                typ: "number",
                description: "Year (e.g., 2024)",
            },
            LuaField {
                name: "month",
                typ: "number",
                description: "Month (1-12)",
            },
            LuaField {
                name: "day",
                typ: "number",
                description: "Day of month (1-31)",
            },
        ],
    },
    LuaClass {
        name: "WriteHashedOptions",
        description: "Options for assets.write_hashed",
        fields: &[LuaField {
            name: "hash_length",
            typ: "number?",
            description: "Hash length in characters (default: 8)",
        }],
    },
    // PWA
    LuaClass {
        name: "ManifestIcon",
        description: "Icon definition for web app manifest",
        fields: &[
            LuaField {
                name: "src",
                typ: "string",
                description: "Icon image path",
            },
            LuaField {
                name: "sizes",
                typ: "string",
                description: "Icon dimensions (e.g., '192x192')",
            },
            LuaField {
                name: "type",
                typ: "string?",
                description: "MIME type (default: 'image/png')",
            },
            LuaField {
                name: "purpose",
                typ: "string?",
                description: "Icon purpose (e.g., 'any', 'maskable')",
            },
        ],
    },
    LuaClass {
        name: "ManifestOptions",
        description: "Options for pwa.manifest",
        fields: &[
            LuaField {
                name: "name",
                typ: "string",
                description: "Full app name",
            },
            LuaField {
                name: "short_name",
                typ: "string",
                description: "Short app name for home screen",
            },
            LuaField {
                name: "description",
                typ: "string?",
                description: "App description",
            },
            LuaField {
                name: "start_url",
                typ: "string?",
                description: "Start URL (default: '/')",
            },
            LuaField {
                name: "display",
                typ: "string?",
                description: "Display mode (default: 'standalone')",
            },
            LuaField {
                name: "background_color",
                typ: "string?",
                description: "Background color (default: '#ffffff')",
            },
            LuaField {
                name: "theme_color",
                typ: "string?",
                description: "Theme color (default: '#000000')",
            },
            LuaField {
                name: "icons",
                typ: "ManifestIcon[]?",
                description: "App icons",
            },
            LuaField {
                name: "output",
                typ: "string",
                description: "Output path for manifest.json",
            },
        ],
    },
    LuaClass {
        name: "ServiceWorkerOptions",
        description: "Options for pwa.service_worker",
        fields: &[
            LuaField {
                name: "cache_name",
                typ: "string",
                description: "Cache name prefix",
            },
            LuaField {
                name: "version",
                typ: "string?",
                description: "Cache version (default: '1')",
            },
            LuaField {
                name: "precache",
                typ: "string[]?",
                description: "URLs to precache on install",
            },
            LuaField {
                name: "network_first",
                typ: "string[]?",
                description: "URL patterns for network-first strategy",
            },
            LuaField {
                name: "cache_first",
                typ: "string[]?",
                description: "URL patterns for cache-first strategy",
            },
            LuaField {
                name: "offline_page",
                typ: "string?",
                description: "URL of offline fallback page",
            },
            LuaField {
                name: "output",
                typ: "string",
                description: "Output path for sw.js",
            },
        ],
    },
    // SEO
    LuaClass {
        name: "SitemapPage",
        description: "Page entry for sitemap",
        fields: &[
            LuaField {
                name: "path",
                typ: "string",
                description: "Page URL path",
            },
            LuaField {
                name: "lastmod",
                typ: "string?",
                description: "Last modification date (YYYY-MM-DD)",
            },
            LuaField {
                name: "changefreq",
                typ: "string?",
                description: "Change frequency (always|hourly|daily|weekly|monthly|yearly|never)",
            },
            LuaField {
                name: "priority",
                typ: "number?",
                description: "Priority 0.0-1.0",
            },
        ],
    },
    LuaClass {
        name: "SitemapOptions",
        description: "Options for seo.sitemap",
        fields: &[
            LuaField {
                name: "base_url",
                typ: "string",
                description: "Base URL for sitemap (e.g., 'https://example.com')",
            },
            LuaField {
                name: "pages",
                typ: "(string|SitemapPage)[]",
                description: "Array of page paths or page objects",
            },
            LuaField {
                name: "output",
                typ: "string",
                description: "Output path for sitemap.xml",
            },
            LuaField {
                name: "default_changefreq",
                typ: "string?",
                description: "Default change frequency for pages",
            },
            LuaField {
                name: "default_priority",
                typ: "number?",
                description: "Default priority for pages (0.0-1.0)",
            },
            LuaField {
                name: "exclude",
                typ: "string[]?",
                description: "Patterns to exclude from sitemap",
            },
        ],
    },
    LuaClass {
        name: "RobotsOptions",
        description: "Options for seo.robots",
        fields: &[
            LuaField {
                name: "output",
                typ: "string",
                description: "Output path for robots.txt",
            },
            LuaField {
                name: "sitemap_url",
                typ: "string?",
                description: "URL to sitemap.xml",
            },
            LuaField {
                name: "user_agent",
                typ: "string?",
                description: "User agent (default: '*')",
            },
            LuaField {
                name: "allow",
                typ: "string[]?",
                description: "Allowed paths",
            },
            LuaField {
                name: "disallow",
                typ: "string[]?",
                description: "Disallowed paths",
            },
        ],
    },
];

// ============================================================================
// FUNCTION DEFINITIONS
// ============================================================================

pub static LUA_FUNCTIONS: &[LuaFunction] = &[
    // FILE OPERATIONS
    LuaFunction {
        name: "read_file",
        module: None,
        description: "Read file contents as string",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to file",
            optional: false,
        }],
        returns: "string|nil",
    },
    LuaFunction {
        name: "write_file",
        module: None,
        description: "Write content to file",
        params: &[
            LuaParam {
                name: "path",
                typ: "string",
                description: "Path to write to",
                optional: false,
            },
            LuaParam {
                name: "content",
                typ: "string",
                description: "Content to write",
                optional: false,
            },
        ],
        returns: "boolean",
    },
    LuaFunction {
        name: "copy_file",
        module: None,
        description: "Copy a file (works with binary files)",
        params: &[
            LuaParam {
                name: "src",
                typ: "string",
                description: "Source file path",
                optional: false,
            },
            LuaParam {
                name: "dest",
                typ: "string",
                description: "Destination file path",
                optional: false,
            },
        ],
        returns: "boolean",
    },
    LuaFunction {
        name: "file_exists",
        module: None,
        description: "Check if file exists",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to check",
            optional: false,
        }],
        returns: "boolean",
    },
    LuaFunction {
        name: "list_files",
        module: None,
        description: "List files in directory matching pattern",
        params: &[
            LuaParam {
                name: "path",
                typ: "string",
                description: "Directory to search",
                optional: false,
            },
            LuaParam {
                name: "pattern",
                typ: "string",
                description: "Glob pattern (default: '*')",
                optional: true,
            },
        ],
        returns: "FileInfo[]",
    },
    LuaFunction {
        name: "list_dirs",
        module: None,
        description: "List subdirectories",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Directory to search",
            optional: false,
        }],
        returns: "string[]",
    },
    LuaFunction {
        name: "load_json",
        module: None,
        description: "Load and parse JSON file",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to JSON file",
            optional: false,
        }],
        returns: "table<string, any>|nil",
    },
    LuaFunction {
        name: "load_yaml",
        module: None,
        description: "Load and parse YAML file",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to YAML file",
            optional: false,
        }],
        returns: "table<string, any>|nil",
    },
    LuaFunction {
        name: "load_toml",
        module: None,
        description: "Load and parse TOML file",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to TOML file",
            optional: false,
        }],
        returns: "table<string, any>|nil",
    },
    LuaFunction {
        name: "read_frontmatter",
        module: None,
        description: "Read and parse frontmatter from markdown file",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to markdown file",
            optional: false,
        }],
        returns: "FrontmatterResult|nil",
    },
    // SEARCH
    LuaFunction {
        name: "glob",
        module: None,
        description: "Find files matching glob pattern",
        params: &[LuaParam {
            name: "pattern",
            typ: "string",
            description: "Glob pattern (e.g., '**/*.md')",
            optional: false,
        }],
        returns: "FileInfo[]",
    },
    LuaFunction {
        name: "scan",
        module: None,
        description: "List directories in path",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Directory to scan",
            optional: false,
        }],
        returns: "FileInfo[]",
    },
    // COLLECTIONS
    LuaFunction {
        name: "filter",
        module: None,
        description: "Filter items using predicate function",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to filter",
                optional: false,
            },
            LuaParam {
                name: "fn",
                typ: "fun(item: T): boolean",
                description: "Predicate function",
                optional: false,
            },
        ],
        returns: "T[]",
    },
    LuaFunction {
        name: "sort",
        module: None,
        description: "Sort items using comparator function",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to sort",
                optional: false,
            },
            LuaParam {
                name: "fn",
                typ: "fun(a: T, b: T): boolean",
                description: "Comparator (return true if a < b)",
                optional: false,
            },
        ],
        returns: "T[]",
    },
    LuaFunction {
        name: "map",
        module: None,
        description: "Transform items using mapping function",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to transform",
                optional: false,
            },
            LuaParam {
                name: "fn",
                typ: "fun(item: T): U",
                description: "Mapping function",
                optional: false,
            },
        ],
        returns: "U[]",
    },
    LuaFunction {
        name: "find",
        module: None,
        description: "Find first item where predicate returns true",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to search",
                optional: false,
            },
            LuaParam {
                name: "fn",
                typ: "fun(item: T): boolean",
                description: "Predicate function",
                optional: false,
            },
        ],
        returns: "T|nil",
    },
    LuaFunction {
        name: "group_by",
        module: None,
        description: "Group items by key returned by key function",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to group",
                optional: false,
            },
            LuaParam {
                name: "fn",
                typ: "fun(item: T): string",
                description: "Key function",
                optional: false,
            },
        ],
        returns: "table<string, T[]>",
    },
    LuaFunction {
        name: "unique",
        module: None,
        description: "Remove duplicates from array",
        params: &[LuaParam {
            name: "items",
            typ: "T[]",
            description: "Items to deduplicate",
            optional: false,
        }],
        returns: "T[]",
    },
    LuaFunction {
        name: "reverse",
        module: None,
        description: "Reverse array order",
        params: &[LuaParam {
            name: "items",
            typ: "T[]",
            description: "Items to reverse",
            optional: false,
        }],
        returns: "T[]",
    },
    LuaFunction {
        name: "take",
        module: None,
        description: "Take first n items from array",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to take from",
                optional: false,
            },
            LuaParam {
                name: "n",
                typ: "number",
                description: "Number of items to take",
                optional: false,
            },
        ],
        returns: "T[]",
    },
    LuaFunction {
        name: "skip",
        module: None,
        description: "Skip first n items from array",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to skip from",
                optional: false,
            },
            LuaParam {
                name: "n",
                typ: "number",
                description: "Number of items to skip",
                optional: false,
            },
        ],
        returns: "T[]",
    },
    LuaFunction {
        name: "keys",
        module: None,
        description: "Get all keys from a table",
        params: &[LuaParam {
            name: "table",
            typ: "table<K, V>",
            description: "Table to get keys from",
            optional: false,
        }],
        returns: "K[]",
    },
    LuaFunction {
        name: "values",
        module: None,
        description: "Get all values from a table",
        params: &[LuaParam {
            name: "table",
            typ: "table<K, V>",
            description: "Table to get values from",
            optional: false,
        }],
        returns: "V[]",
    },
    // TEXT
    LuaFunction {
        name: "slugify",
        module: None,
        description: "Convert text to URL-friendly slug",
        params: &[LuaParam {
            name: "text",
            typ: "string",
            description: "Text to slugify",
            optional: false,
        }],
        returns: "string",
    },
    LuaFunction {
        name: "word_count",
        module: None,
        description: "Count words in text",
        params: &[LuaParam {
            name: "text",
            typ: "string",
            description: "Text to count",
            optional: false,
        }],
        returns: "number",
    },
    LuaFunction {
        name: "reading_time",
        module: None,
        description: "Calculate reading time in minutes",
        params: &[
            LuaParam {
                name: "text",
                typ: "string",
                description: "Text to analyze",
                optional: false,
            },
            LuaParam {
                name: "wpm",
                typ: "number",
                description: "Words per minute (default: 200)",
                optional: true,
            },
        ],
        returns: "number",
    },
    LuaFunction {
        name: "truncate",
        module: None,
        description: "Truncate text with optional suffix",
        params: &[
            LuaParam {
                name: "text",
                typ: "string",
                description: "Text to truncate",
                optional: false,
            },
            LuaParam {
                name: "len",
                typ: "number",
                description: "Maximum length",
                optional: false,
            },
            LuaParam {
                name: "suffix",
                typ: "string",
                description: "Suffix to append (default: '...')",
                optional: true,
            },
        ],
        returns: "string",
    },
    LuaFunction {
        name: "strip_tags",
        module: None,
        description: "Remove HTML tags from string",
        params: &[LuaParam {
            name: "html",
            typ: "string",
            description: "HTML content",
            optional: false,
        }],
        returns: "string",
    },
    LuaFunction {
        name: "format_date",
        module: None,
        description: "Format a date string",
        params: &[
            LuaParam {
                name: "date",
                typ: "string|DateTable",
                description: "Date string (YYYY-MM-DD) or table {year, month, day}",
                optional: false,
            },
            LuaParam {
                name: "format",
                typ: "string",
                description: "Output format (chrono strftime)",
                optional: false,
            },
        ],
        returns: "string|nil",
    },
    LuaFunction {
        name: "parse_date",
        module: None,
        description: "Parse date string to table",
        params: &[LuaParam {
            name: "date_str",
            typ: "string",
            description: "Date string to parse",
            optional: false,
        }],
        returns: "DateTable|nil",
    },
    LuaFunction {
        name: "hash",
        module: None,
        description: "Hash content using xxHash64",
        params: &[LuaParam {
            name: "content",
            typ: "string",
            description: "Content to hash",
            optional: false,
        }],
        returns: "string",
    },
    LuaFunction {
        name: "hash_file",
        module: None,
        description: "Hash file contents using xxHash64",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to file",
            optional: false,
        }],
        returns: "string|nil",
    },
    LuaFunction {
        name: "url_encode",
        module: None,
        description: "URL encode a string",
        params: &[LuaParam {
            name: "text",
            typ: "string",
            description: "Text to encode",
            optional: false,
        }],
        returns: "string",
    },
    LuaFunction {
        name: "url_decode",
        module: None,
        description: "URL decode a string",
        params: &[LuaParam {
            name: "text",
            typ: "string",
            description: "Text to decode",
            optional: false,
        }],
        returns: "string",
    },
    // PATH UTILITIES
    LuaFunction {
        name: "join_path",
        module: None,
        description: "Join path segments",
        params: &[LuaParam {
            name: "...",
            typ: "string",
            description: "Path segments to join",
            optional: false,
        }],
        returns: "string",
    },
    LuaFunction {
        name: "basename",
        module: None,
        description: "Get file name from path",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "File path",
            optional: false,
        }],
        returns: "string",
    },
    LuaFunction {
        name: "dirname",
        module: None,
        description: "Get directory from path",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "File path",
            optional: false,
        }],
        returns: "string",
    },
    LuaFunction {
        name: "extension",
        module: None,
        description: "Get file extension from path",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "File path",
            optional: false,
        }],
        returns: "string",
    },
    // ENV
    LuaFunction {
        name: "env",
        module: None,
        description: "Get environment variable",
        params: &[LuaParam {
            name: "name",
            typ: "string",
            description: "Variable name",
            optional: false,
        }],
        returns: "string|nil",
    },
    LuaFunction {
        name: "print",
        module: None,
        description: "Log message to build output",
        params: &[LuaParam {
            name: "...",
            typ: "any",
            description: "Values to print",
            optional: false,
        }],
        returns: "nil",
    },
    LuaFunction {
        name: "is_gitignored",
        module: None,
        description: "Check if path is gitignored",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to check",
            optional: false,
        }],
        returns: "boolean",
    },
    // GIT
    LuaFunction {
        name: "git_info",
        module: None,
        description: "Get git information for repo or file",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "File/directory path (default: repo root)",
            optional: true,
        }],
        returns: "GitInfo|nil",
    },
    // CONTENT
    LuaFunction {
        name: "render_markdown",
        module: None,
        description: "Render markdown to HTML with optional AST transformation",
        params: &[
            LuaParam {
                name: "content",
                typ: "string",
                description: "Markdown content",
                optional: false,
            },
            LuaParam {
                name: "transform_fn",
                typ: "fun(event: MarkdownEvent, ctx: MarkdownContext): MarkdownEvent|nil",
                description: "Optional transform function",
                optional: true,
            },
        ],
        returns: "string",
    },
    LuaFunction {
        name: "rss_date",
        module: None,
        description: "Format date for RSS (RFC 2822)",
        params: &[LuaParam {
            name: "date_string",
            typ: "string",
            description: "Date string (YYYY-MM-DD)",
            optional: false,
        }],
        returns: "string|nil",
    },
    LuaFunction {
        name: "extract_links_markdown",
        module: None,
        description: "Extract links from markdown content",
        params: &[LuaParam {
            name: "content",
            typ: "string",
            description: "Markdown content",
            optional: false,
        }],
        returns: "string[]",
    },
    LuaFunction {
        name: "extract_links_html",
        module: None,
        description: "Extract links from HTML content",
        params: &[LuaParam {
            name: "content",
            typ: "string",
            description: "HTML content",
            optional: false,
        }],
        returns: "string[]",
    },
    LuaFunction {
        name: "extract_images_markdown",
        module: None,
        description: "Extract image paths from markdown",
        params: &[LuaParam {
            name: "content",
            typ: "string",
            description: "Markdown content",
            optional: false,
        }],
        returns: "string[]",
    },
    LuaFunction {
        name: "extract_images_html",
        module: None,
        description: "Extract image paths from HTML",
        params: &[LuaParam {
            name: "content",
            typ: "string",
            description: "HTML content",
            optional: false,
        }],
        returns: "string[]",
    },
    LuaFunction {
        name: "html_to_text",
        module: None,
        description: "Convert HTML to formatted plain text",
        params: &[LuaParam {
            name: "html",
            typ: "string",
            description: "HTML content",
            optional: false,
        }],
        returns: "string",
    },
    // ASSETS
    LuaFunction {
        name: "build_css",
        module: None,
        description: "Build and concatenate CSS files from glob pattern or array of paths (async)",
        params: &[
            LuaParam {
                name: "paths_or_pattern",
                typ: "string|string[]",
                description: "Glob pattern (e.g., 'styles/*.css') or array of file paths",
                optional: false,
            },
            LuaParam {
                name: "output_path",
                typ: "string",
                description: "Output file path",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "BuildCssOptions",
                description: "Build options (minify)",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
    },
    LuaFunction {
        name: "build_js",
        module: None,
        description: "Build and concatenate JS files with minification and dead code elimination (async)",
        params: &[
            LuaParam {
                name: "paths_or_pattern",
                typ: "string|string[]",
                description: "Glob pattern (e.g., 'scripts/*.js') or array of file paths",
                optional: false,
            },
            LuaParam {
                name: "output_path",
                typ: "string",
                description: "Output file path",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "BuildJsOptions",
                description: "Build options (minify)",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
    },
    LuaFunction {
        name: "check_unused_assets",
        module: None,
        description: "Find assets in output directory not referenced by any HTML file",
        params: &[LuaParam {
            name: "output_dir",
            typ: "string",
            description: "Output directory to check",
            optional: false,
        }],
        returns: "string[]",
    },
    LuaFunction {
        name: "download_google_font",
        module: None,
        description: "Download Google Font files and generate local CSS (async, returns handle)",
        params: &[
            LuaParam {
                name: "family",
                typ: "string",
                description: "Font family name (e.g., 'Lexend', 'Open Sans')",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "GoogleFontOptions",
                description: "Font options (fonts_dir, css_path required)",
                optional: false,
            },
        ],
        returns: "AsyncIOTask",
    },
    // IMAGES
    LuaFunction {
        name: "image_dimensions",
        module: None,
        description: "Get image dimensions",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to image",
            optional: false,
        }],
        returns: "ImageDimensions|nil",
    },
    LuaFunction {
        name: "image_resize",
        module: None,
        description: "Resize image",
        params: &[
            LuaParam {
                name: "input",
                typ: "string",
                description: "Input image path",
                optional: false,
            },
            LuaParam {
                name: "output",
                typ: "string",
                description: "Output image path",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "ImageResizeOptions",
                description: "Resize options (width required, height/quality optional)",
                optional: false,
            },
        ],
        returns: "boolean",
    },
    LuaFunction {
        name: "image_convert",
        module: None,
        description: "Convert image format",
        params: &[
            LuaParam {
                name: "input",
                typ: "string",
                description: "Input image path",
                optional: false,
            },
            LuaParam {
                name: "output",
                typ: "string",
                description: "Output image path",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "ImageConvertOptions",
                description: "Convert options (format/quality)",
                optional: true,
            },
        ],
        returns: "boolean",
    },
    LuaFunction {
        name: "image_optimize",
        module: None,
        description: "Optimize/compress image",
        params: &[
            LuaParam {
                name: "input",
                typ: "string",
                description: "Input image path",
                optional: false,
            },
            LuaParam {
                name: "output",
                typ: "string",
                description: "Output image path",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "ImageOptimizeOptions",
                description: "Optimize options (quality)",
                optional: true,
            },
        ],
        returns: "boolean",
    },
    // CORO MODULE
    LuaFunction {
        name: "task",
        module: Some("coro"),
        description: "Create coroutine task from function",
        params: &[LuaParam {
            name: "fn",
            typ: "function",
            description: "Function to wrap",
            optional: false,
        }],
        returns: "CoroTask",
    },
    LuaFunction {
        name: "await",
        module: Some("coro"),
        description: "Run task to completion",
        params: &[LuaParam {
            name: "task",
            typ: "CoroTask",
            description: "Task to run",
            optional: false,
        }],
        returns: "any",
    },
    LuaFunction {
        name: "yield",
        module: Some("coro"),
        description: "Yield from current task",
        params: &[LuaParam {
            name: "value",
            typ: "any",
            description: "Value to yield",
            optional: true,
        }],
        returns: "any",
    },
    LuaFunction {
        name: "all",
        module: Some("coro"),
        description: "Run multiple tasks concurrently",
        params: &[LuaParam {
            name: "tasks",
            typ: "CoroTask[]",
            description: "Tasks to run",
            optional: false,
        }],
        returns: "any[]",
    },
    LuaFunction {
        name: "race",
        module: Some("coro"),
        description: "Run tasks and return first completed",
        params: &[LuaParam {
            name: "tasks",
            typ: "CoroTask[]",
            description: "Tasks to race",
            optional: false,
        }],
        returns: "any",
    },
    LuaFunction {
        name: "sleep",
        module: Some("coro"),
        description: "Sleep/delay (yields N times for cooperative scheduling)",
        params: &[LuaParam {
            name: "n",
            typ: "number",
            description: "Number of yields",
            optional: true,
        }],
        returns: "nil",
    },
    // PARALLEL MODULE
    LuaFunction {
        name: "load_json",
        module: Some("parallel"),
        description: "Load multiple JSON files in parallel",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Paths to load",
            optional: false,
        }],
        returns: "(table<string, any>|nil)[]",
    },
    LuaFunction {
        name: "read_files",
        module: Some("parallel"),
        description: "Read multiple files in parallel",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Paths to read",
            optional: false,
        }],
        returns: "(string|nil)[]",
    },
    LuaFunction {
        name: "file_exists",
        module: Some("parallel"),
        description: "Check multiple files exist in parallel",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Paths to check",
            optional: false,
        }],
        returns: "boolean[]",
    },
    LuaFunction {
        name: "load_yaml",
        module: Some("parallel"),
        description: "Load multiple YAML files in parallel",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Paths to load",
            optional: false,
        }],
        returns: "(table<string, any>|nil)[]",
    },
    LuaFunction {
        name: "read_frontmatter",
        module: Some("parallel"),
        description: "Parse frontmatter from multiple files in parallel",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Paths to parse",
            optional: false,
        }],
        returns: "(FrontmatterResult|nil)[]",
    },
    LuaFunction {
        name: "map",
        module: Some("parallel"),
        description: "Map over items (structure for parallel-ready code)",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to map",
                optional: false,
            },
            LuaParam {
                name: "fn",
                typ: "fun(item: T): U",
                description: "Mapping function",
                optional: false,
            },
        ],
        returns: "U[]",
    },
    LuaFunction {
        name: "filter",
        module: Some("parallel"),
        description: "Filter items using predicate",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to filter",
                optional: false,
            },
            LuaParam {
                name: "fn",
                typ: "fun(item: T): boolean",
                description: "Predicate function",
                optional: false,
            },
        ],
        returns: "T[]",
    },
    LuaFunction {
        name: "reduce",
        module: Some("parallel"),
        description: "Reduce items to single value",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to reduce",
                optional: false,
            },
            LuaParam {
                name: "initial",
                typ: "U",
                description: "Initial accumulator value",
                optional: false,
            },
            LuaParam {
                name: "fn",
                typ: "fun(acc: U, item: T): U",
                description: "Reducer function",
                optional: false,
            },
        ],
        returns: "U",
    },
    LuaFunction {
        name: "create_dirs",
        module: Some("parallel"),
        description: "Create multiple directories in parallel",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Directory paths to create",
            optional: false,
        }],
        returns: "(boolean|string)[]",
    },
    LuaFunction {
        name: "copy_files",
        module: Some("parallel"),
        description: "Copy multiple files in parallel",
        params: &[
            LuaParam {
                name: "sources",
                typ: "string[]",
                description: "Source file paths",
                optional: false,
            },
            LuaParam {
                name: "destinations",
                typ: "string[]",
                description: "Destination file paths",
                optional: false,
            },
        ],
        returns: "(boolean|string)[]",
    },
    LuaFunction {
        name: "image_convert",
        module: Some("parallel"),
        description: "Convert multiple images in parallel",
        params: &[
            LuaParam {
                name: "sources",
                typ: "string[]",
                description: "Source image paths",
                optional: false,
            },
            LuaParam {
                name: "destinations",
                typ: "string[]",
                description: "Destination image paths",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "ImageConvertOptions",
                description: "Convert options (quality)",
                optional: true,
            },
        ],
        returns: "(boolean|string)[]",
    },
    // ASYNC MODULE (tokio-backed) - all return handles, await with rs.async.await()
    LuaFunction {
        name: "fetch",
        module: Some("async"),
        description: "Fetch URL (returns handle, await to get response)",
        params: &[
            LuaParam {
                name: "url",
                typ: "string",
                description: "URL to fetch",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "FetchOptions",
                description: "Request options",
                optional: true,
            },
        ],
        returns: "AsyncTask",
    },
    LuaFunction {
        name: "fetch_sync",
        module: Some("async"),
        description: "Fetch URL (blocking, returns response directly)",
        params: &[
            LuaParam {
                name: "url",
                typ: "string",
                description: "URL to fetch",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "FetchOptions",
                description: "Request options",
                optional: true,
            },
        ],
        returns: "FetchResponse",
    },
    LuaFunction {
        name: "fetch_json",
        module: Some("async"),
        description: "Fetch URL and parse as JSON",
        params: &[
            LuaParam {
                name: "url",
                typ: "string",
                description: "URL to fetch",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "FetchOptions",
                description: "Request options",
                optional: true,
            },
        ],
        returns: "table",
    },
    LuaFunction {
        name: "fetch_all",
        module: Some("async"),
        description: "Fetch multiple URLs concurrently",
        params: &[LuaParam {
            name: "requests",
            typ: "(string|{url: string, options?: FetchOptions})[]",
            description: "URLs or request objects",
            optional: false,
        }],
        returns: "FetchResponse[]",
    },
    LuaFunction {
        name: "spawn",
        module: Some("async"),
        description: "Spawn async fetch task for later await",
        params: &[
            LuaParam {
                name: "url",
                typ: "string",
                description: "URL to fetch",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "FetchOptions",
                description: "Request options",
                optional: true,
            },
        ],
        returns: "AsyncTask",
    },
    LuaFunction {
        name: "await",
        module: Some("async"),
        description: "Await spawned async task",
        params: &[LuaParam {
            name: "task",
            typ: "AsyncTask",
            description: "Task to await",
            optional: false,
        }],
        returns: "FetchResponse",
    },
    LuaFunction {
        name: "await_all",
        module: Some("async"),
        description: "Await multiple async tasks",
        params: &[LuaParam {
            name: "tasks",
            typ: "AsyncTask[]",
            description: "Tasks to await",
            optional: false,
        }],
        returns: "FetchResponse[]",
    },
    LuaFunction {
        name: "read_file",
        module: Some("async"),
        description: "Read file (returns handle)",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "File path",
            optional: false,
        }],
        returns: "AsyncIOTask",
    },
    LuaFunction {
        name: "write_file",
        module: Some("async"),
        description: "Write file (returns handle)",
        params: &[
            LuaParam {
                name: "path",
                typ: "string",
                description: "File path",
                optional: false,
            },
            LuaParam {
                name: "content",
                typ: "string",
                description: "Content to write",
                optional: false,
            },
        ],
        returns: "AsyncIOTask",
    },
    LuaFunction {
        name: "read_files",
        module: Some("async"),
        description: "Read multiple files concurrently",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "File paths",
            optional: false,
        }],
        returns: "(string|nil)[]",
    },
    LuaFunction {
        name: "load_json",
        module: Some("async"),
        description: "Load and parse JSON file asynchronously",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "JSON file path",
            optional: false,
        }],
        returns: "table",
    },
    LuaFunction {
        name: "copy_file",
        module: Some("async"),
        description: "Copy file (returns handle)",
        params: &[
            LuaParam {
                name: "src",
                typ: "string",
                description: "Source path",
                optional: false,
            },
            LuaParam {
                name: "dst",
                typ: "string",
                description: "Destination path",
                optional: false,
            },
        ],
        returns: "AsyncIOTask",
    },
    LuaFunction {
        name: "rename",
        module: Some("async"),
        description: "Rename/move file or directory asynchronously",
        params: &[
            LuaParam {
                name: "src",
                typ: "string",
                description: "Source path",
                optional: false,
            },
            LuaParam {
                name: "dst",
                typ: "string",
                description: "Destination path",
                optional: false,
            },
        ],
        returns: "boolean",
    },
    LuaFunction {
        name: "create_dir",
        module: Some("async"),
        description: "Create directory including parents (returns handle)",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Directory path",
            optional: false,
        }],
        returns: "AsyncIOTask",
    },
    LuaFunction {
        name: "remove_file",
        module: Some("async"),
        description: "Remove file asynchronously",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "File path",
            optional: false,
        }],
        returns: "boolean",
    },
    LuaFunction {
        name: "remove_dir",
        module: Some("async"),
        description: "Remove directory recursively asynchronously",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Directory path",
            optional: false,
        }],
        returns: "boolean",
    },
    LuaFunction {
        name: "exists",
        module: Some("async"),
        description: "Check if file/directory exists (returns handle)",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to check",
            optional: false,
        }],
        returns: "AsyncIOTask",
    },
    LuaFunction {
        name: "metadata",
        module: Some("async"),
        description: "Get file metadata asynchronously",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "File path",
            optional: false,
        }],
        returns: "FileMetadata",
    },
    LuaFunction {
        name: "read",
        module: Some("async"),
        description: "Read file as binary data asynchronously",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "File path",
            optional: false,
        }],
        returns: "string",
    },
    LuaFunction {
        name: "write",
        module: Some("async"),
        description: "Write binary data to file (returns handle)",
        params: &[
            LuaParam {
                name: "path",
                typ: "string",
                description: "File path",
                optional: false,
            },
            LuaParam {
                name: "data",
                typ: "string",
                description: "Binary data to write",
                optional: false,
            },
        ],
        returns: "AsyncIOTask",
    },
    LuaFunction {
        name: "fetch_bytes",
        module: Some("async"),
        description: "Fetch URL binary (returns handle)",
        params: &[
            LuaParam {
                name: "url",
                typ: "string",
                description: "URL to fetch",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "FetchOptions",
                description: "Request options",
                optional: true,
            },
        ],
        returns: "AsyncBytesTask",
    },
    LuaFunction {
        name: "read_dir",
        module: Some("async"),
        description: "List directory contents asynchronously",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Directory path",
            optional: false,
        }],
        returns: "DirEntry[]",
    },
    LuaFunction {
        name: "canonicalize",
        module: Some("async"),
        description: "Get canonical/absolute path asynchronously",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to canonicalize",
            optional: false,
        }],
        returns: "string",
    },
    // CRYPT MODULE
    LuaFunction {
        name: "encrypt",
        module: Some("crypt"),
        description: "Encrypt content using AES-256-GCM",
        params: &[
            LuaParam {
                name: "content",
                typ: "string",
                description: "Content to encrypt",
                optional: false,
            },
            LuaParam {
                name: "password",
                typ: "string",
                description: "Encryption password (uses SITE_PASSWORD env if not provided)",
                optional: true,
            },
        ],
        returns: "EncryptedData",
    },
    LuaFunction {
        name: "decrypt",
        module: Some("crypt"),
        description: "Decrypt content using AES-256-GCM",
        params: &[
            LuaParam {
                name: "data",
                typ: "EncryptedData",
                description: "Encrypted data table with ciphertext, salt, nonce",
                optional: false,
            },
            LuaParam {
                name: "password",
                typ: "string",
                description: "Decryption password (uses SITE_PASSWORD env if not provided)",
                optional: true,
            },
        ],
        returns: "string",
    },
    LuaFunction {
        name: "encrypt_html",
        module: Some("crypt"),
        description: "Encrypt content and wrap in HTML container for browser decryption",
        params: &[
            LuaParam {
                name: "content",
                typ: "string",
                description: "Content to encrypt (can be HTML)",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "EncryptHtmlOptions",
                description: "Encryption options",
                optional: true,
            },
        ],
        returns: "string",
    },
    // ASSETS MODULE
    LuaFunction {
        name: "hash",
        module: Some("assets"),
        description: "Compute SHA256 hash of content (async)",
        params: &[
            LuaParam {
                name: "content",
                typ: "string",
                description: "Content to hash",
                optional: false,
            },
            LuaParam {
                name: "length",
                typ: "number",
                description: "Hash length in characters (default: 8)",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
    },
    LuaFunction {
        name: "hash_sync",
        module: Some("assets"),
        description: "Compute SHA256 hash of content synchronously",
        params: &[
            LuaParam {
                name: "content",
                typ: "string",
                description: "Content to hash",
                optional: false,
            },
            LuaParam {
                name: "length",
                typ: "number",
                description: "Hash length in characters (default: 8)",
                optional: true,
            },
        ],
        returns: "string",
    },
    LuaFunction {
        name: "write_hashed",
        module: Some("assets"),
        description: "Write file with content-hashed filename",
        params: &[
            LuaParam {
                name: "content",
                typ: "string",
                description: "File content",
                optional: false,
            },
            LuaParam {
                name: "path",
                typ: "string",
                description: "Original file path (e.g., 'styles/main.css')",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "WriteHashedOptions",
                description: "Options (hash_length)",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
    },
    LuaFunction {
        name: "register",
        module: Some("assets"),
        description: "Register original → hashed path mapping",
        params: &[
            LuaParam {
                name: "original",
                typ: "string",
                description: "Original path (e.g., '/styles/main.css')",
                optional: false,
            },
            LuaParam {
                name: "hashed",
                typ: "string",
                description: "Hashed path (e.g., '/styles/main.a1b2c3d4.css')",
                optional: false,
            },
        ],
        returns: "nil",
    },
    LuaFunction {
        name: "get_path",
        module: Some("assets"),
        description: "Get hashed path for original (returns original if not found)",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Original path",
            optional: false,
        }],
        returns: "string",
    },
    LuaFunction {
        name: "manifest",
        module: Some("assets"),
        description: "Get table of all path mappings",
        params: &[],
        returns: "table<string, string>",
    },
    LuaFunction {
        name: "clear",
        module: Some("assets"),
        description: "Clear the asset manifest",
        params: &[],
        returns: "nil",
    },
    // CSS MODULE
    LuaFunction {
        name: "concat",
        module: Some("css"),
        description: "Concatenate CSS files with optional minification (async)",
        params: &[
            LuaParam {
                name: "paths",
                typ: "string[]",
                description: "Array of CSS file paths",
                optional: false,
            },
            LuaParam {
                name: "output",
                typ: "string",
                description: "Output file path",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "BuildCssOptions",
                description: "Build options (minify, purge, safelist)",
                optional: true,
            },
        ],
        returns: "AsyncHandle",
    },
    LuaFunction {
        name: "bundle",
        module: Some("css"),
        description: "Bundle CSS with @import resolution via LightningCSS (async)",
        params: &[
            LuaParam {
                name: "paths",
                typ: "string[]",
                description: "Array of CSS entry file paths",
                optional: false,
            },
            LuaParam {
                name: "output",
                typ: "string",
                description: "Output file path",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "BuildCssOptions",
                description: "Build options (minify, purge, safelist)",
                optional: true,
            },
        ],
        returns: "AsyncHandle",
    },
    LuaFunction {
        name: "bundle_many",
        module: Some("css"),
        description: "Bundle multiple CSS entries to separate output files (async)",
        params: &[
            LuaParam {
                name: "paths",
                typ: "string[]",
                description: "Array of CSS entry file paths",
                optional: false,
            },
            LuaParam {
                name: "output_dir",
                typ: "string",
                description: "Output directory path",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "BuildCssOptions",
                description: "Build options (minify, purge, safelist)",
                optional: true,
            },
        ],
        returns: "AsyncHandle",
    },
    LuaFunction {
        name: "purge",
        module: Some("css"),
        description: "Remove unused CSS rules based on HTML/JS output (async, call in after_build)",
        params: &[
            LuaParam {
                name: "css_path",
                typ: "string",
                description: "Path to CSS file to purge",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "CssPurgeOptions",
                description: "Purge options (safelist)",
                optional: true,
            },
        ],
        returns: "AsyncHandle",
    },
    // PWA
    LuaFunction {
        name: "manifest",
        module: Some("pwa"),
        description: "Generate a web app manifest (async)",
        params: &[LuaParam {
            name: "options",
            typ: "ManifestOptions",
            description: "Manifest options",
            optional: false,
        }],
        returns: "AsyncHandle",
    },
    LuaFunction {
        name: "service_worker",
        module: Some("pwa"),
        description: "Generate a service worker for offline caching (async)",
        params: &[LuaParam {
            name: "options",
            typ: "ServiceWorkerOptions",
            description: "Service worker options",
            optional: false,
        }],
        returns: "AsyncHandle",
    },
    // SEO MODULE
    LuaFunction {
        name: "sitemap",
        module: Some("seo"),
        description: "Generate XML sitemap (async)",
        params: &[LuaParam {
            name: "options",
            typ: "SitemapOptions",
            description: "Sitemap options",
            optional: false,
        }],
        returns: "AsyncIOTask",
    },
    LuaFunction {
        name: "robots",
        module: Some("seo"),
        description: "Generate robots.txt (async)",
        params: &[LuaParam {
            name: "options",
            typ: "RobotsOptions",
            description: "Robots options",
            optional: false,
        }],
        returns: "AsyncIOTask",
    },
];

// ============================================================================
// GENERATION FUNCTIONS
// ============================================================================

/// Generate EmmyLua type definitions
pub fn generate_emmylua() -> String {
    let mut output = String::new();
    output.push_str("---@meta rs-web\n\n");
    output.push_str("-- Auto-generated EmmyLua type definitions for rs-web\n");
    output.push_str("-- Generated with: rs-web types --lua\n");
    output.push_str("--\n");
    output.push_str("-- Usage:\n");
    output.push_str("--   local rs = require(\"rs-web\")\n");
    output.push_str("--   local content = rs.read_file(\"path/to/file.md\")\n");
    output.push_str("--   local html = rs.render_markdown(content)\n\n");

    // Generate class definitions
    output.push_str(
        "-- =============================================================================\n",
    );
    output.push_str("-- TYPE DEFINITIONS\n");
    output.push_str(
        "-- =============================================================================\n\n",
    );

    for class in LUA_CLASSES {
        output.push_str(&format!("---@class {}\n", class.name));
        for field in class.fields {
            output.push_str(&format!(
                "---@field {} {} {}\n",
                field.name, field.typ, field.description
            ));
        }
        output.push('\n');
    }

    // Group functions by module
    let mut global_fns: Vec<&LuaFunction> = Vec::new();
    let mut coro_fns: Vec<&LuaFunction> = Vec::new();
    let mut parallel_fns: Vec<&LuaFunction> = Vec::new();
    let mut async_fns: Vec<&LuaFunction> = Vec::new();
    let mut crypt_fns: Vec<&LuaFunction> = Vec::new();
    let mut assets_fns: Vec<&LuaFunction> = Vec::new();
    let mut pwa_fns: Vec<&LuaFunction> = Vec::new();
    let mut seo_fns: Vec<&LuaFunction> = Vec::new();

    for func in LUA_FUNCTIONS {
        match func.module {
            None => global_fns.push(func),
            Some("coro") => coro_fns.push(func),
            Some("parallel") => parallel_fns.push(func),
            Some("async") => async_fns.push(func),
            Some("crypt") => crypt_fns.push(func),
            Some("assets") => assets_fns.push(func),
            Some("pwa") => pwa_fns.push(func),
            Some("seo") => seo_fns.push(func),
            _ => global_fns.push(func),
        }
    }

    // Generate coro submodule class
    output.push_str(
        "-- =============================================================================\n",
    );
    output.push_str("-- CORO SUBMODULE\n");
    output.push_str(
        "-- =============================================================================\n\n",
    );
    output.push_str("---@class RsCoroModule\n");
    for func in &coro_fns {
        let params = func
            .params
            .iter()
            .map(|p| {
                let opt = if p.optional { "?" } else { "" };
                format!("{}{}: {}", p.name, opt, p.typ)
            })
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "---@field {} fun({}): {} {}\n",
            func.name, params, func.returns, func.description
        ));
    }
    output.push('\n');

    // Generate parallel submodule class
    output.push_str(
        "-- =============================================================================\n",
    );
    output.push_str("-- PARALLEL SUBMODULE\n");
    output.push_str(
        "-- =============================================================================\n\n",
    );
    output.push_str("---@class RsParallelModule\n");
    for func in &parallel_fns {
        let params = func
            .params
            .iter()
            .map(|p| {
                let opt = if p.optional { "?" } else { "" };
                format!("{}{}: {}", p.name, opt, p.typ)
            })
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "---@field {} fun({}): {} {}\n",
            func.name, params, func.returns, func.description
        ));
    }
    output.push('\n');

    // Generate async submodule class
    output.push_str(
        "-- =============================================================================\n",
    );
    output.push_str("-- ASYNC SUBMODULE (tokio-backed)\n");
    output.push_str(
        "-- =============================================================================\n\n",
    );
    output.push_str("---@class RsAsyncModule\n");
    for func in &async_fns {
        let params = func
            .params
            .iter()
            .map(|p| {
                let opt = if p.optional { "?" } else { "" };
                format!("{}{}: {}", p.name, opt, p.typ)
            })
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "---@field {} fun({}): {} {}\n",
            func.name, params, func.returns, func.description
        ));
    }
    output.push('\n');

    // Generate crypt submodule class
    output.push_str(
        "-- =============================================================================\n",
    );
    output.push_str("-- CRYPT SUBMODULE\n");
    output.push_str(
        "-- =============================================================================\n\n",
    );
    output.push_str("---@class RsCryptModule\n");
    for func in &crypt_fns {
        let params = func
            .params
            .iter()
            .map(|p| {
                let opt = if p.optional { "?" } else { "" };
                format!("{}{}: {}", p.name, opt, p.typ)
            })
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "---@field {} fun({}): {} {}\n",
            func.name, params, func.returns, func.description
        ));
    }
    output.push('\n');

    // Generate assets submodule class
    output.push_str(
        "-- =============================================================================\n",
    );
    output.push_str("-- ASSETS SUBMODULE\n");
    output.push_str(
        "-- =============================================================================\n\n",
    );
    output.push_str("---@class RsAssetsModule\n");
    for func in &assets_fns {
        let params = func
            .params
            .iter()
            .map(|p| {
                let opt = if p.optional { "?" } else { "" };
                format!("{}{}: {}", p.name, opt, p.typ)
            })
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "---@field {} fun({}): {} {}\n",
            func.name, params, func.returns, func.description
        ));
    }
    output.push('\n');

    // Generate pwa submodule class
    output.push_str(
        "-- =============================================================================\n",
    );
    output.push_str("-- PWA SUBMODULE\n");
    output.push_str(
        "-- =============================================================================\n\n",
    );
    output.push_str("---@class RsPwaModule\n");
    for func in &pwa_fns {
        let params = func
            .params
            .iter()
            .map(|p| {
                let opt = if p.optional { "?" } else { "" };
                format!("{}{}: {}", p.name, opt, p.typ)
            })
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "---@field {} fun({}): {} {}\n",
            func.name, params, func.returns, func.description
        ));
    }
    output.push('\n');

    // Generate seo submodule class
    output.push_str(
        "-- =============================================================================\n",
    );
    output.push_str("-- SEO SUBMODULE\n");
    output.push_str(
        "-- =============================================================================\n\n",
    );
    output.push_str("---@class RsSeoModule\n");
    for func in &seo_fns {
        let params = func
            .params
            .iter()
            .map(|p| {
                let opt = if p.optional { "?" } else { "" };
                format!("{}{}: {}", p.name, opt, p.typ)
            })
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "---@field {} fun({}): {} {}\n",
            func.name, params, func.returns, func.description
        ));
    }
    output.push('\n');

    // Generate main rs module class
    output.push_str(
        "-- =============================================================================\n",
    );
    output.push_str("-- RS-WEB MODULE\n");
    output.push_str(
        "-- =============================================================================\n\n",
    );
    output.push_str("---@class RsWebModule\n");
    output.push_str("---@field _VERSION string Module version\n");
    output.push_str("---@field _SANDBOX boolean Whether sandbox mode is enabled\n");
    output.push_str("---@field _PROJECT_ROOT string Project root directory\n");
    output.push_str("---@field coro RsCoroModule Coroutine helpers\n");
    output.push_str("---@field parallel RsParallelModule Parallel processing functions\n");
    output.push_str("---@field async RsAsyncModule Async I/O functions (tokio-backed)\n");
    output.push_str("---@field crypt RsCryptModule Encryption functions (AES-256-GCM)\n");
    output.push_str("---@field assets RsAssetsModule Asset hashing and manifest\n");
    output.push_str("---@field pwa RsPwaModule PWA (manifest and service worker)\n");
    output.push_str("---@field seo RsSeoModule SEO (sitemap and robots.txt)\n");

    // Add all global functions as fields
    for func in &global_fns {
        let params = func
            .params
            .iter()
            .map(|p| {
                let opt = if p.optional { "?" } else { "" };
                format!("{}{}: {}", p.name, opt, p.typ)
            })
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!(
            "---@field {} fun({}): {} {}\n",
            func.name, params, func.returns, func.description
        ));
    }
    output.push('\n');

    // Create the rs table
    output.push_str("---The rs-web module\n");
    output.push_str("---@type RsWebModule\n");
    output.push_str("local rs = {}\n\n");

    // Generate function stubs for better IDE completion
    output.push_str(
        "-- =============================================================================\n",
    );
    output.push_str("-- FUNCTION STUBS (for IDE completion)\n");
    output.push_str(
        "-- =============================================================================\n\n",
    );

    for func in &global_fns {
        output.push_str(&format!("---{}\n", func.description));
        for param in func.params {
            let opt = if param.optional { "?" } else { "" };
            output.push_str(&format!(
                "---@param {}{} {} {}\n",
                param.name, opt, param.typ, param.description
            ));
        }
        output.push_str(&format!("---@return {}\n", func.returns));

        let params = func
            .params
            .iter()
            .map(|p| p.name.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        output.push_str(&format!("function rs.{}({}) end\n\n", func.name, params));
    }

    // Submodule stubs - generate all using a common pattern
    let submodules: &[(&str, &str, &[&LuaFunction])] = &[
        ("coro", "Coro submodule", &coro_fns),
        ("parallel", "Parallel submodule", &parallel_fns),
        ("async", "Async submodule (tokio-backed)", &async_fns),
        (
            "crypt",
            "Crypt submodule (AES-256-GCM encryption)",
            &crypt_fns,
        ),
        (
            "assets",
            "Assets submodule (hashing and manifest)",
            &assets_fns,
        ),
        (
            "pwa",
            "PWA submodule (manifest and service worker)",
            &pwa_fns,
        ),
        ("seo", "SEO submodule (sitemap and robots.txt)", &seo_fns),
    ];

    for (module_name, comment, funcs) in submodules {
        output.push_str(&format!("-- {}\n", comment));
        output.push_str(&format!("rs.{} = {{}}\n\n", module_name));

        for func in *funcs {
            output.push_str(&format!("---{}\n", func.description));
            for param in func.params {
                let opt = if param.optional { "?" } else { "" };
                output.push_str(&format!(
                    "---@param {}{} {} {}\n",
                    param.name, opt, param.typ, param.description
                ));
            }
            output.push_str(&format!("---@return {}\n", func.returns));

            let params = func
                .params
                .iter()
                .map(|p| p.name.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!(
                "function rs.{}.{}({}) end\n\n",
                module_name, func.name, params
            ));
        }
    }

    output.push_str("return rs\n");

    output
}

/// Generate Markdown API documentation
pub fn generate_markdown() -> String {
    let mut output = String::new();
    output.push_str("# rs-web Lua API Reference\n\n");
    output.push_str("Auto-generated documentation for rs-web Lua functions.\n\n");
    output.push_str("## Usage\n\n");
    output.push_str("```lua\n");
    output.push_str("local rs = require(\"rs-web\")\n");
    output.push_str("local content = rs.read_file(\"path/to/file.md\")\n");
    output.push_str("local html = rs.render_markdown(content)\n");
    output.push_str("```\n\n");
    output.push_str("## Table of Contents\n\n");
    output.push_str("- [Types](#types)\n");
    output.push_str("- [File Operations](#file-operations)\n");
    output.push_str("- [Search](#search)\n");
    output.push_str("- [Collections](#collections)\n");
    output.push_str("- [Text Processing](#text-processing)\n");
    output.push_str("- [Path Utilities](#path-utilities)\n");
    output.push_str("- [Environment](#environment)\n");
    output.push_str("- [Git](#git)\n");
    output.push_str("- [Content Processing](#content-processing)\n");
    output.push_str("- [Image Processing](#image-processing)\n");
    output.push_str("- [Coro Module](#coro-module)\n");
    output.push_str("- [Parallel Module](#parallel-module)\n");
    output.push_str("- [Async Module](#async-module)\n");
    output.push_str("- [Crypt Module](#crypt-module)\n");
    output.push_str("- [Assets Module](#assets-module)\n");
    output.push_str("- [PWA Module](#pwa-module)\n");
    output.push_str("- [SEO Module](#seo-module)\n\n");

    // Types section
    output.push_str("## Types\n\n");
    for class in LUA_CLASSES {
        output.push_str(&format!("### {}\n\n", class.name));
        output.push_str(&format!("{}\n\n", class.description));
        output.push_str("| Field | Type | Description |\n");
        output.push_str("|-------|------|-------------|\n");
        for field in class.fields {
            output.push_str(&format!(
                "| `{}` | `{}` | {} |\n",
                field.name, field.typ, field.description
            ));
        }
        output.push('\n');
    }

    // Group functions by category
    let categories = [
        (
            "File Operations",
            vec![
                "read_file",
                "write_file",
                "copy_file",
                "file_exists",
                "list_files",
                "list_dirs",
                "load_json",
                "load_yaml",
                "load_toml",
                "read_frontmatter",
            ],
        ),
        ("Search", vec!["glob", "scan"]),
        (
            "Collections",
            vec![
                "filter", "sort", "map", "find", "group_by", "unique", "reverse", "take", "skip",
                "keys", "values",
            ],
        ),
        (
            "Text Processing",
            vec![
                "slugify",
                "word_count",
                "reading_time",
                "truncate",
                "strip_tags",
                "format_date",
                "parse_date",
                "hash",
                "hash_file",
                "url_encode",
                "url_decode",
            ],
        ),
        (
            "Path Utilities",
            vec!["join_path", "basename", "dirname", "extension"],
        ),
        ("Environment", vec!["env", "print", "is_gitignored"]),
        ("Git", vec!["git_info"]),
        (
            "Content Processing",
            vec![
                "render_markdown",
                "rss_date",
                "extract_links_markdown",
                "extract_links_html",
                "extract_images_markdown",
                "extract_images_html",
                "html_to_text",
            ],
        ),
        (
            "Image Processing",
            vec![
                "image_dimensions",
                "image_resize",
                "image_convert",
                "image_optimize",
            ],
        ),
        (
            "Assets",
            vec![
                "build_css",
                "build_js",
                "check_unused_assets",
                "download_google_font",
            ],
        ),
    ];

    for (cat_name, func_names) in &categories {
        output.push_str(&format!("## {}\n\n", cat_name));

        for func_name in func_names {
            if let Some(func) = LUA_FUNCTIONS
                .iter()
                .find(|f| f.name == *func_name && f.module.is_none())
            {
                output.push_str(&format!("### `rs.{}()`\n\n", func.name));
                output.push_str(&format!("{}\n\n", func.description));

                if !func.params.is_empty() {
                    output.push_str("**Parameters:**\n\n");
                    for param in func.params {
                        let opt = if param.optional { " (optional)" } else { "" };
                        output.push_str(&format!(
                            "- `{}`: `{}`{} - {}\n",
                            param.name, param.typ, opt, param.description
                        ));
                    }
                    output.push('\n');
                }

                output.push_str(&format!("**Returns:** `{}`\n\n", func.returns));
            }
        }
    }

    // Module documentation - generate all using a common pattern
    let md_modules: &[(&str, &str, &str)] = &[
        (
            "coro",
            "Coro Module",
            "Coroutine-based cooperative multitasking helpers.",
        ),
        (
            "parallel",
            "Parallel Module",
            "True parallel processing with Rayon.",
        ),
        (
            "async",
            "Async Module",
            "Async I/O operations backed by Tokio.",
        ),
        (
            "crypt",
            "Crypt Module",
            "Encryption functions using AES-256-GCM with Argon2id key derivation.",
        ),
        (
            "assets",
            "Assets Module",
            "Asset hashing for cache busting. Use with Tera `| asset` filter.",
        ),
        (
            "css",
            "CSS Module",
            "CSS bundling, minification, and dead code elimination (purging).",
        ),
        (
            "pwa",
            "PWA Module",
            "Progressive Web App support: manifest and service worker generation.",
        ),
        (
            "seo",
            "SEO Module",
            "SEO support: sitemap and robots.txt generation.",
        ),
    ];

    for (module_key, title, description) in md_modules {
        output.push_str(&format!("## {}\n\n", title));
        output.push_str(&format!("{}\n\n", description));

        for func in LUA_FUNCTIONS
            .iter()
            .filter(|f| f.module == Some(*module_key))
        {
            output.push_str(&format!("### `rs.{}.{}()`\n\n", module_key, func.name));
            output.push_str(&format!("{}\n\n", func.description));

            if !func.params.is_empty() {
                output.push_str("**Parameters:**\n\n");
                for param in func.params {
                    let opt = if param.optional { " (optional)" } else { "" };
                    output.push_str(&format!(
                        "- `{}`: `{}`{} - {}\n",
                        param.name, param.typ, opt, param.description
                    ));
                }
                output.push('\n');
            }

            output.push_str(&format!("**Returns:** `{}`\n\n", func.returns));
        }
    }

    output
}
