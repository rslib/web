//! Tera template engine wrapper with page rendering

use anyhow::{Context, Result};
use log::{debug, trace};
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tera::Tera;

use crate::config::{Config, PageDef};
use crate::data::register_data_functions;
use crate::git::register_git_functions;
use crate::tracker::SharedTracker;

// Regex patterns for parsing template directives
static EXTENDS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\{%\s*extends\s+["']([^"']+)["']\s*%\}"#).unwrap());
static INCLUDE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\{%\s*include\s+["']([^"']+)["']\s*"#).unwrap());

/// Template dependency graph
#[derive(Debug, Clone, Default)]
pub struct TemplateDeps {
    /// Template name → file path
    pub template_files: HashMap<String, PathBuf>,
    /// Template name → templates it depends on (extends/includes)
    pub dependencies: HashMap<String, HashSet<String>>,
    /// Template name → templates that depend on it (reverse lookup)
    pub dependents: HashMap<String, HashSet<String>>,
}

impl TemplateDeps {
    /// Build template dependencies by scanning template files
    pub fn build(template_dir: &Path) -> Result<Self> {
        let mut deps = Self::default();

        // Find all template files
        let pattern = format!("{}/**/*", template_dir.to_string_lossy());
        for entry in glob::glob(&pattern)? {
            let path = entry?;
            if path.is_file() {
                // Get template name relative to template_dir
                if let Ok(rel_path) = path.strip_prefix(template_dir) {
                    let template_name = rel_path.to_string_lossy().to_string();
                    deps.template_files
                        .insert(template_name.clone(), path.clone());

                    // Parse template for dependencies
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let mut template_deps = HashSet::new();

                        // Find {% extends "..." %}
                        for cap in EXTENDS_RE.captures_iter(&content) {
                            if let Some(m) = cap.get(1) {
                                template_deps.insert(m.as_str().to_string());
                            }
                        }

                        // Find {% include "..." %}
                        for cap in INCLUDE_RE.captures_iter(&content) {
                            if let Some(m) = cap.get(1) {
                                template_deps.insert(m.as_str().to_string());
                            }
                        }

                        deps.dependencies.insert(template_name, template_deps);
                    }
                }
            }
        }

        // Build reverse lookup (dependents)
        for (template, template_deps) in &deps.dependencies {
            for dep in template_deps {
                deps.dependents
                    .entry(dep.clone())
                    .or_default()
                    .insert(template.clone());
            }
        }

        debug!(
            "Built template dependency graph: {} templates",
            deps.template_files.len()
        );
        trace!("Template dependencies: {:?}", deps.dependencies);

        Ok(deps)
    }

    /// Get all templates affected by a change to the given template (transitive)
    pub fn get_affected_templates(&self, changed_template: &str) -> HashSet<String> {
        let mut affected = HashSet::new();
        let mut to_process = vec![changed_template.to_string()];

        while let Some(template) = to_process.pop() {
            if affected.insert(template.clone()) {
                // Add all templates that depend on this one
                if let Some(dependents) = self.dependents.get(&template) {
                    for dep in dependents {
                        if !affected.contains(dep) {
                            to_process.push(dep.clone());
                        }
                    }
                }
            }
        }

        affected
    }

    /// Get the file path for a template name
    pub fn get_file_path(&self, template_name: &str) -> Option<&PathBuf> {
        self.template_files.get(template_name)
    }

    /// Find template name from file path
    pub fn find_template_by_path(&self, path: &Path) -> Option<&String> {
        self.template_files
            .iter()
            .find(|(_, p)| *p == path)
            .map(|(name, _)| name)
    }
}

/// Template engine wrapper
pub struct Templates {
    tera: Tera,
    deps: TemplateDeps,
}

impl Templates {
    pub fn new(template_dir: &Path, tracker: Option<SharedTracker>) -> Result<Self> {
        debug!("Loading templates from {:?}", template_dir);

        // Build template dependency graph first
        let deps = TemplateDeps::build(template_dir)?;

        // Record template files in tracker
        if let Some(ref tracker) = tracker {
            for path in deps.template_files.values() {
                if let Ok(content) = std::fs::read(path) {
                    tracker.record_read(path.clone(), &content);
                }
            }
        }

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

        Ok(Self { tera, deps })
    }

    /// Get template dependency graph
    pub fn deps(&self) -> &TemplateDeps {
        &self.deps
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

        // Get content and check if it needs Tera processing
        let raw_content = html_content
            .map(|s| s.to_string())
            .or_else(|| page.html.clone());

        // If content contains Tera syntax, pre-process it through Tera
        let processed_content = if let Some(ref content) = raw_content {
            if content.contains("{{") || content.contains("{%") {
                // Content has Tera syntax - render it with context
                trace!(
                    "Pre-processing content with Tera syntax for page '{}'",
                    page.path
                );
                match Tera::one_off(content, &context, false) {
                    Ok(rendered) => Some(rendered),
                    Err(e) => {
                        // Log warning but continue with original content
                        debug!("Failed to pre-process content Tera syntax: {}", e);
                        raw_content
                    }
                }
            } else {
                raw_content
            }
        } else {
            None
        };

        // Insert content directly for easy access
        if let Some(ref content) = processed_content {
            context.insert("content", content);
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
