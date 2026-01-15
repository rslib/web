//! Configuration loader for rs-web
//!
//! This module provides Lua-based configuration.
//! Config files are written in Lua and can include computed values and custom filters.

use anyhow::{Context, Result};
use mlua::{Function, Lua, LuaSerdeExt, Table, Value};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Configuration data structure (deserializable from Lua)
#[derive(Debug, Clone)]
pub struct ConfigData {
    pub site: SiteConfig,
    pub seo: SeoConfig,
    pub build: BuildConfig,
    pub images: ImagesConfig,
    pub highlight: HighlightConfig,
    pub paths: PathsConfig,
    pub templates: TemplatesConfig,
    pub permalinks: PermalinksConfig,
    pub encryption: EncryptionConfig,
    pub graph: GraphConfig,
    pub rss: RssConfig,
    pub text: TextConfig,
    pub sections: SectionsConfig,
}

/// Main configuration structure with embedded Lua state
pub struct Config {
    // Configuration data
    pub data: ConfigData,

    // Lua runtime state (for computed values, filters, sort functions)
    lua: Lua,
    computed: HashMap<String, mlua::RegistryKey>,
    filters: HashMap<String, mlua::RegistryKey>,
    functions: HashMap<String, mlua::RegistryKey>,
    computed_pages: Option<mlua::RegistryKey>,
    sort_fns: HashMap<String, mlua::RegistryKey>,
    filter_fns: HashMap<String, mlua::RegistryKey>,
    before_build: Option<mlua::RegistryKey>,
    after_build: Option<mlua::RegistryKey>,
}

// Provide convenient access to data fields
impl std::ops::Deref for Config {
    type Target = ConfigData;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl std::ops::DerefMut for Config {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SiteConfig {
    pub title: String,
    pub description: String,
    pub base_url: String,
    pub author: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct SeoConfig {
    pub twitter_handle: Option<String>,
    pub default_og_image: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BuildConfig {
    pub output_dir: String,
    #[serde(default = "default_true")]
    pub minify_css: bool,
    #[serde(default = "default_css_output")]
    pub css_output: String,
}

fn default_css_output() -> String {
    "rs.css".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct ImagesConfig {
    #[serde(default = "default_quality")]
    pub quality: f32,
    #[serde(default = "default_scale_factor")]
    pub scale_factor: f64,
}

fn default_quality() -> f32 {
    85.0
}

fn default_scale_factor() -> f64 {
    1.0
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct HighlightConfig {
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default = "default_highlight_class")]
    pub class: String,
}

fn default_highlight_class() -> String {
    "me".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct PathsConfig {
    #[serde(default = "default_content_dir")]
    pub content: String,
    #[serde(default = "default_styles_dir")]
    pub styles: String,
    #[serde(default = "default_static_dir")]
    pub static_files: String,
    #[serde(default = "default_templates_dir")]
    pub templates: String,
    #[serde(default = "default_home_page")]
    pub home: String,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default = "default_true")]
    pub exclude_defaults: bool,
    #[serde(default = "default_true")]
    pub respect_gitignore: bool,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            content: default_content_dir(),
            styles: default_styles_dir(),
            static_files: default_static_dir(),
            templates: default_templates_dir(),
            home: default_home_page(),
            exclude: Vec::new(),
            exclude_defaults: true,
            respect_gitignore: true,
        }
    }
}

fn default_content_dir() -> String {
    "content".to_string()
}
fn default_styles_dir() -> String {
    "styles".to_string()
}
fn default_static_dir() -> String {
    "static".to_string()
}
fn default_templates_dir() -> String {
    "templates".to_string()
}
fn default_home_page() -> String {
    "index.md".to_string()
}

/// Template mapping: section name -> template file
#[derive(Debug, Deserialize, Clone, Default)]
pub struct TemplatesConfig {
    #[serde(flatten)]
    pub sections: HashMap<String, String>,
}

/// Permalink patterns: section name -> pattern
#[derive(Debug, Deserialize, Clone, Default)]
pub struct PermalinksConfig {
    #[serde(flatten)]
    pub sections: HashMap<String, String>,
}

/// Encryption config for password-protected posts
#[derive(Debug, Deserialize, Clone, Default)]
pub struct EncryptionConfig {
    pub password_command: Option<String>,
    pub password: Option<String>,
}

/// Graph visualization config
#[derive(Debug, Deserialize, Clone)]
pub struct GraphConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_graph_template")]
    pub template: String,
    #[serde(default = "default_graph_path")]
    pub path: String,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            template: default_graph_template(),
            path: default_graph_path(),
        }
    }
}

fn default_graph_template() -> String {
    "graph.html".to_string()
}

fn default_graph_path() -> String {
    "graph".to_string()
}

/// RSS feed config
#[derive(Debug, Deserialize, Clone)]
pub struct RssConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_rss_filename")]
    pub filename: String,
    #[serde(default)]
    pub sections: Vec<String>,
    #[serde(default = "default_rss_limit")]
    pub limit: usize,
    #[serde(default)]
    pub exclude_encrypted_blocks: bool,
}

impl Default for RssConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            filename: default_rss_filename(),
            sections: Vec::new(),
            limit: default_rss_limit(),
            exclude_encrypted_blocks: false,
        }
    }
}

fn default_rss_filename() -> String {
    "rss.xml".to_string()
}

fn default_rss_limit() -> usize {
    20
}

/// Plain text output config
#[derive(Debug, Deserialize, Clone)]
pub struct TextConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub sections: Vec<String>,
    #[serde(default)]
    pub exclude_encrypted: bool,
    #[serde(default = "default_true")]
    pub include_home: bool,
}

impl Default for TextConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sections: Vec::new(),
            exclude_encrypted: false,
            include_home: true,
        }
    }
}

/// Section-specific configuration
#[derive(Debug, Deserialize, Clone, Default)]
pub struct SectionsConfig {
    #[serde(flatten)]
    pub sections: HashMap<String, SectionConfig>,
}

/// Configuration for a single section
#[derive(Debug, Deserialize, Clone)]
pub struct SectionConfig {
    /// How to iterate content: "files" (default) or "directories"
    #[serde(default = "default_iterate")]
    pub iterate: String,
}

impl Default for SectionConfig {
    fn default() -> Self {
        Self {
            iterate: default_iterate(),
        }
    }
}

fn default_iterate() -> String {
    "files".to_string()
}

fn default_true() -> bool {
    true
}

/// A computed page to be generated
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ComputedPage {
    /// URL path (e.g., "/tags/array/")
    pub path: String,
    /// Template to use (e.g., "tag.html")
    pub template: String,
    /// Page title
    pub title: String,
    /// Custom data available in template as `page.data`
    pub data: serde_json::Value,
}

impl Config {
    /// Create a Config with default Lua state (for testing)
    #[cfg(test)]
    pub fn from_data(data: ConfigData) -> Self {
        let lua = Lua::new();
        Self {
            data,
            lua,
            computed: HashMap::new(),
            filters: HashMap::new(),
            functions: HashMap::new(),
            computed_pages: None,
            sort_fns: HashMap::new(),
            filter_fns: HashMap::new(),
            before_build: None,
            after_build: None,
        }
    }

    /// Load config from a Lua file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        // Determine actual config file path
        let config_path = if path.is_dir() {
            let lua_path = path.join("config.lua");
            if lua_path.exists() {
                lua_path
            } else {
                anyhow::bail!("No config.lua found in {:?}", path);
            }
        } else {
            path.to_path_buf()
        };

        let lua = Lua::new();

        // Get project root (directory containing config file)
        let project_root = config_path
            .parent()
            .map(|p| {
                if p.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    p.to_path_buf()
                }
            })
            .unwrap_or_else(|| PathBuf::from("."));
        let project_root = project_root
            .canonicalize()
            .unwrap_or_else(|_| project_root.clone());

        // First pass: register functions without sandbox to load config
        register_lua_functions(&lua, &project_root, false)
            .map_err(|e| anyhow::anyhow!("Failed to register Lua functions: {}", e))?;

        // Load and execute the config file
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

        let config_table: Table = lua
            .load(&content)
            .set_name(config_path.to_string_lossy())
            .eval()
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute config file {:?}: {}", config_path, e)
            })?;

        // Check sandbox setting (default: true)
        let sandbox = config_table
            .get::<Table>("lua")
            .ok()
            .and_then(|t| t.get::<bool>("sandbox").ok())
            .unwrap_or(true);

        // Re-register functions with proper sandbox setting if sandbox is enabled
        if sandbox {
            register_lua_functions(&lua, &project_root, true)
                .map_err(|e| anyhow::anyhow!("Failed to register Lua functions: {}", e))?;
        }

        // Parse the config table
        let mut sort_fns = HashMap::new();
        let mut filter_fns = HashMap::new();
        let data = parse_config(&lua, &config_table, &mut sort_fns, &mut filter_fns)
            .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))?;

        // Extract computed functions
        let computed = extract_functions(&lua, &config_table, "computed")
            .map_err(|e| anyhow::anyhow!("Failed to extract computed functions: {}", e))?;

        // Extract filter functions
        let filters = extract_functions(&lua, &config_table, "filters")
            .map_err(|e| anyhow::anyhow!("Failed to extract filter functions: {}", e))?;

        // Extract custom template functions
        let functions = extract_functions(&lua, &config_table, "functions")
            .map_err(|e| anyhow::anyhow!("Failed to extract custom functions: {}", e))?;

        // Extract computed_pages function
        let computed_pages = if let Ok(func) = config_table.get::<Function>("computed_pages") {
            Some(
                lua.create_registry_value(func)
                    .map_err(|e| anyhow::anyhow!("Failed to store computed_pages: {}", e))?,
            )
        } else {
            None
        };

        // Extract hooks
        let hooks: Option<Table> = config_table.get("hooks").ok();
        let before_build = if let Some(ref h) = hooks {
            h.get::<Function>("before_build")
                .ok()
                .map(|f| lua.create_registry_value(f))
                .transpose()
                .map_err(|e| anyhow::anyhow!("Failed to store before_build hook: {}", e))?
        } else {
            None
        };
        let after_build = if let Some(ref h) = hooks {
            h.get::<Function>("after_build")
                .ok()
                .map(|f| lua.create_registry_value(f))
                .transpose()
                .map_err(|e| anyhow::anyhow!("Failed to store after_build hook: {}", e))?
        } else {
            None
        };

        Ok(Config {
            data,
            lua,
            computed,
            filters,
            functions,
            computed_pages,
            sort_fns,
            filter_fns,
            before_build,
            after_build,
        })
    }

    /// Call a computed function with sections data
    pub fn call_computed(&self, name: &str, sections_json: &str) -> Result<serde_json::Value> {
        let key = self
            .computed
            .get(name)
            .with_context(|| format!("Computed function '{}' not found", name))?;

        let func: Function = self
            .lua
            .registry_value(key)
            .map_err(|e| anyhow::anyhow!("Failed to get computed function: {}", e))?;

        let json_value: serde_json::Value = serde_json::from_str(sections_json)
            .map_err(|e| anyhow::anyhow!("Invalid JSON: {}", e))?;
        let sections: Value = self
            .lua
            .to_value(&json_value)
            .map_err(|e| anyhow::anyhow!("Failed to convert to Lua: {}", e))?;

        let result: Value = func
            .call(sections)
            .map_err(|e| anyhow::anyhow!("Failed to call computed '{}': {}", name, e))?;
        let json_value: serde_json::Value = self
            .lua
            .from_value(result)
            .map_err(|e| anyhow::anyhow!("Failed to convert result: {}", e))?;

        Ok(json_value)
    }

    /// Check if a section has a custom sort function
    pub fn has_sort_fn(&self, section_name: &str) -> bool {
        self.sort_fns.contains_key(section_name)
    }

    /// Call the sort function for a section (C-style comparator: returns -1, 0, 1)
    pub fn call_sort_fn(
        &self,
        section_name: &str,
        a_json: &serde_json::Value,
        b_json: &serde_json::Value,
    ) -> Result<std::cmp::Ordering> {
        let key = self
            .sort_fns
            .get(section_name)
            .with_context(|| format!("Sort function for '{}' not found", section_name))?;

        let func: Function = self
            .lua
            .registry_value(key)
            .map_err(|e| anyhow::anyhow!("Failed to get sort function: {}", e))?;

        let a: Value = self
            .lua
            .to_value(a_json)
            .map_err(|e| anyhow::anyhow!("Failed to convert a to Lua: {}", e))?;
        let b: Value = self
            .lua
            .to_value(b_json)
            .map_err(|e| anyhow::anyhow!("Failed to convert b to Lua: {}", e))?;

        let result: i32 = func
            .call((a, b))
            .map_err(|e| anyhow::anyhow!("Sort function failed: {}", e))?;

        Ok(match result {
            n if n < 0 => std::cmp::Ordering::Less,
            n if n > 0 => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        })
    }

    /// Check if a section has a custom filter function
    pub fn has_filter_fn(&self, section_name: &str) -> bool {
        self.filter_fns.contains_key(section_name)
    }

    /// Call the filter function for a section (returns true to keep, false to exclude)
    pub fn call_filter_fn(
        &self,
        section_name: &str,
        post_json: &serde_json::Value,
    ) -> Result<bool> {
        let key = self
            .filter_fns
            .get(section_name)
            .with_context(|| format!("Filter function for '{}' not found", section_name))?;

        let func: Function = self
            .lua
            .registry_value(key)
            .map_err(|e| anyhow::anyhow!("Failed to get filter function: {}", e))?;

        let post: Value = self
            .lua
            .to_value(post_json)
            .map_err(|e| anyhow::anyhow!("Failed to convert post to Lua: {}", e))?;

        let result: bool = func
            .call(post)
            .map_err(|e| anyhow::anyhow!("Filter function failed: {}", e))?;

        Ok(result)
    }

    /// Get all computed function names
    pub fn computed_names(&self) -> Vec<&str> {
        self.computed.keys().map(|s| s.as_str()).collect()
    }

    /// Get all filter function names
    pub fn filter_names(&self) -> Vec<&str> {
        self.filters.keys().map(|s| s.as_str()).collect()
    }

    /// Check if computed_pages function exists
    pub fn has_computed_pages(&self) -> bool {
        self.computed_pages.is_some()
    }

    /// Call before_build hook
    pub fn call_before_build(&self) -> Result<()> {
        if let Some(ref key) = self.before_build {
            let func: Function = self
                .lua
                .registry_value(key)
                .map_err(|e| anyhow::anyhow!("Failed to get before_build: {}", e))?;
            func.call::<()>(())
                .map_err(|e| anyhow::anyhow!("before_build hook failed: {}", e))?;
        }
        Ok(())
    }

    /// Call after_build hook
    pub fn call_after_build(&self) -> Result<()> {
        if let Some(ref key) = self.after_build {
            let func: Function = self
                .lua
                .registry_value(key)
                .map_err(|e| anyhow::anyhow!("Failed to get after_build: {}", e))?;
            func.call::<()>(())
                .map_err(|e| anyhow::anyhow!("after_build hook failed: {}", e))?;
        }
        Ok(())
    }

    /// Call a filter function with a value
    pub fn call_filter(&self, name: &str, value: &str) -> Result<String> {
        let key = self
            .filters
            .get(name)
            .with_context(|| format!("Filter '{}' not found", name))?;

        let func: Function = self
            .lua
            .registry_value(key)
            .map_err(|e| anyhow::anyhow!("Failed to get filter: {}", e))?;

        let result: String = func
            .call(value.to_string())
            .map_err(|e| anyhow::anyhow!("Filter '{}' failed: {}", name, e))?;

        Ok(result)
    }

    /// Call a custom template function
    pub fn call_function(
        &self,
        name: &str,
        args: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let key = self
            .functions
            .get(name)
            .with_context(|| format!("Function '{}' not found", name))?;

        let func: Function = self
            .lua
            .registry_value(key)
            .map_err(|e| anyhow::anyhow!("Failed to get function: {}", e))?;

        let lua_args: Vec<Value> = args
            .into_iter()
            .map(|v| self.lua.to_value(&v))
            .collect::<mlua::Result<Vec<_>>>()
            .map_err(|e| anyhow::anyhow!("Failed to convert args: {}", e))?;

        let result: Value = func
            .call(mlua::MultiValue::from_iter(lua_args))
            .map_err(|e| anyhow::anyhow!("Function '{}' failed: {}", name, e))?;

        let json_result: serde_json::Value = self
            .lua
            .from_value(result)
            .map_err(|e| anyhow::anyhow!("Failed to convert result: {}", e))?;

        Ok(json_result)
    }

    /// Get all custom function names
    pub fn function_names(&self) -> Vec<&str> {
        self.functions.keys().map(|s| s.as_str()).collect()
    }

    /// Call computed_pages function to generate dynamic pages
    pub fn call_computed_pages(&self, sections_json: &str) -> Result<Vec<ComputedPage>> {
        let key = match &self.computed_pages {
            Some(k) => k,
            None => return Ok(Vec::new()),
        };

        let func: Function = self
            .lua
            .registry_value(key)
            .map_err(|e| anyhow::anyhow!("Failed to get computed_pages: {}", e))?;

        let json_value: serde_json::Value = serde_json::from_str(sections_json)
            .map_err(|e| anyhow::anyhow!("Invalid JSON: {}", e))?;
        let sections: Value = self
            .lua
            .to_value(&json_value)
            .map_err(|e| anyhow::anyhow!("Failed to convert to Lua: {}", e))?;

        let result: Value = func
            .call(sections)
            .map_err(|e| anyhow::anyhow!("Failed to call computed_pages: {}", e))?;
        let pages: Vec<ComputedPage> = self
            .lua
            .from_value(result)
            .map_err(|e| anyhow::anyhow!("Failed to convert result: {}", e))?;

        Ok(pages)
    }
}

/// Check if a path is within the project root (for sandbox mode)
fn is_path_within_root(path: &Path, root: &Path) -> bool {
    // Try to canonicalize the path, handling both existing and non-existing paths
    let resolved = if path.exists() {
        path.canonicalize().ok()
    } else {
        // For non-existing paths, canonicalize the parent and append the filename
        path.parent()
            .map(|p| {
                if p.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    p.to_path_buf()
                }
            })
            .and_then(|p| p.canonicalize().ok())
            .map(|p| p.join(path.file_name().unwrap_or_default()))
    };

    match resolved {
        Some(abs_path) => abs_path.starts_with(root),
        None => false,
    }
}

/// Resolve a path relative to project root
fn resolve_path(path: &str, root: &Path) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

/// Register helper functions available in config.lua
fn register_lua_functions(lua: &Lua, project_root: &Path, sandbox: bool) -> mlua::Result<()> {
    let globals = lua.globals();

    // Store sandbox settings in Lua for reference
    globals.set("__sandbox_enabled", sandbox)?;
    globals.set("__project_root", project_root.to_string_lossy().to_string())?;

    let root = project_root.to_path_buf();

    // load_json(path) - Load and parse a JSON file
    let root_clone = root.clone();
    let load_json = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        let content = match std::fs::read_to_string(&resolved) {
            Ok(c) => c,
            Err(_) => return Ok(Value::Nil),
        };

        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(v) => lua.to_value(&v),
            Err(_) => Ok(Value::Nil),
        }
    })?;
    globals.set("load_json", load_json)?;

    // read_file(path) - Read a file as text
    let root_clone = root.clone();
    let read_file = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        match std::fs::read_to_string(&resolved) {
            Ok(content) => Ok(Value::String(lua.create_string(&content)?)),
            Err(_) => Ok(Value::Nil),
        }
    })?;
    globals.set("read_file", read_file)?;

    // file_exists(path) - Check if a file exists
    let root_clone = root.clone();
    let file_exists = lua.create_function(move |_, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }
        Ok(resolved.exists())
    })?;
    globals.set("file_exists", file_exists)?;

    // list_files(path, pattern?) - List files in directory
    let root_clone = root.clone();
    let list_files = lua.create_function(move |lua, (path, pattern): (String, Option<String>)| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        let pattern = pattern.unwrap_or_else(|| "*".to_string());
        let glob_pattern = format!("{}/{}", resolved.display(), pattern);

        let mut files = Vec::new();
        if let Ok(entries) = glob::glob(&glob_pattern) {
            for entry in entries.flatten() {
                // Skip files outside sandbox (in case glob pattern escapes)
                if sandbox && !is_path_within_root(&entry, &root_clone) {
                    continue;
                }
                if entry.is_file() {
                    let table = lua.create_table()?;
                    table.set("path", entry.to_string_lossy().to_string())?;
                    table.set(
                        "name",
                        entry
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    )?;
                    table.set(
                        "stem",
                        entry
                            .file_stem()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    )?;
                    table.set(
                        "ext",
                        entry
                            .extension()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    )?;
                    files.push(table);
                }
            }
        }

        let result = lua.create_table()?;
        for (i, file) in files.into_iter().enumerate() {
            result.set(i + 1, file)?;
        }
        Ok(result)
    })?;
    globals.set("list_files", list_files)?;

    // list_dirs(path) - List subdirectories
    let root_clone = root.clone();
    let list_dirs = lua.create_function(move |lua, path: String| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot access '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        let mut dirs = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&resolved) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                // Skip directories outside sandbox
                if sandbox && !is_path_within_root(&entry_path, &root_clone) {
                    continue;
                }
                if entry_path.is_dir()
                    && let Some(name) = entry_path.file_name().and_then(|n| n.to_str())
                    && !name.starts_with('.')
                {
                    dirs.push(name.to_string());
                }
            }
        }
        dirs.sort();

        let result = lua.create_table()?;
        for (i, dir) in dirs.into_iter().enumerate() {
            result.set(i + 1, dir)?;
        }
        Ok(result)
    })?;
    globals.set("list_dirs", list_dirs)?;

    // write_file(path, content) - Write content to a file
    let root_clone = root.clone();
    let write_file = lua.create_function(move |_, (path, content): (String, String)| {
        let resolved = resolve_path(&path, &root_clone);
        if sandbox && !is_path_within_root(&resolved, &root_clone) {
            return Err(mlua::Error::RuntimeError(format!(
                "Sandbox: cannot write '{}' outside project directory. Set lua.sandbox = false to disable.",
                path
            )));
        }

        // Create parent directories if needed
        if let Some(parent) = resolved.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&resolved, &content) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    })?;
    globals.set("write_file", write_file)?;

    // env(name) - Get environment variable
    let env_fn = lua.create_function(|lua, name: String| match std::env::var(&name) {
        Ok(val) => Ok(Value::String(lua.create_string(&val)?)),
        Err(_) => Ok(Value::Nil),
    })?;
    globals.set("env", env_fn)?;

    // print - Override print to use log::info
    let print_fn = lua.create_function(|_, args: mlua::Variadic<String>| {
        let msg = args
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\t");
        log::info!("[Lua] {}", msg);
        Ok(())
    })?;
    globals.set("print", print_fn)?;

    // Register async/await style helpers using coroutines
    register_async_helpers(lua)?;

    // Register parallel processing functions
    register_parallel_functions(lua, project_root, sandbox)?;

    Ok(())
}

/// Register async/await style helpers for coroutine-based concurrency
fn register_async_helpers(lua: &Lua) -> mlua::Result<()> {
    // Create async module
    let async_code = r#"
        local async = {}

        -- Create a task from a function (wraps in coroutine)
        function async.task(fn)
            return {
                _co = coroutine.create(fn),
                _completed = false,
                _result = nil,
            }
        end

        -- Run a task to completion
        function async.await(task)
            if task._completed then
                return task._result
            end
            while coroutine.status(task._co) ~= "dead" do
                local ok, result = coroutine.resume(task._co)
                if not ok then
                    error(result)
                end
                task._result = result
            end
            task._completed = true
            return task._result
        end

        -- Yield from current task (for cooperative multitasking)
        function async.yield(value)
            return coroutine.yield(value)
        end

        -- Run multiple tasks concurrently (interleaved execution)
        function async.all(tasks)
            local results = {}
            local pending = {}

            for i, task in ipairs(tasks) do
                pending[i] = task
                results[i] = nil
            end

            -- Round-robin execution until all complete
            local any_pending = true
            while any_pending do
                any_pending = false
                for i, task in ipairs(pending) do
                    if task and coroutine.status(task._co) ~= "dead" then
                        any_pending = true
                        local ok, result = coroutine.resume(task._co)
                        if not ok then
                            error(result)
                        end
                        task._result = result
                    elseif task then
                        results[i] = task._result
                        task._completed = true
                        pending[i] = nil
                    end
                end
            end

            return results
        end

        -- Run tasks and return first completed result
        function async.race(tasks)
            while true do
                for i, task in ipairs(tasks) do
                    if coroutine.status(task._co) ~= "dead" then
                        local ok, result = coroutine.resume(task._co)
                        if not ok then
                            error(result)
                        end
                        if coroutine.status(task._co) == "dead" then
                            task._result = result
                            task._completed = true
                            return result, i
                        end
                    end
                end
            end
        end

        -- Sleep/delay (yields N times for cooperative scheduling)
        function async.sleep(n)
            for _ = 1, (n or 1) do
                coroutine.yield()
            end
        end

        return async
    "#;

    let async_module: Table = lua.load(async_code).eval()?;
    lua.globals().set("async", async_module)?;

    Ok(())
}

/// Register parallel processing functions
fn register_parallel_functions(lua: &Lua, project_root: &Path, sandbox: bool) -> mlua::Result<()> {
    let parallel = lua.create_table()?;
    let root = project_root.to_path_buf();

    // parallel.load_json(paths) - Load multiple JSON files in parallel
    let root_clone = root.clone();
    let load_json_parallel = lua.create_function(move |lua, paths: Table| {
        use rayon::prelude::*;

        // Collect paths from Lua table
        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        // Process in parallel
        let results: Vec<Option<serde_json::Value>> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, &root_clone);
                if sandbox && !is_path_within_root(&resolved, &root_clone) {
                    return None;
                }
                std::fs::read_to_string(&resolved)
                    .ok()
                    .and_then(|content| serde_json::from_str(&content).ok())
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
    let read_files_parallel = lua.create_function(move |lua, paths: Table| {
        use rayon::prelude::*;

        let path_list: Vec<String> = paths
            .sequence_values::<String>()
            .filter_map(|r| r.ok())
            .collect();

        let results: Vec<Option<String>> = path_list
            .par_iter()
            .map(|path| {
                let resolved = resolve_path(path, &root_clone);
                if sandbox && !is_path_within_root(&resolved, &root_clone) {
                    return None;
                }
                std::fs::read_to_string(&resolved).ok()
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
        use rayon::prelude::*;

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

    // parallel.map(items, fn) - Map over items, calling Lua function (sequential fn calls, parallel-ready structure)
    let map_fn = lua.create_function(|lua, (items, func): (Table, Function)| {
        let result_table = lua.create_table()?;
        let mut i = 1;
        for v in items.sequence_values::<Value>().flatten() {
            let res: Value = func.call(v)?;
            result_table.set(i, res)?;
            i += 1;
        }
        Ok(result_table)
    })?;
    parallel.set("map", map_fn)?;

    // parallel.filter(items, fn) - Filter items using predicate function
    let filter_fn = lua.create_function(|lua, (items, func): (Table, Function)| {
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
    parallel.set("filter", filter_fn)?;

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

    lua.globals().set("parallel", parallel)?;
    Ok(())
}

/// Extract functions from a table (computed or filters)
fn extract_functions(
    lua: &Lua,
    config_table: &Table,
    key: &str,
) -> mlua::Result<HashMap<String, mlua::RegistryKey>> {
    let mut functions = HashMap::new();

    if let Ok(table) = config_table.get::<Table>(key) {
        for pair in table.pairs::<String, Function>() {
            let (name, func) = pair?;
            let registry_key = lua.create_registry_value(func)?;
            functions.insert(name, registry_key);
        }
    }

    Ok(functions)
}

/// Parse the config table into ConfigData
fn parse_config(
    lua: &Lua,
    table: &Table,
    sort_fns: &mut HashMap<String, mlua::RegistryKey>,
    section_filters: &mut HashMap<String, mlua::RegistryKey>,
) -> mlua::Result<ConfigData> {
    let site = parse_site_config(table)?;
    let seo = parse_seo_config(table)?;
    let build = parse_build_config(table)?;
    let images = parse_images_config(table)?;
    let highlight = parse_highlight_config(table)?;
    let paths = parse_paths_config(table)?;
    let templates = parse_templates_config(table)?;
    let permalinks = parse_permalinks_config(table)?;
    let encryption = parse_encryption_config(table)?;
    let graph = parse_graph_config(table)?;
    let rss = parse_rss_config(table)?;
    let text = parse_text_config(table)?;
    let sections = parse_sections_config(lua, table, sort_fns, section_filters)?;

    Ok(ConfigData {
        site,
        seo,
        build,
        images,
        highlight,
        paths,
        templates,
        permalinks,
        encryption,
        graph,
        rss,
        text,
        sections,
    })
}

fn parse_site_config(table: &Table) -> mlua::Result<SiteConfig> {
    let site: Table = table.get("site")?;

    Ok(SiteConfig {
        title: site.get("title").unwrap_or_default(),
        description: site.get("description").unwrap_or_default(),
        base_url: site.get("base_url").unwrap_or_default(),
        author: site.get("author").unwrap_or_default(),
    })
}

fn parse_seo_config(table: &Table) -> mlua::Result<SeoConfig> {
    let seo: Table = table.get("seo").unwrap_or_else(|_| table.clone());

    Ok(SeoConfig {
        twitter_handle: seo.get("twitter_handle").ok(),
        default_og_image: seo.get("default_og_image").ok(),
    })
}

fn parse_build_config(table: &Table) -> mlua::Result<BuildConfig> {
    let build: Table = table.get("build").unwrap_or_else(|_| table.clone());

    Ok(BuildConfig {
        output_dir: build
            .get("output_dir")
            .unwrap_or_else(|_| "dist".to_string()),
        minify_css: build.get("minify_css").unwrap_or(true),
        css_output: build
            .get("css_output")
            .unwrap_or_else(|_| "rs.css".to_string()),
    })
}

fn parse_images_config(table: &Table) -> mlua::Result<ImagesConfig> {
    let images: Table = table.get("images").unwrap_or_else(|_| table.clone());

    Ok(ImagesConfig {
        quality: images.get("quality").unwrap_or(85.0),
        scale_factor: images.get("scale_factor").unwrap_or(1.0),
    })
}

fn parse_highlight_config(table: &Table) -> mlua::Result<HighlightConfig> {
    let highlight: Table = table.get("highlight").unwrap_or_else(|_| table.clone());

    let names: Vec<String> = highlight
        .get::<Table>("names")
        .map(|t| {
            t.sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect()
        })
        .unwrap_or_default();

    Ok(HighlightConfig {
        names,
        class: highlight.get("class").unwrap_or_else(|_| "me".to_string()),
    })
}

fn parse_paths_config(table: &Table) -> mlua::Result<PathsConfig> {
    let paths: Table = table.get("paths").unwrap_or_else(|_| table.clone());

    let exclude: Vec<String> = paths
        .get::<Table>("exclude")
        .map(|t| {
            t.sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect()
        })
        .unwrap_or_default();

    Ok(PathsConfig {
        content: paths
            .get("content")
            .unwrap_or_else(|_| "content".to_string()),
        styles: paths.get("styles").unwrap_or_else(|_| "styles".to_string()),
        static_files: paths
            .get("static_files")
            .unwrap_or_else(|_| "static".to_string()),
        templates: paths
            .get("templates")
            .unwrap_or_else(|_| "templates".to_string()),
        home: paths.get("home").unwrap_or_else(|_| "index.md".to_string()),
        exclude,
        exclude_defaults: paths.get("exclude_defaults").unwrap_or(true),
        respect_gitignore: paths.get("respect_gitignore").unwrap_or(true),
    })
}

fn parse_templates_config(table: &Table) -> mlua::Result<TemplatesConfig> {
    let mut sections = HashMap::new();

    if let Ok(templates) = table.get::<Table>("templates") {
        for (k, v) in templates.pairs::<String, String>().flatten() {
            sections.insert(k, v);
        }
    }

    Ok(TemplatesConfig { sections })
}

fn parse_permalinks_config(table: &Table) -> mlua::Result<PermalinksConfig> {
    let mut sections = HashMap::new();

    if let Ok(permalinks) = table.get::<Table>("permalinks") {
        for (k, v) in permalinks.pairs::<String, String>().flatten() {
            sections.insert(k, v);
        }
    }

    Ok(PermalinksConfig { sections })
}

fn parse_encryption_config(table: &Table) -> mlua::Result<EncryptionConfig> {
    let encryption: Table = table.get("encryption").unwrap_or_else(|_| table.clone());

    Ok(EncryptionConfig {
        password_command: encryption.get("password_command").ok(),
        password: encryption.get("password").ok(),
    })
}

fn parse_graph_config(table: &Table) -> mlua::Result<GraphConfig> {
    let graph: Table = table.get("graph").unwrap_or_else(|_| table.clone());

    Ok(GraphConfig {
        enabled: graph.get("enabled").unwrap_or(true),
        template: graph
            .get("template")
            .unwrap_or_else(|_| "graph.html".to_string()),
        path: graph.get("path").unwrap_or_else(|_| "graph".to_string()),
    })
}

fn parse_rss_config(table: &Table) -> mlua::Result<RssConfig> {
    let rss: Table = table.get("rss").unwrap_or_else(|_| table.clone());

    let sections: Vec<String> = rss
        .get::<Table>("sections")
        .map(|t| {
            t.sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect()
        })
        .unwrap_or_default();

    Ok(RssConfig {
        enabled: rss.get("enabled").unwrap_or(true),
        filename: rss
            .get("filename")
            .unwrap_or_else(|_| "rss.xml".to_string()),
        sections,
        limit: rss.get("limit").unwrap_or(20),
        exclude_encrypted_blocks: rss.get("exclude_encrypted_blocks").unwrap_or(false),
    })
}

fn parse_text_config(table: &Table) -> mlua::Result<TextConfig> {
    let text: Table = table.get("text").unwrap_or_else(|_| table.clone());

    let sections: Vec<String> = text
        .get::<Table>("sections")
        .map(|t| {
            t.sequence_values::<String>()
                .filter_map(|r| r.ok())
                .collect()
        })
        .unwrap_or_default();

    Ok(TextConfig {
        enabled: text.get("enabled").unwrap_or(false),
        sections,
        exclude_encrypted: text.get("exclude_encrypted").unwrap_or(false),
        include_home: text.get("include_home").unwrap_or(true),
    })
}

fn parse_sections_config(
    lua: &Lua,
    table: &Table,
    sort_fns: &mut HashMap<String, mlua::RegistryKey>,
    filter_fns: &mut HashMap<String, mlua::RegistryKey>,
) -> mlua::Result<SectionsConfig> {
    let mut sections = HashMap::new();

    if let Ok(sections_table) = table.get::<Table>("sections") {
        for (name, section_table) in sections_table.pairs::<String, Table>().flatten() {
            let iterate = section_table
                .get("iterate")
                .unwrap_or_else(|_| "files".to_string());

            // Store sort function if provided
            if let Ok(func) = section_table.get::<mlua::Function>("sort") {
                let key = lua.create_registry_value(func)?;
                sort_fns.insert(name.clone(), key);
            }

            // Store filter function if provided
            if let Ok(func) = section_table.get::<mlua::Function>("filter") {
                let key = lua.create_registry_value(func)?;
                filter_fns.insert(name.clone(), key);
            }

            sections.insert(name, SectionConfig { iterate });
        }
    }

    Ok(SectionsConfig { sections })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_project_root() -> PathBuf {
        std::env::current_dir().unwrap()
    }

    #[test]
    fn test_minimal_lua_config() {
        let lua = Lua::new();
        let root = test_project_root();
        register_lua_functions(&lua, &root, false).unwrap();

        let config_str = r#"
            return {
                site = {
                    title = "Test Site",
                    description = "A test site",
                    base_url = "https://example.com",
                    author = "Test Author",
                },
                build = {
                    output_dir = "dist",
                },
            }
        "#;

        let table: Table = lua.load(config_str).eval().unwrap();
        let mut sort_fns = HashMap::new();
        let mut filter_fns = HashMap::new();
        let config = parse_config(&lua, &table, &mut sort_fns, &mut filter_fns).unwrap();

        assert_eq!(config.site.title, "Test Site");
        assert_eq!(config.site.base_url, "https://example.com");
        assert_eq!(config.build.output_dir, "dist");
    }

    #[test]
    fn test_lua_config_with_sections() {
        let lua = Lua::new();
        let root = test_project_root();
        register_lua_functions(&lua, &root, false).unwrap();

        let config_str = r#"
            return {
                site = {
                    title = "Test",
                    description = "Test",
                    base_url = "https://example.com",
                    author = "Test",
                },
                build = { output_dir = "dist" },
                sections = {
                    problems = { iterate = "directories" },
                    blog = { iterate = "files" },
                },
            }
        "#;

        let table: Table = lua.load(config_str).eval().unwrap();
        let mut sort_fns = HashMap::new();
        let mut filter_fns = HashMap::new();
        let config = parse_config(&lua, &table, &mut sort_fns, &mut filter_fns).unwrap();

        let problems = config.sections.sections.get("problems");
        assert!(problems.is_some());
        assert_eq!(problems.unwrap().iterate, "directories");

        let blog = config.sections.sections.get("blog");
        assert!(blog.is_some());
        assert_eq!(blog.unwrap().iterate, "files");
    }

    #[test]
    fn test_lua_helper_functions() {
        let lua = Lua::new();
        let root = test_project_root();
        register_lua_functions(&lua, &root, false).unwrap();

        // Test file_exists
        let result: bool = lua.load("return file_exists('Cargo.toml')").eval().unwrap();
        assert!(result);

        let result: bool = lua
            .load("return file_exists('nonexistent.file')")
            .eval()
            .unwrap();
        assert!(!result);
    }

    #[test]
    fn test_sandbox_blocks_outside_access() {
        let lua = Lua::new();
        let root = test_project_root();
        register_lua_functions(&lua, &root, true).unwrap();

        // Trying to access /etc/passwd should fail with sandbox enabled
        let result = lua.load("return read_file('/etc/passwd')").eval::<Value>();
        assert!(result.is_err());

        // Trying to access parent directory should fail
        let result = lua.load("return read_file('../some_file')").eval::<Value>();
        assert!(result.is_err());
    }

    #[test]
    fn test_sandbox_allows_project_access() {
        let lua = Lua::new();
        let root = test_project_root();
        register_lua_functions(&lua, &root, true).unwrap();

        // Accessing files within project should work
        let result: bool = lua.load("return file_exists('Cargo.toml')").eval().unwrap();
        assert!(result);

        // Reading files within project should work
        let result = lua.load("return read_file('Cargo.toml')").eval::<Value>();
        assert!(result.is_ok());
    }
}
