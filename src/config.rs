//! Configuration loader for rs-web
//!
//! This module provides Lua-based configuration.
//! Config files are written in Lua and can include computed values and custom filters.

use anyhow::{Context, Result};
use mlua::{Function, Lua, LuaSerdeExt, Table, Value};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::lua::{AssetManifest, create_manifest};
use crate::tracker::{BuildTracker, SharedTracker};

/// Configuration data structure (deserializable from Lua)
#[derive(Debug, Clone)]
pub struct ConfigData {
    pub site: SiteConfig,
    pub seo: SeoConfig,
    pub build: BuildConfig,
    pub paths: PathsConfig,
}

/// Main configuration structure with embedded Lua state
pub struct Config {
    // Configuration data
    pub data: ConfigData,

    // Lua runtime state
    lua: Lua,
    before_build: Option<mlua::RegistryKey>,
    after_build: Option<mlua::RegistryKey>,

    // Data-driven page generation
    data_fn: Option<mlua::RegistryKey>,
    pages_fn: Option<mlua::RegistryKey>,
    update_data_fn: Option<mlua::RegistryKey>,

    // Build dependency tracker
    tracker: SharedTracker,

    // Asset manifest for hashed filenames
    pub asset_manifest: AssetManifest,
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
}

#[derive(Debug, Deserialize, Clone)]
pub struct PathsConfig {
    #[serde(default = "default_templates_dir")]
    pub templates: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            templates: default_templates_dir(),
        }
    }
}

fn default_templates_dir() -> String {
    "templates".to_string()
}

/// Page definition
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PageDef {
    /// URL path (e.g., "/blog/hello/") - must end with /
    pub path: String,
    /// Template file to use (e.g., "post.html"). If not set, outputs html directly.
    #[serde(default)]
    pub template: Option<String>,
    /// Page title (for `<title>` and ctx.page.title)
    #[serde(default)]
    pub title: Option<String>,
    /// Meta description
    #[serde(default)]
    pub description: Option<String>,
    /// OG image path
    #[serde(default)]
    pub image: Option<String>,
    /// Raw markdown content to render (becomes ctx.page.content as HTML)
    #[serde(default)]
    pub content: Option<String>,
    /// Pre-rendered HTML (skips markdown processing)
    #[serde(default)]
    pub html: Option<String>,
    /// Page-specific data (available as ctx.page.data.*)
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    /// Whether to minify HTML output (default: true)
    #[serde(default = "default_minify")]
    pub minify: bool,
}

fn default_minify() -> bool {
    true
}

impl Config {
    /// Create a Config with default Lua state (for testing)
    #[cfg(test)]
    pub fn from_data(data: ConfigData) -> Self {
        let lua = Lua::new();
        Self {
            data,
            lua,
            before_build: None,
            after_build: None,
            data_fn: None,
            pages_fn: None,
            update_data_fn: None,
            tracker: Arc::new(BuildTracker::disabled()),
            asset_manifest: create_manifest(),
        }
    }

    /// Load config from a Lua file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::load_with_tracker(path, Arc::new(BuildTracker::new()))
    }

    /// Load config with a custom tracker
    pub fn load_with_tracker<P: AsRef<Path>>(path: P, tracker: SharedTracker) -> Result<Self> {
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

        // Create asset manifest early so it can be used in register calls
        let asset_manifest = create_manifest();

        // First pass: register functions without sandbox to load config
        crate::lua::register(
            &lua,
            &project_root,
            false,
            tracker.clone(),
            None,
            asset_manifest.clone(),
        )
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
            crate::lua::register(
                &lua,
                &project_root,
                true,
                tracker.clone(),
                None,
                asset_manifest.clone(),
            )
            .map_err(|e| anyhow::anyhow!("Failed to register Lua functions: {}", e))?;
        }

        // Parse the config table
        let data = parse_config(&lua, &config_table)
            .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))?;

        // Extract data-driven functions
        let data_fn: Option<mlua::RegistryKey> = config_table
            .get::<Function>("data")
            .ok()
            .map(|f| lua.create_registry_value(f))
            .transpose()
            .map_err(|e| anyhow::anyhow!("Failed to store data function: {}", e))?;

        let pages_fn: Option<mlua::RegistryKey> = config_table
            .get::<Function>("pages")
            .ok()
            .map(|f| lua.create_registry_value(f))
            .transpose()
            .map_err(|e| anyhow::anyhow!("Failed to store pages function: {}", e))?;

        let update_data_fn: Option<mlua::RegistryKey> = config_table
            .get::<Function>("update_data")
            .ok()
            .map(|f| lua.create_registry_value(f))
            .transpose()
            .map_err(|e| anyhow::anyhow!("Failed to store update_data function: {}", e))?;

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
            before_build,
            after_build,
            data_fn,
            pages_fn,
            update_data_fn,
            tracker,
            asset_manifest,
        })
    }

    /// Get a reference to the build tracker
    pub fn tracker(&self) -> &SharedTracker {
        &self.tracker
    }

    /// Call before_build hook with ctx
    pub fn call_before_build(&self) -> Result<()> {
        if let Some(ref key) = self.before_build {
            let func: Function = self
                .lua
                .registry_value(key)
                .map_err(|e| anyhow::anyhow!("Failed to get before_build: {}", e))?;
            let ctx = self.create_ctx(None)?;
            func.call::<()>(ctx)
                .map_err(|e| anyhow::anyhow!("before_build hook failed: {}", e))?;
        }
        Ok(())
    }

    /// Call after_build hook with ctx
    pub fn call_after_build(&self) -> Result<()> {
        if let Some(ref key) = self.after_build {
            let func: Function = self
                .lua
                .registry_value(key)
                .map_err(|e| anyhow::anyhow!("Failed to get after_build: {}", e))?;
            let ctx = self.create_ctx(None)?;
            func.call::<()>(ctx)
                .map_err(|e| anyhow::anyhow!("after_build hook failed: {}", e))?;
        }
        Ok(())
    }

    /// Create context table for Lua functions
    fn create_ctx(&self, data: Option<&serde_json::Value>) -> Result<Value> {
        let ctx = self
            .lua
            .create_table()
            .map_err(|e| anyhow::anyhow!("Failed to create ctx: {}", e))?;

        ctx.set("output_dir", self.data.build.output_dir.as_str())
            .map_err(|e| anyhow::anyhow!("Failed to set output_dir: {}", e))?;
        ctx.set("base_url", self.data.site.base_url.as_str())
            .map_err(|e| anyhow::anyhow!("Failed to set base_url: {}", e))?;

        if let Some(data) = data {
            let data_value: Value = self
                .lua
                .to_value(data)
                .map_err(|e| anyhow::anyhow!("Failed to convert data to Lua: {}", e))?;
            ctx.set("data", data_value)
                .map_err(|e| anyhow::anyhow!("Failed to set data: {}", e))?;
        }

        Ok(Value::Table(ctx))
    }

    /// Call the data(ctx) function to get global template data
    pub fn call_data(&self) -> Result<serde_json::Value> {
        let key = match &self.data_fn {
            Some(k) => k,
            _ => return Ok(serde_json::Value::Object(serde_json::Map::new())),
        };

        let func: Function = self
            .lua
            .registry_value(key)
            .map_err(|e| anyhow::anyhow!("Failed to get data function: {}", e))?;

        let ctx = self.create_ctx(None)?;

        let result: Value = func
            .call(ctx)
            .map_err(|e| anyhow::anyhow!("Failed to call data(): {}", e))?;
        let json_value: serde_json::Value = self
            .lua
            .from_value(result)
            .map_err(|e| anyhow::anyhow!("Failed to convert data() result: {}", e))?;

        Ok(json_value)
    }

    /// Call the pages(ctx) function to get page definitions
    /// ctx.data contains the result from data()
    pub fn call_pages(&self, global_data: &serde_json::Value) -> Result<Vec<PageDef>> {
        let key = match &self.pages_fn {
            Some(k) => k,
            _ => return Ok(Vec::new()),
        };

        let func: Function = self
            .lua
            .registry_value(key)
            .map_err(|e| anyhow::anyhow!("Failed to get pages function: {}", e))?;

        let ctx = self.create_ctx(Some(global_data))?;

        let result: Value = func
            .call(ctx)
            .map_err(|e| anyhow::anyhow!("Failed to call pages(): {}", e))?;
        let pages: Vec<PageDef> = self
            .lua
            .from_value(result)
            .map_err(|e| anyhow::anyhow!("Failed to convert pages() result: {}", e))?;

        Ok(pages)
    }

    /// Check if update_data function is defined
    pub fn has_update_data(&self) -> bool {
        self.update_data_fn.is_some()
    }

    /// Call update_data(ctx) for incremental updates
    /// ctx.data = cached data, ctx.changed_paths = list of changed paths
    pub fn call_update_data(
        &self,
        cached_data: &serde_json::Value,
        changed_paths: &[std::path::PathBuf],
    ) -> Result<serde_json::Value> {
        let key = match &self.update_data_fn {
            Some(k) => k,
            None => return Err(anyhow::anyhow!("update_data function not defined")),
        };

        let func: Function = self
            .lua
            .registry_value(key)
            .map_err(|e| anyhow::anyhow!("Failed to get update_data function: {}", e))?;

        // Create ctx with cached data and changed paths
        let ctx = self
            .lua
            .create_table()
            .map_err(|e| anyhow::anyhow!("Failed to create ctx: {}", e))?;

        ctx.set("output_dir", self.data.build.output_dir.as_str())
            .map_err(|e| anyhow::anyhow!("Failed to set output_dir: {}", e))?;
        ctx.set("base_url", self.data.site.base_url.as_str())
            .map_err(|e| anyhow::anyhow!("Failed to set base_url: {}", e))?;

        // Set cached data as ctx.data
        let cached: Value = self
            .lua
            .to_value(cached_data)
            .map_err(|e| anyhow::anyhow!("Failed to convert cached data to Lua: {}", e))?;
        ctx.set("data", cached)
            .map_err(|e| anyhow::anyhow!("Failed to set data: {}", e))?;

        // Set changed paths as ctx.changed_paths
        let paths_table = self.lua.create_table()?;
        for (i, path) in changed_paths.iter().enumerate() {
            paths_table.set(i + 1, path.to_string_lossy().to_string())?;
        }
        ctx.set("changed_paths", paths_table)
            .map_err(|e| anyhow::anyhow!("Failed to set changed_paths: {}", e))?;

        let result: Value = func
            .call(Value::Table(ctx))
            .map_err(|e| anyhow::anyhow!("Failed to call update_data(): {}", e))?;

        let json_value: serde_json::Value = self
            .lua
            .from_value(result)
            .map_err(|e| anyhow::anyhow!("Failed to convert update_data() result: {}", e))?;

        Ok(json_value)
    }
}
/// Parse the config table into ConfigData
fn parse_config(_lua: &Lua, table: &Table) -> mlua::Result<ConfigData> {
    let site = parse_site_config(table)?;
    let seo = parse_seo_config(table)?;
    let build = parse_build_config(table)?;
    let paths = parse_paths_config(table)?;

    Ok(ConfigData {
        site,
        seo,
        build,
        paths,
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
    })
}

fn parse_paths_config(table: &Table) -> mlua::Result<PathsConfig> {
    let paths: Table = table.get("paths").unwrap_or_else(|_| table.clone());

    Ok(PathsConfig {
        templates: paths
            .get("templates")
            .unwrap_or_else(|_| "templates".to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_project_root() -> PathBuf {
        std::env::current_dir().expect("failed to get current directory")
    }

    #[test]
    fn test_minimal_lua_config() {
        let lua = Lua::new();
        let root = test_project_root();
        crate::lua::register(
            &lua,
            &root,
            false,
            Arc::new(BuildTracker::disabled()),
            None,
            create_manifest(),
        )
        .expect("failed to register Lua functions");

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

        let table: Table = lua
            .load(config_str)
            .eval()
            .expect("failed to load config string");
        let config = parse_config(&lua, &table).expect("failed to parse config");

        assert_eq!(config.site.title, "Test Site");
        assert_eq!(config.site.base_url, "https://example.com");
        assert_eq!(config.build.output_dir, "dist");
    }

    #[test]
    fn test_lua_helper_functions() {
        let lua = Lua::new();
        let root = test_project_root();
        crate::lua::register(
            &lua,
            &root,
            false,
            Arc::new(BuildTracker::disabled()),
            None,
            create_manifest(),
        )
        .expect("failed to register Lua functions");

        // Test file_exists
        let result: bool = lua
            .load("return rs.file_exists('Cargo.toml')")
            .eval()
            .expect("failed to eval file_exists for Cargo.toml");
        assert!(result);

        let result: bool = lua
            .load("return rs.file_exists('nonexistent.file')")
            .eval()
            .expect("failed to eval file_exists for nonexistent.file");
        assert!(!result);
    }

    #[test]
    fn test_sandbox_blocks_outside_access() {
        let lua = Lua::new();
        let root = test_project_root();
        crate::lua::register(
            &lua,
            &root,
            true,
            Arc::new(BuildTracker::disabled()),
            None,
            create_manifest(),
        )
        .expect("failed to register Lua functions");

        // Trying to access /etc/passwd should fail with sandbox enabled
        let result = lua
            .load("return rs.read_file('/etc/passwd')")
            .eval::<Value>();
        assert!(
            result.is_err(),
            "sandbox should block access to /etc/passwd"
        );

        // Trying to access parent directory should fail
        let result = lua
            .load("return rs.read_file('../some_file')")
            .eval::<Value>();
        assert!(
            result.is_err(),
            "sandbox should block access to parent directory"
        );
    }

    #[test]
    fn test_sandbox_allows_project_access() {
        let lua = Lua::new();
        let root = test_project_root();
        crate::lua::register(
            &lua,
            &root,
            true,
            Arc::new(BuildTracker::disabled()),
            None,
            create_manifest(),
        )
        .expect("failed to register Lua functions");

        // Accessing files within project should work
        let result: bool = lua
            .load("return rs.file_exists('Cargo.toml')")
            .eval()
            .expect("sandbox should allow file_exists within project");
        assert!(result);

        // Reading files within project should work
        let result = lua
            .load("return rs.read_file('Cargo.toml')")
            .eval::<Value>();
        assert!(
            result.is_ok(),
            "sandbox should allow reading files within project"
        );
    }
}
