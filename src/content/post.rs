use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::frontmatter::{Frontmatter, parse_frontmatter};
use crate::config::Config;
use crate::encryption::EncryptedContent;

/// Content type of the post source file
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ContentType {
    /// Markdown file (.md) - processed through markdown pipeline
    Markdown,
    /// HTML file (.html) - processed through Tera templating
    Html,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Post {
    /// Slug derived from filename (with date prefix stripped)
    pub file_slug: String,
    pub section: String,
    pub frontmatter: Frontmatter,
    pub content: String,
    pub html: String,
    pub reading_time: u32,
    pub word_count: usize,
    /// Encrypted content data (set when frontmatter.encrypted is true)
    pub encrypted_content: Option<EncryptedContent>,
    /// Whether this post has :::encrypted blocks (partial encryption)
    pub has_encrypted_blocks: bool,
    /// Source file type (Markdown or HTML)
    pub content_type: ContentType,
    /// Path to the source file
    pub source_path: PathBuf,
    /// Path to the source directory (for directory-based posts)
    /// This is set when iterate = "directories" and allows templates to access
    /// the full directory path for loading additional files via Tera functions
    pub source_dir: Option<PathBuf>,
}

impl Post {
    pub fn from_file_with_section<P: AsRef<Path>>(path: P, section: &str) -> Result<Self> {
        let path = path.as_ref();
        let raw_content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read post: {:?}", path))?;

        let (frontmatter, content) = parse_frontmatter(&raw_content)?;

        // Determine content type from file extension
        let content_type = match path.extension().and_then(|e| e.to_str()) {
            Some("html") | Some("htm") => ContentType::Html,
            _ => ContentType::Markdown,
        };

        // Extract slug from filename (e.g., "2024-01-15-my-post.md" -> "my-post")
        let file_slug = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| {
                // Remove date prefix if present (YYYY-MM-DD-)
                if s.len() > 11 && s.chars().nth(4) == Some('-') && s.chars().nth(7) == Some('-') {
                    s[11..].to_string()
                } else {
                    s.to_string()
                }
            })
            .unwrap_or_else(|| "untitled".to_string());

        let word_count = content.split_whitespace().count();
        let reading_time = (word_count / 200).max(1) as u32; // ~200 words per minute

        Ok(Self {
            file_slug,
            section: section.to_string(),
            frontmatter,
            content: content.to_string(),
            html: String::new(), // Will be filled by markdown pipeline or Tera
            reading_time,
            word_count,
            encrypted_content: None, // Will be filled if frontmatter.encrypted is true
            has_encrypted_blocks: false, // Will be set if :::encrypted blocks found
            content_type,
            source_path: path.to_path_buf(),
            source_dir: None, // Set by caller for directory-based posts
        })
    }

    /// Get the effective slug (frontmatter override or file-based)
    pub fn slug(&self) -> &str {
        self.frontmatter.slug.as_deref().unwrap_or(&self.file_slug)
    }

    /// Get the slugified title
    fn title_slug(&self) -> String {
        self.frontmatter
            .title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }

    /// Resolve a permalink pattern to an actual URL
    /// Patterns: :year, :month, :day, :slug, :title, :section
    fn resolve_pattern(&self, pattern: &str) -> String {
        let mut url = pattern.to_string();

        // Replace :section
        url = url.replace(":section", &self.section);

        // Replace :slug
        url = url.replace(":slug", self.slug());

        // Replace :title
        url = url.replace(":title", &self.title_slug());

        // Replace date parts if date exists
        if let Some(date) = self.frontmatter.date {
            url = url.replace(":year", &date.format("%Y").to_string());
            url = url.replace(":month", &date.format("%m").to_string());
            url = url.replace(":day", &date.format("%d").to_string());
        } else {
            // Remove date patterns if no date
            url = url.replace(":year", "");
            url = url.replace(":month", "");
            url = url.replace(":day", "");
        }

        // Clean up double slashes
        while url.contains("//") {
            url = url.replace("//", "/");
        }

        // Ensure leading slash
        if !url.starts_with('/') {
            url = format!("/{}", url);
        }

        // Ensure trailing slash
        if !url.ends_with('/') {
            url = format!("{}/", url);
        }

        url
    }

    /// Get the URL for this post, resolving permalink patterns
    /// Priority: frontmatter permalink > config pattern > default
    pub fn url(&self, config: &Config) -> String {
        // 1. Frontmatter permalink (highest priority)
        if let Some(permalink) = &self.frontmatter.permalink {
            return self.resolve_pattern(permalink);
        }

        // 2. Config pattern for this section
        if let Some(pattern) = config.permalinks.sections.get(&self.section) {
            return self.resolve_pattern(pattern);
        }

        // 3. Default: /:section/:slug/
        self.resolve_pattern("/:section/:slug/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::frontmatter::Frontmatter;
    use chrono::NaiveDate;

    fn make_post(section: &str, file_slug: &str, frontmatter: Frontmatter) -> Post {
        Post {
            file_slug: file_slug.to_string(),
            section: section.to_string(),
            frontmatter,
            content: String::new(),
            html: String::new(),
            reading_time: 1,
            word_count: 100,
            encrypted_content: None,
            has_encrypted_blocks: false,
            content_type: ContentType::Markdown,
            source_path: PathBuf::new(),
            source_dir: None,
        }
    }

    fn make_frontmatter(title: &str, date: Option<NaiveDate>) -> Frontmatter {
        Frontmatter {
            title: title.to_string(),
            description: None,
            date,
            tags: None,
            draft: None,
            image: None,
            template: None,
            slug: None,
            permalink: None,
            encrypted: false,
            password: None,
        }
    }

    fn make_config() -> Config {
        Config::from_data(crate::config::ConfigData {
            site: crate::config::SiteConfig {
                title: "Test".to_string(),
                description: "Test".to_string(),
                base_url: "https://example.com".to_string(),
                author: "Test".to_string(),
            },
            seo: crate::config::SeoConfig {
                twitter_handle: None,
                default_og_image: None,
            },
            build: crate::config::BuildConfig {
                output_dir: "dist".to_string(),
                minify_css: false,
                css_output: "rs.css".to_string(),
            },
            images: crate::config::ImagesConfig {
                quality: 85.0,
                scale_factor: 1.0,
            },
            highlight: Default::default(),
            paths: Default::default(),
            templates: Default::default(),
            permalinks: Default::default(),
            encryption: Default::default(),
            graph: Default::default(),
            rss: Default::default(),
            text: Default::default(),
            sections: Default::default(),
        })
    }

    #[test]
    fn test_default_permalink() {
        let fm = make_frontmatter("Hello World", None);
        let post = make_post("blog", "hello-world", fm);
        let config = make_config();

        assert_eq!(post.url(&config), "/blog/hello-world/");
    }

    #[test]
    fn test_permalink_with_date() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let fm = make_frontmatter("Hello World", Some(date));
        let post = make_post("blog", "hello-world", fm);

        assert_eq!(
            post.resolve_pattern("/:year/:month/:day/:slug/"),
            "/2024/01/15/hello-world/"
        );
        assert_eq!(
            post.resolve_pattern("/:year/:month/:slug/"),
            "/2024/01/hello-world/"
        );
        assert_eq!(post.resolve_pattern("/:year/:slug/"), "/2024/hello-world/");
    }

    #[test]
    fn test_permalink_with_section() {
        let fm = make_frontmatter("Hello World", None);
        let post = make_post("projects", "my-project", fm);

        assert_eq!(
            post.resolve_pattern("/:section/:slug/"),
            "/projects/my-project/"
        );
    }

    #[test]
    fn test_permalink_with_title() {
        let fm = make_frontmatter("Hello World!", None);
        let post = make_post("blog", "hello-world", fm);

        assert_eq!(post.resolve_pattern("/:title/"), "/hello-world/");
    }

    #[test]
    fn test_frontmatter_slug_override() {
        let mut fm = make_frontmatter("Hello World", None);
        fm.slug = Some("custom-slug".to_string());
        let post = make_post("blog", "hello-world", fm);
        let config = make_config();

        assert_eq!(post.slug(), "custom-slug");
        assert_eq!(post.url(&config), "/blog/custom-slug/");
    }

    #[test]
    fn test_frontmatter_permalink_override() {
        let mut fm = make_frontmatter("Hello World", None);
        fm.permalink = Some("/custom/path/".to_string());
        let post = make_post("blog", "hello-world", fm);
        let config = make_config();

        assert_eq!(post.url(&config), "/custom/path/");
    }

    #[test]
    fn test_frontmatter_permalink_pattern() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let mut fm = make_frontmatter("Hello World", Some(date));
        fm.permalink = Some("/:year/:month/:slug/".to_string());
        let post = make_post("blog", "hello-world", fm);
        let config = make_config();

        assert_eq!(post.url(&config), "/2024/01/hello-world/");
    }

    #[test]
    fn test_config_permalink_pattern() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let fm = make_frontmatter("Hello World", Some(date));
        let post = make_post("blog", "hello-world", fm);

        let mut config = make_config();
        config
            .permalinks
            .sections
            .insert("blog".to_string(), "/:year/:month/:slug/".to_string());

        assert_eq!(post.url(&config), "/2024/01/hello-world/");
    }

    #[test]
    fn test_permalink_priority() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let mut fm = make_frontmatter("Hello World", Some(date));
        fm.permalink = Some("/frontmatter-wins/".to_string());
        let post = make_post("blog", "hello-world", fm);

        let mut config = make_config();
        config
            .permalinks
            .sections
            .insert("blog".to_string(), "/:year/:slug/".to_string());

        // Frontmatter should win over config
        assert_eq!(post.url(&config), "/frontmatter-wins/");
    }

    #[test]
    fn test_permalink_cleans_double_slashes() {
        let fm = make_frontmatter("Hello World", None);
        let post = make_post("blog", "hello-world", fm);

        // Missing date should not leave double slashes
        assert_eq!(post.resolve_pattern("/:year/:slug/"), "/hello-world/");
    }

    #[test]
    fn test_permalink_ensures_slashes() {
        let fm = make_frontmatter("Hello World", None);
        let post = make_post("blog", "hello-world", fm);

        // Should add leading and trailing slashes
        assert_eq!(post.resolve_pattern(":section/:slug"), "/blog/hello-world/");
    }
}
