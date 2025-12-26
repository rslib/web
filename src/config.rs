use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub site: SiteConfig,
    pub seo: SeoConfig,
    pub build: BuildConfig,
    pub images: ImagesConfig,
    #[serde(default)]
    pub highlight: HighlightConfig,
    #[serde(default)]
    pub paths: PathsConfig,
    #[serde(default)]
    pub templates: TemplatesConfig,
    #[serde(default)]
    pub permalinks: PermalinksConfig,
    #[serde(default)]
    pub encryption: EncryptionConfig,
    #[serde(default)]
    pub graph: GraphConfig,
    #[serde(default)]
    pub rss: RssConfig,
    #[serde(default)]
    pub text: TextConfig,
}

/// RSS feed config
#[derive(Debug, Deserialize, Clone)]
pub struct RssConfig {
    /// Enable RSS generation
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Output filename
    #[serde(default = "default_rss_filename")]
    pub filename: String,
    /// Sections to include (empty = all)
    #[serde(default)]
    pub sections: Vec<String>,
    /// Maximum number of items
    #[serde(default = "default_rss_limit")]
    pub limit: usize,
    /// Exclude posts with encrypted blocks
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

/// Plain text output config for curl-friendly pages
#[derive(Debug, Deserialize, Clone)]
pub struct TextConfig {
    /// Enable text generation (default: false)
    #[serde(default)]
    pub enabled: bool,
    /// Sections to include (empty = all)
    #[serde(default)]
    pub sections: Vec<String>,
    /// Exclude posts with encrypted content
    #[serde(default)]
    pub exclude_encrypted: bool,
    /// Include home page
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

/// Graph visualization config
#[derive(Debug, Deserialize, Clone)]
pub struct GraphConfig {
    /// Enable graph generation
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Template for the full graph page
    #[serde(default = "default_graph_template")]
    pub template: String,
    /// Output path for the graph page (e.g., "graph" -> /graph/)
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

/// Template mapping: section name -> template file
#[derive(Debug, Deserialize, Clone, Default)]
pub struct TemplatesConfig {
    #[serde(flatten)]
    pub sections: HashMap<String, String>,
}

/// Permalink patterns: section name -> pattern
/// Patterns can use: :year, :month, :day, :slug, :title, :section
#[derive(Debug, Deserialize, Clone, Default)]
pub struct PermalinksConfig {
    #[serde(flatten)]
    pub sections: HashMap<String, String>,
}

/// Encryption config for password-protected posts
/// Password resolution order: SITE_PASSWORD env var → password_command → password
#[derive(Debug, Deserialize, Clone, Default)]
pub struct EncryptionConfig {
    /// Command to execute to get the password (e.g., "pass show website/encrypted-notes")
    pub password_command: Option<String>,
    /// Raw password (less secure, prefer env var or command)
    pub password: Option<String>,
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
    /// Patterns to exclude (supports regex). Matches both directories and files.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Include default exclusions like README.md, LICENSE.md, etc. (default: true)
    #[serde(default = "default_true")]
    pub exclude_defaults: bool,
    /// Respect .gitignore when discovering content (default: true)
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
pub struct SiteConfig {
    pub title: String,
    pub description: String,
    pub base_url: String,
    pub author: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SeoConfig {
    pub twitter_handle: Option<String>,
    pub default_og_image: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BuildConfig {
    #[allow(dead_code)]
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

fn default_true() -> bool {
    true
}

fn default_quality() -> f32 {
    85.0
}

fn default_scale_factor() -> f64 {
    1.0
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {:?}", path.as_ref()))?;

        let config: Config =
            toml::from_str(&content).with_context(|| "Failed to parse config file")?;

        Ok(config)
    }

    /// Parse config from string (for testing)
    #[cfg(test)]
    pub fn from_str(content: &str) -> Result<Self> {
        let config: Config = toml::from_str(content).with_context(|| "Failed to parse config")?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_config() -> &'static str {
        r#"
[site]
title = "Test Site"
description = "A test site"
base_url = "https://example.com"
author = "Test Author"

[seo]

[build]
output_dir = "dist"

[images]
"#
    }

    #[test]
    fn test_minimal_config() {
        let config = Config::from_str(minimal_config()).unwrap();
        assert_eq!(config.site.title, "Test Site");
        assert_eq!(config.site.base_url, "https://example.com");
    }

    #[test]
    fn test_config_with_permalinks() {
        let content = format!(
            r#"{}
[permalinks]
blog = "/:year/:month/:slug/"
projects = "/:slug/"
"#,
            minimal_config()
        );

        let config = Config::from_str(&content).unwrap();
        assert_eq!(
            config.permalinks.sections.get("blog"),
            Some(&"/:year/:month/:slug/".to_string())
        );
        assert_eq!(
            config.permalinks.sections.get("projects"),
            Some(&"/:slug/".to_string())
        );
    }

    #[test]
    fn test_config_with_templates() {
        let content = format!(
            r#"{}
[templates]
blog = "post.html"
projects = "project.html"
"#,
            minimal_config()
        );

        let config = Config::from_str(&content).unwrap();
        assert_eq!(
            config.templates.sections.get("blog"),
            Some(&"post.html".to_string())
        );
        assert_eq!(
            config.templates.sections.get("projects"),
            Some(&"project.html".to_string())
        );
    }

    #[test]
    fn test_config_with_paths() {
        let content = format!(
            r#"{}
[paths]
content = "my-content"
styles = "my-styles"
static_files = "my-static"
templates = "my-templates"
home = "home.md"
exclude = ["drafts", "private"]
"#,
            minimal_config()
        );

        let config = Config::from_str(&content).unwrap();
        assert_eq!(config.paths.content, "my-content");
        assert_eq!(config.paths.styles, "my-styles");
        assert_eq!(config.paths.static_files, "my-static");
        assert_eq!(config.paths.templates, "my-templates");
        assert_eq!(config.paths.home, "home.md");
        assert_eq!(config.paths.exclude, vec!["drafts", "private"]);
    }

    #[test]
    fn test_config_defaults() {
        let config = Config::from_str(minimal_config()).unwrap();

        // Paths defaults
        assert_eq!(config.paths.content, "content");
        assert_eq!(config.paths.styles, "styles");
        assert_eq!(config.paths.static_files, "static");
        assert_eq!(config.paths.templates, "templates");
        assert_eq!(config.paths.home, "index.md");
        assert!(config.paths.exclude.is_empty());
        assert!(config.paths.exclude_defaults);
        assert!(config.paths.respect_gitignore);

        // Templates and permalinks default to empty
        assert!(config.templates.sections.is_empty());
        assert!(config.permalinks.sections.is_empty());

        // Build defaults
        assert!(config.build.minify_css);

        // Images defaults
        assert_eq!(config.images.quality, 85.0);
        assert_eq!(config.images.scale_factor, 1.0);
    }

    #[test]
    fn test_config_with_highlight() {
        let content = format!(
            r#"{}
[highlight]
names = ["John Doe", "Jane Doe"]
class = "author"
"#,
            minimal_config()
        );

        let config = Config::from_str(&content).unwrap();
        assert_eq!(config.highlight.names, vec!["John Doe", "Jane Doe"]);
        assert_eq!(config.highlight.class, "author");
    }

    #[test]
    fn test_config_with_encryption() {
        let content = format!(
            r#"{}
[encryption]
password_command = "pass show website/notes"
"#,
            minimal_config()
        );

        let config = Config::from_str(&content).unwrap();
        assert_eq!(
            config.encryption.password_command,
            Some("pass show website/notes".to_string())
        );
        assert!(config.encryption.password.is_none());
    }

    #[test]
    fn test_config_encryption_with_raw_password() {
        let content = format!(
            r#"{}
[encryption]
password = "secret123"
"#,
            minimal_config()
        );

        let config = Config::from_str(&content).unwrap();
        assert!(config.encryption.password_command.is_none());
        assert_eq!(config.encryption.password, Some("secret123".to_string()));
    }

    #[test]
    fn test_config_encryption_defaults_to_none() {
        let config = Config::from_str(minimal_config()).unwrap();
        assert!(config.encryption.password_command.is_none());
        assert!(config.encryption.password.is_none());
    }

    #[test]
    fn test_config_graph_defaults() {
        let config = Config::from_str(minimal_config()).unwrap();
        assert!(config.graph.enabled);
        assert_eq!(config.graph.template, "graph.html");
        assert_eq!(config.graph.path, "graph");
    }

    #[test]
    fn test_config_with_graph() {
        let content = format!(
            r#"{}
[graph]
enabled = false
template = "custom-graph.html"
path = "brain"
"#,
            minimal_config()
        );

        let config = Config::from_str(&content).unwrap();
        assert!(!config.graph.enabled);
        assert_eq!(config.graph.template, "custom-graph.html");
        assert_eq!(config.graph.path, "brain");
    }

    #[test]
    fn test_config_rss_defaults() {
        let config = Config::from_str(minimal_config()).unwrap();
        assert!(config.rss.enabled);
        assert_eq!(config.rss.filename, "rss.xml");
        assert!(config.rss.sections.is_empty());
        assert_eq!(config.rss.limit, 20);
        assert!(!config.rss.exclude_encrypted_blocks);
    }

    #[test]
    fn test_config_with_rss() {
        let content = format!(
            r#"{}
[rss]
enabled = true
filename = "feed.xml"
sections = ["blog", "notes"]
limit = 50
exclude_encrypted_blocks = true
"#,
            minimal_config()
        );

        let config = Config::from_str(&content).unwrap();
        assert!(config.rss.enabled);
        assert_eq!(config.rss.filename, "feed.xml");
        assert_eq!(config.rss.sections, vec!["blog", "notes"]);
        assert_eq!(config.rss.limit, 50);
        assert!(config.rss.exclude_encrypted_blocks);
    }

    #[test]
    fn test_config_text_defaults() {
        let config = Config::from_str(minimal_config()).unwrap();
        assert!(!config.text.enabled); // Disabled by default
        assert!(config.text.sections.is_empty());
        assert!(!config.text.exclude_encrypted);
        assert!(config.text.include_home);
    }

    #[test]
    fn test_config_with_text() {
        let content = format!(
            r#"{}
[text]
enabled = true
sections = ["blog", "notes"]
exclude_encrypted = true
include_home = false
"#,
            minimal_config()
        );

        let config = Config::from_str(&content).unwrap();
        assert!(config.text.enabled);
        assert_eq!(config.text.sections, vec!["blog", "notes"]);
        assert!(config.text.exclude_encrypted);
        assert!(!config.text.include_home);
    }

    #[test]
    fn test_config_text_enabled_only() {
        let content = format!(
            r#"{}
[text]
enabled = true
"#,
            minimal_config()
        );

        let config = Config::from_str(&content).unwrap();
        assert!(config.text.enabled);
        assert!(config.text.sections.is_empty()); // All sections
        assert!(!config.text.exclude_encrypted);
        assert!(config.text.include_home);
    }
}
