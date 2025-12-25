use anyhow::{Context, Result};
use std::path::Path;

use super::frontmatter::{Frontmatter, parse_frontmatter};

#[derive(Debug, Clone)]
pub struct Page {
    pub frontmatter: Frontmatter,
    pub content: String,
    pub html: String,
}

impl Page {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let raw_content = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read page: {:?}", path.as_ref()))?;

        let (frontmatter, content) = parse_frontmatter(&raw_content)?;

        Ok(Self {
            frontmatter,
            content: content.to_string(),
            html: String::new(), // Will be filled by markdown pipeline
        })
    }

    pub fn with_html(mut self, html: String) -> Self {
        self.html = html;
        self
    }
}
