use std::path::Path;

use super::parser::{events_to_html, parse_markdown};
use super::transforms::AstTransform;
use crate::config::Config;

/// Context passed to transforms (fields available for custom transforms)
#[allow(dead_code)]
pub struct TransformContext<'a> {
    pub config: &'a Config,
    pub current_path: &'a Path,
    pub base_url: &'a str,
}

/// Markdown processing pipeline
pub struct Pipeline {
    transforms: Vec<Box<dyn AstTransform>>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            transforms: Vec::new(),
        }
    }

    /// Add a transform to the pipeline
    pub fn with<T: AstTransform + 'static>(mut self, transform: T) -> Self {
        self.transforms.push(Box::new(transform));
        self
    }

    /// Build the pipeline (sorts by priority)
    pub fn build(mut self) -> Self {
        self.transforms.sort_by_key(|t| t.priority());
        self
    }

    /// Process markdown content through the pipeline
    pub fn process(&self, content: &str, ctx: &TransformContext<'_>) -> String {
        // Strip frontmatter if present (delimited by ---)
        let content = if let Some(stripped) = content.strip_prefix("---") {
            if let Some(end) = stripped.find("---") {
                &stripped[end + 3..]
            } else {
                content
            }
        } else {
            content
        };

        let mut events = parse_markdown(content);

        // Apply each transform
        for transform in &self.transforms {
            events = transform.transform(events, ctx);
        }

        // Convert to HTML
        events_to_html(events.into_iter())
    }
}

impl Pipeline {
    /// Create pipeline with default transforms and configuration
    pub fn from_config(_config: &Config) -> Self {
        use super::transforms::*;

        Self::new()
            .with(LazyImagesTransform)
            .with(HeadingAnchorsTransform::new())
            .with(ExternalLinksTransform)
            .build()
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        use super::transforms::*;

        Self::new()
            .with(LazyImagesTransform)
            .with(HeadingAnchorsTransform::new())
            .with(ExternalLinksTransform)
            .build()
    }
}
