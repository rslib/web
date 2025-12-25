use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use tera::Tera;

use crate::config::Config;
use crate::content::{Content, Page, Post};
use crate::links::{GraphData, LinkGraph};

/// Template engine wrapper
pub struct Templates {
    tera: Tera,
}

impl Templates {
    pub fn new(template_dir: &Path) -> Result<Self> {
        let template_dir_str = template_dir.to_string_lossy();
        let pattern = format!("{}/**/*.html", template_dir_str);
        let tera = Tera::new(&pattern)
            .with_context(|| format!("Failed to load templates from {}", template_dir_str))?;

        Ok(Self { tera })
    }

    /// Render inline HTML content through Tera with post context
    /// Used for .html content files that contain Tera template syntax
    pub fn render_html_content(&self, config: &Config, post: &Post) -> Result<String> {
        let mut tera = self.tera.clone();

        // Register the content as a one-off template
        let template_name = format!("__inline__{}", post.slug());
        tera.add_raw_template(&template_name, &post.content)
            .with_context(|| {
                format!(
                    "Failed to parse HTML content as Tera template: {}",
                    post.slug()
                )
            })?;

        // Build context similar to render_post
        let mut context = tera::Context::new();
        context.insert("site", &SiteContext::from(config));

        let post_ctx = PostContext::from_post(config, post);
        context.insert("post", &post_ctx);
        context.insert(
            "page",
            &PageContext {
                title: post_ctx.title.clone(),
                description: post_ctx.description.clone(),
                url: post_ctx.full_url.clone(),
                image: post_ctx.image.clone(),
            },
        );

        tera.render(&template_name, &context)
            .with_context(|| format!("Failed to render HTML content: {}", post.slug()))
    }

    /// Resolve template name with priority:
    /// 1. Frontmatter template (highest)
    /// 2. Config section mapping
    /// 3. Convention: {section}.html
    /// 4. Default: post.html
    fn resolve_template(&self, post: &Post, config: &Config) -> String {
        // 1. Check frontmatter
        if let Some(template) = &post.frontmatter.template {
            return template.clone();
        }

        // 2. Check config mapping
        if let Some(template) = config.templates.sections.get(&post.section) {
            return template.clone();
        }

        // 3. Convention: try {section}.html
        let section_template = format!("{}.html", post.section);
        if self
            .tera
            .get_template_names()
            .any(|n| n == section_template)
        {
            return section_template;
        }

        // 4. Default fallback
        "post.html".to_string()
    }

    /// Render the home page
    pub fn render_home(&self, config: &Config, page: &Page, content: &Content) -> Result<String> {
        let mut context = tera::Context::new();

        // Site info
        context.insert("site", &SiteContext::from(config));

        // Page info
        context.insert("page", &PageContext::from_page(config, page));

        // All sections with their posts
        let sections_ctx: HashMap<String, SectionContext> = content
            .sections
            .iter()
            .map(|(name, section)| {
                (
                    name.clone(),
                    SectionContext {
                        name: section.name.clone(),
                        posts: section
                            .posts
                            .iter()
                            .map(|p| PostContext::from_post(config, p))
                            .collect(),
                    },
                )
            })
            .collect();
        context.insert("sections", &sections_ctx);

        // Page content
        context.insert("content", &page.html);

        self.tera
            .render("home.html", &context)
            .with_context(|| "Failed to render home page")
    }

    /// Render a blog post
    pub fn render_post(
        &self,
        config: &Config,
        post: &Post,
        link_graph: &LinkGraph,
    ) -> Result<String> {
        let mut context = tera::Context::new();

        // Site info
        context.insert("site", &SiteContext::from(config));

        // Post info
        let post_ctx = PostContext::from_post(config, post);
        context.insert("post", &post_ctx);

        // Also add as page for head.html compatibility
        context.insert(
            "page",
            &PageContext {
                title: post_ctx.title.clone(),
                description: post_ctx.description.clone(),
                url: post_ctx.full_url.clone(),
                image: post_ctx.image.clone(),
            },
        );

        // Backlinks - posts that link to this post
        let backlinks: Vec<BacklinkContext> = link_graph
            .backlinks_for(&post.url(config))
            .iter()
            .map(|bl| BacklinkContext {
                url: bl.url.clone(),
                title: bl.title.clone(),
                section: bl.section.clone(),
            })
            .collect();
        context.insert("backlinks", &backlinks);

        // Local graph - this post and its connections
        let local_graph = link_graph.local_graph_for(&post.url(config));
        context.insert("graph", &local_graph);

        // Post content
        context.insert("content", &post.html);

        let template = self.resolve_template(post, config);
        self.tera.render(&template, &context).with_context(|| {
            format!(
                "Failed to render post: {} with template: {}",
                post.slug(),
                template
            )
        })
    }

    /// Render the graph page
    pub fn render_graph(&self, config: &Config, graph_data: &GraphData) -> Result<String> {
        let mut context = tera::Context::new();
        context.insert("site", &SiteContext::from(config));
        context.insert(
            "page",
            &PageContext {
                title: "Graph".to_string(),
                description: "Knowledge graph".to_string(),
                url: format!("{}/{}/", config.site.base_url, config.graph.path),
                image: None,
            },
        );
        context.insert("graph", graph_data);

        self.tera
            .render(&config.graph.template, &context)
            .with_context(|| {
                format!(
                    "Failed to render graph page with template: {}",
                    config.graph.template
                )
            })
    }
}

#[derive(Serialize)]
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

#[derive(Serialize)]
struct PageContext {
    title: String,
    description: String,
    url: String,
    image: Option<String>,
}

#[derive(Serialize)]
struct SectionContext {
    name: String,
    posts: Vec<PostContext>,
}

#[derive(Serialize)]
struct BacklinkContext {
    url: String,
    title: String,
    section: String,
}

impl PageContext {
    fn from_page(config: &Config, page: &Page) -> Self {
        Self {
            title: page.frontmatter.title.clone(),
            description: page
                .frontmatter
                .description
                .clone()
                .unwrap_or_else(|| config.site.description.clone()),
            url: config.site.base_url.clone(),
            image: page
                .frontmatter
                .image
                .clone()
                .or_else(|| config.seo.default_og_image.clone()),
        }
    }
}

#[derive(Serialize)]
struct PostContext {
    title: String,
    description: String,
    url: String,
    full_url: String,
    slug: String,
    section: String,
    date: Option<String>,
    date_iso: Option<String>,
    tags: Vec<String>,
    image: Option<String>,
    reading_time: u32,
    word_count: usize,
    /// Whether this post is fully encrypted
    encrypted: bool,
    /// Whether this post has :::encrypted blocks (partial encryption)
    has_encrypted_blocks: bool,
    /// Base64-encoded ciphertext (only if encrypted)
    ciphertext: Option<String>,
    /// Base64-encoded salt (only if encrypted)
    salt: Option<String>,
    /// Base64-encoded nonce (only if encrypted)
    nonce: Option<String>,
}

impl PostContext {
    fn from_post(config: &Config, post: &Post) -> Self {
        let url = post.url(config);

        // Extract encrypted content if present
        let (encrypted, ciphertext, salt, nonce) = if let Some(ref enc) = post.encrypted_content {
            (
                true,
                Some(enc.ciphertext.clone()),
                Some(enc.salt.clone()),
                Some(enc.nonce.clone()),
            )
        } else {
            (post.frontmatter.encrypted, None, None, None)
        };

        Self {
            title: post.frontmatter.title.clone(),
            description: post.frontmatter.description.clone().unwrap_or_default(),
            full_url: format!("{}{}", config.site.base_url, &url),
            url,
            slug: post.slug().to_string(),
            section: post.section.clone(),
            date: post
                .frontmatter
                .date
                .map(|d| d.format("%B %d, %Y").to_string()),
            date_iso: post
                .frontmatter
                .date
                .map(|d| d.format("%Y-%m-%d").to_string()),
            tags: post.frontmatter.tags.clone().unwrap_or_default(),
            image: post
                .frontmatter
                .image
                .clone()
                .or_else(|| config.seo.default_og_image.clone()),
            reading_time: post.reading_time,
            word_count: post.word_count,
            encrypted,
            has_encrypted_blocks: post.has_encrypted_blocks,
            ciphertext,
            salt,
            nonce,
        }
    }
}
