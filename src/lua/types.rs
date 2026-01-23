//! Lua API type definitions for EmmyLua annotations

/// A Lua function definition
#[derive(Debug, Clone)]
pub struct LuaFunction {
    pub name: &'static str,
    pub module: Option<&'static str>,
    pub description: &'static str,
    pub params: &'static [LuaParam],
    pub returns: &'static str,
    /// For generic return types like AsyncIOTask<T>, specifies what T is
    pub generic_return: Option<&'static str>,
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
    /// Whether this class is generic (e.g., AsyncIOTask<T>)
    pub is_generic: bool,
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
        name: "AsyncTask",
        description: "Async task handle for fetch operations",
        is_generic: true,
        fields: &[LuaField {
            name: "is_completed",
            typ: "fun(): boolean",
            description: "Check if task is completed",
        }],
    },
    LuaClass {
        name: "AsyncIOTask",
        description: "Async task handle for I/O operations",
        is_generic: true,
        fields: &[LuaField {
            name: "is_completed",
            typ: "fun(): boolean",
            description: "Check if task is completed",
        }],
    },
    LuaClass {
        name: "AsyncLuaTask",
        description: "Async task handle for wrapped Lua functions",
        is_generic: true,
        fields: &[LuaField {
            name: "is_completed",
            typ: "fun(): boolean",
            description: "Check if task is completed",
        }],
    },
    LuaClass {
        name: "FetchResponse",
        description: "HTTP response from fetch operations",
        is_generic: false,
        fields: &[
            LuaField {
                name: "status",
                typ: "number",
                description: "HTTP status code",
            },
            LuaField {
                name: "ok",
                typ: "boolean",
                description: "True if status is 2xx",
            },
            LuaField {
                name: "body",
                typ: "string",
                description: "Response body",
            },
            LuaField {
                name: "headers",
                typ: "table<string, string>",
                description: "Response headers",
            },
            LuaField {
                name: "json",
                typ: "fun(): any",
                description: "Parse body as JSON",
            },
        ],
    },
    LuaClass {
        name: "FileMetadata",
        description: "File metadata from async.metadata",
        is_generic: false,
        fields: &[
            LuaField {
                name: "is_file",
                typ: "boolean",
                description: "True if path is a file",
            },
            LuaField {
                name: "is_dir",
                typ: "boolean",
                description: "True if path is a directory",
            },
            LuaField {
                name: "len",
                typ: "number",
                description: "File size in bytes",
            },
            LuaField {
                name: "readonly",
                typ: "boolean",
                description: "True if file is read-only",
            },
            LuaField {
                name: "modified",
                typ: "number?",
                description: "Unix timestamp of last modification",
            },
        ],
    },
    LuaClass {
        name: "DirEntry",
        description: "Directory entry from async.read_dir",
        is_generic: false,
        fields: &[
            LuaField {
                name: "path",
                typ: "string",
                description: "Full path to entry",
            },
            LuaField {
                name: "name",
                typ: "string",
                description: "Entry name",
            },
            LuaField {
                name: "is_file",
                typ: "boolean",
                description: "True if entry is a file",
            },
            LuaField {
                name: "is_dir",
                typ: "boolean",
                description: "True if entry is a directory",
            },
            LuaField {
                name: "is_symlink",
                typ: "boolean",
                description: "True if entry is a symlink",
            },
        ],
    },
    LuaClass {
        name: "FileInfo",
        description: "File metadata",
        is_generic: false,
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
        is_generic: false,
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
                name: "[string]",
                typ: "any",
                description: "Additional frontmatter fields",
            },
        ],
    },
    LuaClass {
        name: "GitInfo",
        description: "Git repository/file information",
        is_generic: false,
        fields: &[
            LuaField {
                name: "hash",
                typ: "string",
                description: "Full commit hash",
            },
            LuaField {
                name: "short_hash",
                typ: "string",
                description: "Short commit hash (7 chars)",
            },
            LuaField {
                name: "timestamp",
                typ: "number",
                description: "Commit timestamp (Unix seconds)",
            },
            LuaField {
                name: "author",
                typ: "string?",
                description: "Commit author (for file commits)",
            },
            LuaField {
                name: "branch",
                typ: "string?",
                description: "Current branch (for repo info)",
            },
            LuaField {
                name: "dirty",
                typ: "boolean?",
                description: "Whether repo has uncommitted changes",
            },
        ],
    },
    LuaClass {
        name: "ImageDimensions",
        description: "Image dimensions",
        is_generic: false,
        fields: &[
            LuaField {
                name: "width",
                typ: "number",
                description: "Width in pixels",
            },
            LuaField {
                name: "height",
                typ: "number",
                description: "Height in pixels",
            },
        ],
    },
    LuaClass {
        name: "DateTime",
        description: "Date and time components",
        is_generic: false,
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
            LuaField {
                name: "hour",
                typ: "number",
                description: "Hour (0-23)",
            },
            LuaField {
                name: "min",
                typ: "number",
                description: "Minute (0-59)",
            },
            LuaField {
                name: "sec",
                typ: "number",
                description: "Second (0-59)",
            },
            LuaField {
                name: "weekday",
                typ: "number",
                description: "Day of week (1=Monday, 7=Sunday)",
            },
            LuaField {
                name: "yday",
                typ: "number",
                description: "Day of year (1-366)",
            },
        ],
    },
];

// ============================================================================
// FUNCTION DEFINITIONS
// ============================================================================

pub static LUA_FUNCTIONS: &[LuaFunction] = &[
    // ========================================================================
    // rs.ops - Collection Operations
    // ========================================================================
    LuaFunction {
        name: "map",
        module: Some("ops"),
        description: "Transform each item using function",
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
                description: "Transform function",
                optional: false,
            },
        ],
        returns: "U[]",
        generic_return: None,
    },
    LuaFunction {
        name: "filter",
        module: Some("ops"),
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
        generic_return: None,
    },
    LuaFunction {
        name: "sort",
        module: Some("ops"),
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
                description: "Comparator (true if a < b)",
                optional: false,
            },
        ],
        returns: "T[]",
        generic_return: None,
    },
    LuaFunction {
        name: "find",
        module: Some("ops"),
        description: "Find first item matching predicate",
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
        generic_return: None,
    },
    LuaFunction {
        name: "group_by",
        module: Some("ops"),
        description: "Group items by key returned by function",
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
        generic_return: None,
    },
    LuaFunction {
        name: "unique",
        module: Some("ops"),
        description: "Remove duplicate items",
        params: &[LuaParam {
            name: "items",
            typ: "T[]",
            description: "Items to deduplicate",
            optional: false,
        }],
        returns: "T[]",
        generic_return: None,
    },
    LuaFunction {
        name: "reverse",
        module: Some("ops"),
        description: "Reverse array order",
        params: &[LuaParam {
            name: "items",
            typ: "T[]",
            description: "Items to reverse",
            optional: false,
        }],
        returns: "T[]",
        generic_return: None,
    },
    LuaFunction {
        name: "take",
        module: Some("ops"),
        description: "Take first n items",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items",
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
        generic_return: None,
    },
    LuaFunction {
        name: "skip",
        module: Some("ops"),
        description: "Skip first n items",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items",
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
        generic_return: None,
    },
    LuaFunction {
        name: "keys",
        module: Some("ops"),
        description: "Get all keys from a table",
        params: &[LuaParam {
            name: "table",
            typ: "table",
            description: "Table to get keys from",
            optional: false,
        }],
        returns: "any[]",
        generic_return: None,
    },
    LuaFunction {
        name: "values",
        module: Some("ops"),
        description: "Get all values from a table",
        params: &[LuaParam {
            name: "table",
            typ: "table",
            description: "Table to get values from",
            optional: false,
        }],
        returns: "any[]",
        generic_return: None,
    },
    LuaFunction {
        name: "reduce",
        module: Some("ops"),
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
        generic_return: None,
    },
    // rs.ops.par - Parallel Collection Operations
    LuaFunction {
        name: "map",
        module: Some("ops.par"),
        description: "Parallel map with optional context",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to transform",
                optional: false,
            },
            LuaParam {
                name: "fn",
                typ: "fun(item: T, ctx: table?): U",
                description: "Transform function",
                optional: false,
            },
            LuaParam {
                name: "ctx",
                typ: "table",
                description: "Context passed to each call",
                optional: true,
            },
        ],
        returns: "U[]",
        generic_return: None,
    },
    LuaFunction {
        name: "filter",
        module: Some("ops.par"),
        description: "Parallel filter with optional context",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to filter",
                optional: false,
            },
            LuaParam {
                name: "fn",
                typ: "fun(item: T, ctx: table?): boolean",
                description: "Predicate function",
                optional: false,
            },
            LuaParam {
                name: "ctx",
                typ: "table",
                description: "Context passed to each call",
                optional: true,
            },
        ],
        returns: "T[]",
        generic_return: None,
    },
    // ========================================================================
    // rs.fs - File System Operations
    // ========================================================================
    LuaFunction {
        name: "read",
        module: Some("fs"),
        description: "Read file contents as string",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to file",
            optional: false,
        }],
        returns: "string|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "write",
        module: Some("fs"),
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
        generic_return: None,
    },
    LuaFunction {
        name: "copy",
        module: Some("fs"),
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
        generic_return: None,
    },
    LuaFunction {
        name: "exists",
        module: Some("fs"),
        description: "Check if file exists",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to check",
            optional: false,
        }],
        returns: "boolean",
        generic_return: None,
    },
    LuaFunction {
        name: "list",
        module: Some("fs"),
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
        generic_return: None,
    },
    LuaFunction {
        name: "list_dirs",
        module: Some("fs"),
        description: "List subdirectories",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Directory to search",
            optional: false,
        }],
        returns: "string[]",
        generic_return: None,
    },
    LuaFunction {
        name: "glob",
        module: Some("fs"),
        description: "Find files matching glob pattern",
        params: &[LuaParam {
            name: "pattern",
            typ: "string",
            description: "Glob pattern (e.g., '**/*.md')",
            optional: false,
        }],
        returns: "FileInfo[]",
        generic_return: None,
    },
    LuaFunction {
        name: "scan",
        module: Some("fs"),
        description: "List directories in path",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Directory to scan",
            optional: false,
        }],
        returns: "FileInfo[]",
        generic_return: None,
    },
    // rs.fs.par - Parallel File Operations
    LuaFunction {
        name: "read",
        module: Some("fs.par"),
        description: "Read multiple files in parallel",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Paths to read",
            optional: false,
        }],
        returns: "(string|nil)[]",
        generic_return: None,
    },
    LuaFunction {
        name: "exists",
        module: Some("fs.par"),
        description: "Check multiple files exist in parallel",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Paths to check",
            optional: false,
        }],
        returns: "boolean[]",
        generic_return: None,
    },
    LuaFunction {
        name: "copy",
        module: Some("fs.par"),
        description: "Copy multiple files in parallel",
        params: &[
            LuaParam {
                name: "sources",
                typ: "string[]",
                description: "Source paths",
                optional: false,
            },
            LuaParam {
                name: "dests",
                typ: "string[]",
                description: "Destination paths",
                optional: false,
            },
        ],
        returns: "(boolean|string)[]",
        generic_return: None,
    },
    LuaFunction {
        name: "create_dirs",
        module: Some("fs.par"),
        description: "Create multiple directories in parallel",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Directory paths to create",
            optional: false,
        }],
        returns: "(boolean|string)[]",
        generic_return: None,
    },
    // ========================================================================
    // rs.data - Data Loading
    // ========================================================================
    LuaFunction {
        name: "load_json",
        module: Some("data"),
        description: "Load and parse JSON file",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to JSON file",
            optional: false,
        }],
        returns: "table|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "load_yaml",
        module: Some("data"),
        description: "Load and parse YAML file",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to YAML file",
            optional: false,
        }],
        returns: "table|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "load_toml",
        module: Some("data"),
        description: "Load and parse TOML file",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to TOML file",
            optional: false,
        }],
        returns: "table|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "load_frontmatter",
        module: Some("data"),
        description: "Read and parse frontmatter from markdown file",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to markdown file",
            optional: false,
        }],
        returns: "FrontmatterResult|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "from_json",
        module: Some("data"),
        description: "Parse JSON string to Lua value",
        params: &[LuaParam {
            name: "str",
            typ: "string",
            description: "JSON string",
            optional: false,
        }],
        returns: "any",
        generic_return: None,
    },
    LuaFunction {
        name: "to_json",
        module: Some("data"),
        description: "Serialize Lua value to JSON string",
        params: &[
            LuaParam {
                name: "value",
                typ: "any",
                description: "Value to serialize",
                optional: false,
            },
            LuaParam {
                name: "pretty",
                typ: "boolean",
                description: "Pretty print",
                optional: true,
            },
        ],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "from_yaml",
        module: Some("data"),
        description: "Parse YAML string to Lua value",
        params: &[LuaParam {
            name: "str",
            typ: "string",
            description: "YAML string",
            optional: false,
        }],
        returns: "any",
        generic_return: None,
    },
    LuaFunction {
        name: "to_yaml",
        module: Some("data"),
        description: "Serialize Lua value to YAML string",
        params: &[LuaParam {
            name: "value",
            typ: "any",
            description: "Value to serialize",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "from_toml",
        module: Some("data"),
        description: "Parse TOML string to Lua value",
        params: &[LuaParam {
            name: "str",
            typ: "string",
            description: "TOML string",
            optional: false,
        }],
        returns: "any",
        generic_return: None,
    },
    LuaFunction {
        name: "to_toml",
        module: Some("data"),
        description: "Serialize Lua value to TOML string",
        params: &[LuaParam {
            name: "value",
            typ: "any",
            description: "Value to serialize",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    // rs.data.par - Parallel Data Loading
    LuaFunction {
        name: "load_json",
        module: Some("data.par"),
        description: "Load multiple JSON files in parallel",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Paths to JSON files",
            optional: false,
        }],
        returns: "(table|nil)[]",
        generic_return: None,
    },
    LuaFunction {
        name: "load_yaml",
        module: Some("data.par"),
        description: "Load multiple YAML files in parallel",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Paths to YAML files",
            optional: false,
        }],
        returns: "(table|nil)[]",
        generic_return: None,
    },
    LuaFunction {
        name: "load_frontmatter",
        module: Some("data.par"),
        description: "Parse frontmatter from multiple files in parallel",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Paths to markdown files",
            optional: false,
        }],
        returns: "(FrontmatterResult|nil)[]",
        generic_return: None,
    },
    // ========================================================================
    // rs.image - Image Processing
    // ========================================================================
    LuaFunction {
        name: "dimensions",
        module: Some("image"),
        description: "Get image width and height",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to image",
            optional: false,
        }],
        returns: "ImageDimensions|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "resize",
        module: Some("image"),
        description: "Resize image",
        params: &[
            LuaParam {
                name: "input",
                typ: "string",
                description: "Input path",
                optional: false,
            },
            LuaParam {
                name: "output",
                typ: "string",
                description: "Output path",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ width: number, height?: number, quality?: number }",
                description: "Resize options",
                optional: false,
            },
        ],
        returns: "boolean",
        generic_return: None,
    },
    LuaFunction {
        name: "convert",
        module: Some("image"),
        description: "Convert image format",
        params: &[
            LuaParam {
                name: "input",
                typ: "string",
                description: "Input path",
                optional: false,
            },
            LuaParam {
                name: "output",
                typ: "string",
                description: "Output path",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ format?: string, quality?: number }",
                description: "Convert options",
                optional: true,
            },
        ],
        returns: "boolean",
        generic_return: None,
    },
    LuaFunction {
        name: "optimize",
        module: Some("image"),
        description: "Optimize/compress image",
        params: &[
            LuaParam {
                name: "input",
                typ: "string",
                description: "Input path",
                optional: false,
            },
            LuaParam {
                name: "output",
                typ: "string",
                description: "Output path",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ quality?: number }",
                description: "Optimize options",
                optional: true,
            },
        ],
        returns: "boolean",
        generic_return: None,
    },
    // rs.image.par - Parallel Image Processing
    LuaFunction {
        name: "resize",
        module: Some("image.par"),
        description: "Resize multiple images in parallel",
        params: &[
            LuaParam {
                name: "inputs",
                typ: "string[]",
                description: "Input paths",
                optional: false,
            },
            LuaParam {
                name: "outputs",
                typ: "string[]",
                description: "Output paths",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ width: number, height?: number, quality?: number }",
                description: "Resize options",
                optional: false,
            },
        ],
        returns: "(boolean|string)[]",
        generic_return: None,
    },
    LuaFunction {
        name: "convert",
        module: Some("image.par"),
        description: "Convert multiple images in parallel",
        params: &[
            LuaParam {
                name: "inputs",
                typ: "string[]",
                description: "Input paths",
                optional: false,
            },
            LuaParam {
                name: "outputs",
                typ: "string[]",
                description: "Output paths",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ quality?: number }",
                description: "Convert options",
                optional: true,
            },
        ],
        returns: "(boolean|string)[]",
        generic_return: None,
    },
    LuaFunction {
        name: "optimize",
        module: Some("image.par"),
        description: "Optimize multiple images in parallel",
        params: &[
            LuaParam {
                name: "inputs",
                typ: "string[]",
                description: "Input paths",
                optional: false,
            },
            LuaParam {
                name: "outputs",
                typ: "string[]",
                description: "Output paths",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ quality?: number }",
                description: "Optimize options",
                optional: true,
            },
        ],
        returns: "(boolean|string)[]",
        generic_return: None,
    },
    // ========================================================================
    // rs.text - Text Operations
    // ========================================================================
    LuaFunction {
        name: "slugify",
        module: Some("text"),
        description: "Convert text to URL-friendly slug",
        params: &[LuaParam {
            name: "text",
            typ: "string",
            description: "Text to slugify",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "word_count",
        module: Some("text"),
        description: "Count words in text",
        params: &[LuaParam {
            name: "text",
            typ: "string",
            description: "Text to count",
            optional: false,
        }],
        returns: "number",
        generic_return: None,
    },
    LuaFunction {
        name: "reading_time",
        module: Some("text"),
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
        generic_return: None,
    },
    LuaFunction {
        name: "truncate",
        module: Some("text"),
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
                description: "Suffix (default: '...')",
                optional: true,
            },
        ],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "url_encode",
        module: Some("text"),
        description: "URL encode a string",
        params: &[LuaParam {
            name: "text",
            typ: "string",
            description: "Text to encode",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "url_decode",
        module: Some("text"),
        description: "URL decode a string",
        params: &[LuaParam {
            name: "text",
            typ: "string",
            description: "Text to decode",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    // ========================================================================
    // rs.date - Date Operations
    // ========================================================================
    LuaFunction {
        name: "now",
        module: Some("date"),
        description: "Get current Unix timestamp",
        params: &[],
        returns: "number",
        generic_return: None,
    },
    LuaFunction {
        name: "from_timestamp",
        module: Some("date"),
        description: "Convert Unix timestamp to datetime table",
        params: &[LuaParam {
            name: "ts",
            typ: "number",
            description: "Unix timestamp (seconds)",
            optional: false,
        }],
        returns: "DateTime|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "to_timestamp",
        module: Some("date"),
        description: "Convert date/datetime to Unix timestamp",
        params: &[LuaParam {
            name: "date",
            typ: "number|string|DateTime",
            description: "Date to convert",
            optional: false,
        }],
        returns: "number|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "format",
        module: Some("date"),
        description: "Format a date/datetime using strftime format",
        params: &[
            LuaParam {
                name: "date",
                typ: "number|string|DateTime",
                description: "Date to format (timestamp, string, or table)",
                optional: false,
            },
            LuaParam {
                name: "format",
                typ: "string",
                description: "Format string (strftime)",
                optional: false,
            },
        ],
        returns: "string|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "parse",
        module: Some("date"),
        description: "Parse date/datetime string to table",
        params: &[
            LuaParam {
                name: "str",
                typ: "string",
                description: "Date string to parse",
                optional: false,
            },
            LuaParam {
                name: "format",
                typ: "string",
                description: "Custom strftime format (auto-detects if omitted)",
                optional: true,
            },
        ],
        returns: "DateTime|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "rss_format",
        module: Some("date"),
        description: "Format date for RSS feeds (RFC 2822)",
        params: &[LuaParam {
            name: "date",
            typ: "number|string|DateTime",
            description: "Date to format",
            optional: false,
        }],
        returns: "string|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "iso_format",
        module: Some("date"),
        description: "Format date as ISO 8601",
        params: &[LuaParam {
            name: "date",
            typ: "number|string|DateTime",
            description: "Date to format",
            optional: false,
        }],
        returns: "string|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "add",
        module: Some("date"),
        description: "Add time to a date",
        params: &[
            LuaParam {
                name: "date",
                typ: "number|string|DateTime",
                description: "Base date",
                optional: false,
            },
            LuaParam {
                name: "delta",
                typ: "{ years?: number, months?: number, days?: number, hours?: number, mins?: number, secs?: number }",
                description: "Time to add",
                optional: false,
            },
        ],
        returns: "DateTime|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "diff",
        module: Some("date"),
        description: "Get difference between two dates in seconds",
        params: &[
            LuaParam {
                name: "date1",
                typ: "number|string|DateTime",
                description: "First date",
                optional: false,
            },
            LuaParam {
                name: "date2",
                typ: "number|string|DateTime",
                description: "Second date",
                optional: false,
            },
        ],
        returns: "number|nil",
        generic_return: None,
    },
    // ========================================================================
    // rs.path - Path Operations
    // ========================================================================
    LuaFunction {
        name: "join",
        module: Some("path"),
        description: "Join path segments",
        params: &[LuaParam {
            name: "...",
            typ: "string",
            description: "Path segments",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "basename",
        module: Some("path"),
        description: "Get file name from path",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "dirname",
        module: Some("path"),
        description: "Get directory from path",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "extension",
        module: Some("path"),
        description: "Get file extension",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    // ========================================================================
    // rs.hash - Hash Operations
    // ========================================================================
    LuaFunction {
        name: "content",
        module: Some("hash"),
        description: "Hash string content (xxHash64)",
        params: &[LuaParam {
            name: "content",
            typ: "string",
            description: "Content to hash",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "file",
        module: Some("hash"),
        description: "Hash file contents",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to file",
            optional: false,
        }],
        returns: "string|nil",
        generic_return: None,
    },
    // ========================================================================
    // rs.git - Git Operations
    // ========================================================================
    LuaFunction {
        name: "info",
        module: Some("git"),
        description: "Get git info for repo or file",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to file/directory (optional, defaults to repo)",
            optional: true,
        }],
        returns: "GitInfo|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "is_ignored",
        module: Some("git"),
        description: "Check if path is ignored by .gitignore",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to check",
            optional: false,
        }],
        returns: "boolean",
        generic_return: None,
    },
    // ========================================================================
    // rs.html - HTML Operations
    // ========================================================================
    LuaFunction {
        name: "to_text",
        module: Some("html"),
        description: "Convert HTML to plain text",
        params: &[LuaParam {
            name: "html",
            typ: "string",
            description: "HTML content",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "strip_tags",
        module: Some("html"),
        description: "Remove HTML tags",
        params: &[LuaParam {
            name: "html",
            typ: "string",
            description: "HTML content",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "extract_links",
        module: Some("html"),
        description: "Extract links from HTML",
        params: &[LuaParam {
            name: "html",
            typ: "string",
            description: "HTML content",
            optional: false,
        }],
        returns: "string[]",
        generic_return: None,
    },
    LuaFunction {
        name: "extract_images",
        module: Some("html"),
        description: "Extract image paths from HTML",
        params: &[LuaParam {
            name: "html",
            typ: "string",
            description: "HTML content",
            optional: false,
        }],
        returns: "string[]",
        generic_return: None,
    },
    // ========================================================================
    // rs.log - Logging
    // ========================================================================
    LuaFunction {
        name: "trace",
        module: Some("log"),
        description: "Log at trace level",
        params: &[LuaParam {
            name: "...",
            typ: "string",
            description: "Messages to log",
            optional: false,
        }],
        returns: "nil",
        generic_return: None,
    },
    LuaFunction {
        name: "debug",
        module: Some("log"),
        description: "Log at debug level",
        params: &[LuaParam {
            name: "...",
            typ: "string",
            description: "Messages to log",
            optional: false,
        }],
        returns: "nil",
        generic_return: None,
    },
    LuaFunction {
        name: "info",
        module: Some("log"),
        description: "Log at info level",
        params: &[LuaParam {
            name: "...",
            typ: "string",
            description: "Messages to log",
            optional: false,
        }],
        returns: "nil",
        generic_return: None,
    },
    LuaFunction {
        name: "warn",
        module: Some("log"),
        description: "Log at warn level",
        params: &[LuaParam {
            name: "...",
            typ: "string",
            description: "Messages to log",
            optional: false,
        }],
        returns: "nil",
        generic_return: None,
    },
    LuaFunction {
        name: "error",
        module: Some("log"),
        description: "Log at error level",
        params: &[LuaParam {
            name: "...",
            typ: "string",
            description: "Messages to log",
            optional: false,
        }],
        returns: "nil",
        generic_return: None,
    },
    LuaFunction {
        name: "print",
        module: Some("log"),
        description: "Log message (always visible)",
        params: &[LuaParam {
            name: "...",
            typ: "string",
            description: "Messages to log",
            optional: false,
        }],
        returns: "nil",
        generic_return: None,
    },
    // ========================================================================
    // rs.env - Environment
    // ========================================================================
    LuaFunction {
        name: "get",
        module: Some("env"),
        description: "Get environment variable with optional default value",
        params: &[
            LuaParam {
                name: "name",
                typ: "string",
                description: "Variable name",
                optional: false,
            },
            LuaParam {
                name: "default",
                typ: "string",
                description: "Default value if variable is not set",
                optional: true,
            },
        ],
        returns: "string|nil",
        generic_return: None,
    },
    // ========================================================================
    // rs.markdown - Markdown Processing
    // ========================================================================
    // plugins sub-module (callable table)
    LuaFunction {
        name: "default",
        module: Some("markdown.plugins"),
        description: "Get default markdown plugins",
        params: &[LuaParam {
            name: "opts",
            typ: "{ lazy_images?: boolean, heading_anchors?: boolean, external_links?: boolean }",
            description: "Options to disable specific plugins",
            optional: true,
        }],
        returns: "function[]",
        generic_return: None,
    },
    LuaFunction {
        name: "lazy_images",
        module: Some("markdown.plugins"),
        description: "Plugin for lazy-loading images",
        params: &[LuaParam {
            name: "opts",
            typ: "table",
            description: "Plugin options",
            optional: true,
        }],
        returns: "function",
        generic_return: None,
    },
    LuaFunction {
        name: "heading_anchors",
        module: Some("markdown.plugins"),
        description: "Plugin for adding heading anchor IDs",
        params: &[LuaParam {
            name: "opts",
            typ: "table",
            description: "Plugin options",
            optional: true,
        }],
        returns: "function",
        generic_return: None,
    },
    LuaFunction {
        name: "external_links",
        module: Some("markdown.plugins"),
        description: "Plugin for external link attributes",
        params: &[LuaParam {
            name: "opts",
            typ: "table",
            description: "Plugin options",
            optional: true,
        }],
        returns: "function",
        generic_return: None,
    },
    LuaFunction {
        name: "render",
        module: Some("markdown"),
        description: "Render markdown to HTML",
        params: &[
            LuaParam {
                name: "content",
                typ: "string",
                description: "Markdown content",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ plugins?: function[] }",
                description: "Render options",
                optional: true,
            },
        ],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "extract_links",
        module: Some("markdown"),
        description: "Extract links from markdown",
        params: &[LuaParam {
            name: "content",
            typ: "string",
            description: "Markdown content",
            optional: false,
        }],
        returns: "string[]",
        generic_return: None,
    },
    LuaFunction {
        name: "extract_images",
        module: Some("markdown"),
        description: "Extract image paths from markdown",
        params: &[LuaParam {
            name: "content",
            typ: "string",
            description: "Markdown content",
            optional: false,
        }],
        returns: "string[]",
        generic_return: None,
    },
    // ========================================================================
    // rs.assets - Asset Hashing
    // ========================================================================
    LuaFunction {
        name: "hash",
        module: Some("assets"),
        description: "Compute hash of content, returns task for await",
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
                description: "Hash length (default: 8)",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
        generic_return: Some("string"),
    },
    LuaFunction {
        name: "hash_sync",
        module: Some("assets"),
        description: "Compute hash of content synchronously",
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
                description: "Hash length (default: 8)",
                optional: true,
            },
        ],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "write_hashed",
        module: Some("assets"),
        description: "Write content with hashed filename (async)",
        params: &[
            LuaParam {
                name: "content",
                typ: "string",
                description: "Content to write",
                optional: false,
            },
            LuaParam {
                name: "path",
                typ: "string",
                description: "Output path",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ hash_length?: number }",
                description: "Options",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
        generic_return: Some("string"),
    },
    LuaFunction {
        name: "get_path",
        module: Some("assets"),
        description: "Get hashed path for original",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Original path",
            optional: false,
        }],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "register",
        module: Some("assets"),
        description: "Register a path mapping",
        params: &[
            LuaParam {
                name: "original",
                typ: "string",
                description: "Original path",
                optional: false,
            },
            LuaParam {
                name: "hashed",
                typ: "string",
                description: "Hashed path",
                optional: false,
            },
        ],
        returns: "nil",
        generic_return: None,
    },
    LuaFunction {
        name: "manifest",
        module: Some("assets"),
        description: "Get all asset mappings",
        params: &[],
        returns: "table<string, string>",
        generic_return: None,
    },
    LuaFunction {
        name: "clear",
        module: Some("assets"),
        description: "Clear the asset manifest",
        params: &[],
        returns: "nil",
        generic_return: None,
    },
    LuaFunction {
        name: "check_unused",
        module: Some("assets"),
        description: "Find assets not referenced in HTML/CSS",
        params: &[LuaParam {
            name: "output_dir",
            typ: "string",
            description: "Output directory to scan",
            optional: false,
        }],
        returns: "string[]",
        generic_return: None,
    },
    // ========================================================================
    // rs.js - JavaScript Processing
    // ========================================================================
    LuaFunction {
        name: "concat",
        module: Some("js"),
        description: "Concatenate JavaScript files (async)",
        params: &[
            LuaParam {
                name: "input",
                typ: "string|string[]",
                description: "Input path(s) or glob pattern",
                optional: false,
            },
            LuaParam {
                name: "output",
                typ: "string",
                description: "Output path",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ minify?: boolean }",
                description: "Options",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
        generic_return: Some("nil"),
    },
    LuaFunction {
        name: "bundle",
        module: Some("js"),
        description: "Bundle and minify JavaScript (async)",
        params: &[
            LuaParam {
                name: "input",
                typ: "string|string[]",
                description: "Input path(s)",
                optional: false,
            },
            LuaParam {
                name: "output",
                typ: "string",
                description: "Output path",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ minify?: boolean, treeshake?: boolean, format?: string }",
                description: "Options",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
        generic_return: Some("nil"),
    },
    // ========================================================================
    // rs.css - CSS Processing
    // ========================================================================
    LuaFunction {
        name: "concat",
        module: Some("css"),
        description: "Concatenate CSS files (async)",
        params: &[
            LuaParam {
                name: "input",
                typ: "string|string[]",
                description: "Input path(s) or glob pattern",
                optional: false,
            },
            LuaParam {
                name: "output",
                typ: "string",
                description: "Output path",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ minify?: boolean, purge?: boolean, safelist?: string[] }",
                description: "Options",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
        generic_return: Some("nil"),
    },
    LuaFunction {
        name: "bundle",
        module: Some("css"),
        description: "Bundle and minify CSS with LightningCSS (async)",
        params: &[
            LuaParam {
                name: "input",
                typ: "string|string[]",
                description: "Input path(s)",
                optional: false,
            },
            LuaParam {
                name: "output",
                typ: "string",
                description: "Output path",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ minify?: boolean, purge?: boolean, safelist?: string[] }",
                description: "Options",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
        generic_return: Some("nil"),
    },
    LuaFunction {
        name: "purge",
        module: Some("css"),
        description: "Purge unused CSS rules from a CSS file (async)",
        params: &[
            LuaParam {
                name: "css_path",
                typ: "string",
                description: "Path to CSS file",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "{ safelist?: string[] }",
                description: "Purge options",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
        generic_return: Some("nil"),
    },
    LuaFunction {
        name: "critical",
        module: Some("css"),
        description: "Extract critical CSS for HTML content (async)",
        params: &[
            LuaParam {
                name: "html_content",
                typ: "string",
                description: "HTML content to analyze",
                optional: false,
            },
            LuaParam {
                name: "css_path",
                typ: "string",
                description: "Path to CSS file",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "{ minify?: boolean, safelist?: string[] }",
                description: "Options",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
        generic_return: Some("string"),
    },
    LuaFunction {
        name: "inline_critical",
        module: Some("css"),
        description: "Inline critical CSS into HTML file (async)",
        params: &[
            LuaParam {
                name: "html_path",
                typ: "string",
                description: "Path to HTML file",
                optional: false,
            },
            LuaParam {
                name: "css_path",
                typ: "string",
                description: "Path to CSS file",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "{ minify?: boolean, safelist?: string[], css_href?: string }",
                description: "Options",
                optional: true,
            },
        ],
        returns: "AsyncIOTask",
        generic_return: Some("nil"),
    },
    // ========================================================================
    // rs.fonts - Font Handling
    // ========================================================================
    LuaFunction {
        name: "download_google_font",
        module: Some("fonts"),
        description: "Download Google Font files (async)",
        params: &[
            LuaParam {
                name: "family",
                typ: "string",
                description: "Font family name",
                optional: false,
            },
            LuaParam {
                name: "options",
                typ: "{ fonts_dir: string, css_path: string, css_prefix?: string, weights?: number[], display?: string, cache?: boolean|string, minify?: boolean }",
                description: "Download options",
                optional: false,
            },
        ],
        returns: "AsyncIOTask",
        generic_return: Some("nil"),
    },
    // ========================================================================
    // rs.crypt - Encryption
    // ========================================================================
    LuaFunction {
        name: "encrypt",
        module: Some("crypt"),
        description: "Encrypt content with password",
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
                description: "Encryption password",
                optional: true,
            },
        ],
        returns: "string",
        generic_return: None,
    },
    LuaFunction {
        name: "decrypt",
        module: Some("crypt"),
        description: "Decrypt content with password",
        params: &[
            LuaParam {
                name: "encrypted",
                typ: "string",
                description: "Encrypted content",
                optional: false,
            },
            LuaParam {
                name: "password",
                typ: "string",
                description: "Decryption password",
                optional: true,
            },
        ],
        returns: "string|nil",
        generic_return: None,
    },
    LuaFunction {
        name: "encrypt_html",
        module: Some("crypt"),
        description: "Encrypt HTML with embedded decryption",
        params: &[
            LuaParam {
                name: "html",
                typ: "string",
                description: "HTML to encrypt",
                optional: false,
            },
            LuaParam {
                name: "password",
                typ: "string",
                description: "Encryption password",
                optional: true,
            },
        ],
        returns: "string",
        generic_return: None,
    },
    // ========================================================================
    // rs.pwa - PWA Generation
    // ========================================================================
    LuaFunction {
        name: "manifest",
        module: Some("pwa"),
        description: "Generate PWA manifest.json (async)",
        params: &[LuaParam {
            name: "options",
            typ: "{ name: string, short_name: string, output: string, description?: string, start_url?: string, display?: string, background_color?: string, theme_color?: string, icons?: { src: string, sizes: string, type?: string, purpose?: string }[] }",
            description: "Manifest configuration",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("nil"),
    },
    LuaFunction {
        name: "service_worker",
        module: Some("pwa"),
        description: "Generate service worker (async)",
        params: &[LuaParam {
            name: "options",
            typ: "{ cache_name: string, output: string, version?: string, precache?: string[], network_first?: string[], cache_first?: string[], offline_page?: string }",
            description: "Service worker configuration",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("nil"),
    },
    // ========================================================================
    // rs.seo - SEO Generation
    // ========================================================================
    LuaFunction {
        name: "sitemap",
        module: Some("seo"),
        description: "Generate sitemap.xml (async)",
        params: &[LuaParam {
            name: "options",
            typ: "{ base_url: string, pages: (string | { path: string, lastmod?: string, changefreq?: string, priority?: number })[], output: string, default_changefreq?: string, default_priority?: number, exclude?: string[] }",
            description: "Sitemap configuration",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("nil"),
    },
    LuaFunction {
        name: "robots",
        module: Some("seo"),
        description: "Generate robots.txt (async)",
        params: &[LuaParam {
            name: "options",
            typ: "{ output: string, sitemap_url?: string, allow?: string[], disallow?: string[], user_agent?: string }",
            description: "Robots configuration",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("nil"),
    },
    // ========================================================================
    // rs.coro - Coroutines
    // ========================================================================
    LuaFunction {
        name: "task",
        module: Some("coro"),
        description: "Create a task from a function",
        params: &[LuaParam {
            name: "fn",
            typ: "function",
            description: "Function to run",
            optional: false,
        }],
        returns: "Task",
        generic_return: None,
    },
    LuaFunction {
        name: "await",
        module: Some("coro"),
        description: "Await a task result",
        params: &[LuaParam {
            name: "task",
            typ: "Task",
            description: "Task to await",
            optional: false,
        }],
        returns: "any",
        generic_return: None,
    },
    // ========================================================================
    // rs.parallel - Parallel Processing (legacy)
    // ========================================================================
    LuaFunction {
        name: "map",
        module: Some("parallel"),
        description: "Parallel map with optional context",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to process",
                optional: false,
            },
            LuaParam {
                name: "fn",
                typ: "fun(item: T, ctx: table?): U",
                description: "Transform function",
                optional: false,
            },
            LuaParam {
                name: "ctx",
                typ: "table",
                description: "Context passed to each call",
                optional: true,
            },
        ],
        returns: "U[]",
        generic_return: None,
    },
    LuaFunction {
        name: "filter",
        module: Some("parallel"),
        description: "Parallel filter with optional context",
        params: &[
            LuaParam {
                name: "items",
                typ: "T[]",
                description: "Items to filter",
                optional: false,
            },
            LuaParam {
                name: "fn",
                typ: "fun(item: T, ctx: table?): boolean",
                description: "Predicate function",
                optional: false,
            },
            LuaParam {
                name: "ctx",
                typ: "table",
                description: "Context passed to each call",
                optional: true,
            },
        ],
        returns: "T[]",
        generic_return: None,
    },
    // ========================================================================
    // rs.async - Async I/O
    // ========================================================================
    LuaFunction {
        name: "fetch",
        module: Some("async"),
        description: "Fetch URL content (async)",
        params: &[
            LuaParam {
                name: "url",
                typ: "string",
                description: "URL to fetch",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ method?: string, headers?: table, body?: string, timeout?: number, cache?: boolean|string }",
                description: "Request options",
                optional: true,
            },
        ],
        returns: "AsyncTask",
        generic_return: Some("FetchResponse"),
    },
    LuaFunction {
        name: "fetch_sync",
        module: Some("async"),
        description: "Fetch URL content synchronously",
        params: &[
            LuaParam {
                name: "url",
                typ: "string",
                description: "URL to fetch",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ method?: string, headers?: table, body?: string, timeout?: number, cache?: boolean|string }",
                description: "Request options",
                optional: true,
            },
        ],
        returns: "FetchResponse",
        generic_return: None,
    },
    LuaFunction {
        name: "fetch_json",
        module: Some("async"),
        description: "Fetch and parse JSON",
        params: &[
            LuaParam {
                name: "url",
                typ: "string",
                description: "URL to fetch",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ method?: string, headers?: table, body?: string, timeout?: number }",
                description: "Request options",
                optional: true,
            },
        ],
        returns: "any",
        generic_return: None,
    },
    LuaFunction {
        name: "fetch_bytes",
        module: Some("async"),
        description: "Fetch binary data, returns task for await",
        params: &[
            LuaParam {
                name: "url",
                typ: "string",
                description: "URL to fetch",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ method?: string, headers?: table, body?: string, timeout?: number, cache?: boolean|string }",
                description: "Request options",
                optional: true,
            },
        ],
        returns: "AsyncTask",
        generic_return: Some("FetchResponse"),
    },
    LuaFunction {
        name: "fetch_bytes_sync",
        module: Some("async"),
        description: "Fetch binary data synchronously",
        params: &[
            LuaParam {
                name: "url",
                typ: "string",
                description: "URL to fetch",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ method?: string, headers?: table, body?: string, timeout?: number, cache?: boolean|string }",
                description: "Request options",
                optional: true,
            },
        ],
        returns: "{ status: number, ok: boolean, body: string }",
        generic_return: None,
    },
    LuaFunction {
        name: "fetch_all",
        module: Some("async"),
        description: "Fetch multiple URLs concurrently",
        params: &[LuaParam {
            name: "requests",
            typ: "(string | { url: string, options?: table })[]",
            description: "URLs or request objects",
            optional: false,
        }],
        returns: "FetchResponse[]",
        generic_return: None,
    },
    LuaFunction {
        name: "spawn",
        module: Some("async"),
        description: "Spawn a fetch task for later await",
        params: &[
            LuaParam {
                name: "url",
                typ: "string",
                description: "URL to fetch",
                optional: false,
            },
            LuaParam {
                name: "opts",
                typ: "{ method?: string, headers?: table, body?: string }",
                description: "Request options",
                optional: true,
            },
        ],
        returns: "AsyncTask",
        generic_return: Some("FetchResponse"),
    },
    LuaFunction {
        name: "wrap",
        module: Some("async"),
        description: "Wrap a Lua function for deferred execution",
        params: &[LuaParam {
            name: "fn",
            typ: "function",
            description: "Function to wrap",
            optional: false,
        }],
        returns: "AsyncLuaTask",
        generic_return: Some("T"),
    },
    LuaFunction {
        name: "await",
        module: Some("async"),
        description: "Await an async task",
        params: &[LuaParam {
            name: "task",
            typ: "AsyncTask|AsyncIOTask|AsyncLuaTask",
            description: "Task to await",
            optional: false,
        }],
        returns: "any",
        generic_return: None,
    },
    LuaFunction {
        name: "await_all",
        module: Some("async"),
        description: "Await multiple tasks concurrently",
        params: &[LuaParam {
            name: "tasks",
            typ: "(AsyncTask|AsyncIOTask|AsyncLuaTask)[]",
            description: "Tasks to await",
            optional: false,
        }],
        returns: "any[]",
        generic_return: None,
    },
    LuaFunction {
        name: "read_file",
        module: Some("async"),
        description: "Read text file, returns task for await",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to file",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("string"),
    },
    LuaFunction {
        name: "read_files",
        module: Some("async"),
        description: "Read multiple files concurrently, returns task for await",
        params: &[LuaParam {
            name: "paths",
            typ: "string[]",
            description: "Paths to read",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("(string|nil)[]"),
    },
    LuaFunction {
        name: "write_file",
        module: Some("async"),
        description: "Write text file, returns task for await",
        params: &[
            LuaParam {
                name: "path",
                typ: "string",
                description: "Path to write",
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
        generic_return: Some("boolean"),
    },
    LuaFunction {
        name: "write",
        module: Some("async"),
        description: "Write binary file, returns task for await",
        params: &[
            LuaParam {
                name: "path",
                typ: "string",
                description: "Path to write",
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
        generic_return: Some("boolean"),
    },
    LuaFunction {
        name: "exists",
        module: Some("async"),
        description: "Check if path exists, returns task for await",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to check",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("boolean"),
    },
    LuaFunction {
        name: "load_json",
        module: Some("async"),
        description: "Load and parse JSON file, returns task for await",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to JSON file",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("any"),
    },
    LuaFunction {
        name: "copy_file",
        module: Some("async"),
        description: "Copy file, returns task for await",
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
        generic_return: Some("number"),
    },
    LuaFunction {
        name: "rename",
        module: Some("async"),
        description: "Rename file or directory, returns task for await",
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
        generic_return: Some("boolean"),
    },
    LuaFunction {
        name: "create_dir",
        module: Some("async"),
        description: "Create directory, returns task for await",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Directory path",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("boolean"),
    },
    LuaFunction {
        name: "remove_file",
        module: Some("async"),
        description: "Remove file, returns task for await",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "File path",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("boolean"),
    },
    LuaFunction {
        name: "remove_dir",
        module: Some("async"),
        description: "Remove directory recursively, returns task for await",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Directory path",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("boolean"),
    },
    LuaFunction {
        name: "metadata",
        module: Some("async"),
        description: "Get file metadata, returns task for await",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to file/directory",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("FileMetadata"),
    },
    LuaFunction {
        name: "read_dir",
        module: Some("async"),
        description: "List directory contents, returns task for await",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Directory path",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("DirEntry[]"),
    },
    LuaFunction {
        name: "canonicalize",
        module: Some("async"),
        description: "Get canonical/absolute path, returns task for await",
        params: &[LuaParam {
            name: "path",
            typ: "string",
            description: "Path to resolve",
            optional: false,
        }],
        returns: "AsyncIOTask",
        generic_return: Some("string"),
    },
];

// ============================================================================
// DOCUMENTATION GENERATORS
// ============================================================================

/// Generate EmmyLua annotations
pub fn generate_emmylua() -> String {
    let mut output = String::new();
    output.push_str("---@meta rs-web\n\n");
    output.push_str("-- Auto-generated EmmyLua annotations for rs-web\n\n");

    // Generate class definitions
    for class in LUA_CLASSES {
        if class.is_generic {
            // Generic class with type parameter
            output.push_str(&format!("---@class {}<T>\n", class.name));
        } else {
            output.push_str(&format!("---@class {}\n", class.name));
        }
        for field in class.fields {
            output.push_str(&format!(
                "---@field {} {} {}\n",
                field.name, field.typ, field.description
            ));
        }
        output.push('\n');
    }

    // Generate module structure
    output.push_str("---@class rs\n");
    output.push_str("local rs = {}\n\n");

    // Group functions by module
    let mut modules: std::collections::HashMap<&str, Vec<&LuaFunction>> =
        std::collections::HashMap::new();
    for func in LUA_FUNCTIONS {
        let mod_name = func.module.unwrap_or("");
        modules.entry(mod_name).or_default().push(func);
    }

    // Track declared modules to avoid duplicates
    let mut declared_modules: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Generate each module
    for (mod_name, funcs) in &modules {
        if mod_name.is_empty() {
            continue;
        }

        let parts: Vec<&str> = mod_name.split('.').collect();

        // Declare all parent modules first
        let mut current_path = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                current_path.push('.');
            }
            current_path.push_str(part);

            if !declared_modules.contains(&current_path) {
                declared_modules.insert(current_path.clone());
                output.push_str(&format!("---@class rs.{}\n", current_path));
                output.push_str(&format!("rs.{} = {{}}\n\n", current_path));
            }
        }

        for func in funcs {
            // Generate function documentation
            output.push_str(&format!("--- {}\n", func.description));
            for param in func.params {
                let optional = if param.optional { "?" } else { "" };
                output.push_str(&format!(
                    "---@param {}{} {} {}\n",
                    param.name, optional, param.typ, param.description
                ));
            }

            // Handle generic return types
            let return_type = if let Some(inner_type) = func.generic_return {
                format!("{}<{}>", func.returns, inner_type)
            } else {
                func.returns.to_string()
            };
            output.push_str(&format!("---@return {}\n", return_type));

            output.push_str(&format!("function rs.{}.{}(", mod_name, func.name));
            let param_names: Vec<&str> = func.params.iter().map(|p| p.name).collect();
            output.push_str(&param_names.join(", "));
            output.push_str(") end\n\n");
        }
    }

    output.push_str("return rs\n");
    output
}

/// Generate markdown documentation
pub fn generate_markdown() -> String {
    let mut output = String::new();
    output.push_str("# Lua API Reference\n\n");

    // Group functions by module
    let mut modules: std::collections::BTreeMap<&str, Vec<&LuaFunction>> =
        std::collections::BTreeMap::new();
    for func in LUA_FUNCTIONS {
        let mod_name = func.module.unwrap_or("rs");
        modules.entry(mod_name).or_default().push(func);
    }

    for (mod_name, funcs) in modules {
        output.push_str(&format!("## rs.{}\n\n", mod_name));

        for func in funcs {
            output.push_str(&format!("### `rs.{}.{}`\n\n", mod_name, func.name));
            output.push_str(&format!("{}\n\n", func.description));

            if !func.params.is_empty() {
                output.push_str("**Parameters:**\n\n");
                for param in func.params {
                    let optional = if param.optional { " (optional)" } else { "" };
                    output.push_str(&format!(
                        "- `{}`: `{}`{} - {}\n",
                        param.name, param.typ, optional, param.description
                    ));
                }
                output.push('\n');
            }

            output.push_str(&format!("**Returns:** `{}`\n\n", func.returns));
        }
    }

    output
}
