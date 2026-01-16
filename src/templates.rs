use anyhow::{Context, Result};
use log::{debug, trace};
use serde::Serialize;
use std::path::Path;
use tera::Tera;

use crate::config::{Config, PageDef};
use crate::data::register_data_functions;
use crate::git::register_git_functions;
use crate::lua::SharedTracker;

/// Template engine wrapper
pub struct Templates {
    tera: Tera,
}

impl Templates {
    pub fn new(template_dir: &Path, tracker: Option<SharedTracker>) -> Result<Self> {
        debug!("Loading templates from {:?}", template_dir);
        let template_dir_str = template_dir.to_string_lossy();
        let pattern = format!("{}/**/*", template_dir_str);
        let mut tera = Tera::new(&pattern)
            .with_context(|| format!("Failed to load templates from {}", template_dir_str))?;

        register_git_functions(&mut tera);
        register_data_functions(&mut tera, tracker);

        let template_count = tera.get_template_names().count();
        debug!("Loaded {} templates", template_count);
        trace!(
            "Available templates: {:?}",
            tera.get_template_names().collect::<Vec<_>>()
        );

        Ok(Self { tera })
    }

    /// Render a page
    ///
    /// Template context:
    /// - ctx.site: Site configuration (title, description, base_url, author)
    /// - ctx.global: Global data from data() function
    /// - ctx.page: Page-specific data (title, description, url, content, data)
    pub fn render_page(
        &self,
        config: &Config,
        page: &PageDef,
        global_data: &serde_json::Value,
        html_content: Option<&str>,
    ) -> Result<String> {
        let mut context = tera::Context::new();

        // Build the ctx root context
        let ctx = CtxRoot {
            site: SiteContext::from(config),
            global: global_data.clone(),
            page: PageContext::from_page_def(config, page, html_content),
        };
        context.insert("ctx", &ctx);
        context.insert("site", &ctx.site);
        context.insert("global", &ctx.global);
        context.insert("page", &ctx.page);

        // Insert content directly for easy access
        if let Some(content) = html_content {
            context.insert("content", content);
        } else if let Some(ref html) = page.html {
            context.insert("content", html);
        }

        let template = page
            .template
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Page '{}' has no template", page.path))?;

        self.tera.render(template, &context).with_context(|| {
            format!(
                "Failed to render page '{}' with template '{}'",
                page.path, template
            )
        })
    }
}

/// Root context wrapper (ctx.*)
#[derive(Serialize)]
struct CtxRoot {
    site: SiteContext,
    global: serde_json::Value,
    page: PageContext,
}

/// Site configuration context (ctx.site.*)
#[derive(Serialize, Clone)]
struct SiteContext {
    title: String,
    description: String,
    base_url: String,
    author: String,
    twitter_handle: Option<String>,
    default_og_image: Option<String>,
}

impl From<&Config> for SiteContext {
    fn from(config: &Config) -> Self {
        Self {
            title: config.site.title.clone(),
            description: config.site.description.clone(),
            base_url: config.site.base_url.clone(),
            author: config.site.author.clone(),
            twitter_handle: config.seo.twitter_handle.clone(),
            default_og_image: config.seo.default_og_image.clone(),
        }
    }
}

/// Page context (ctx.page.*)
#[derive(Serialize)]
struct PageContext {
    /// Page title
    title: String,
    /// Meta description
    description: String,
    /// Full URL (base_url + path)
    url: String,
    /// Relative path (e.g., "/blog/hello/")
    path: String,
    /// OG image URL
    image: Option<String>,
    /// Rendered HTML content
    content: Option<String>,
    /// Page-specific data from Lua
    data: Option<serde_json::Value>,
}

impl PageContext {
    fn from_page_def(config: &Config, page: &PageDef, html_content: Option<&str>) -> Self {
        Self {
            title: page.title.clone().unwrap_or_default(),
            description: page
                .description
                .clone()
                .unwrap_or_else(|| config.site.description.clone()),
            url: format!(
                "{}{}",
                config.site.base_url.trim_end_matches('/'),
                page.path
            ),
            path: page.path.clone(),
            image: page
                .image
                .clone()
                .or_else(|| config.seo.default_og_image.clone()),
            content: html_content
                .map(|s| s.to_string())
                .or_else(|| page.html.clone()),
            data: page.data.clone(),
        }
    }
}
