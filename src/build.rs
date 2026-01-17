//! Build orchestrator for static site generation

use anyhow::{Context, Result};
use log::{debug, info, trace};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::{Config, PageDef};
use crate::markdown::{Pipeline, TransformContext};
use crate::templates::Templates;
use crate::tracker::{
    AssetRef, BuildTracker, CachedDeps, SharedTracker, extract_html_asset_refs,
    extract_markdown_asset_refs, resolve_url_to_source,
};

/// Cache file name
const CACHE_FILE: &str = ".rs-web-cache/deps.bin";

/// Main build orchestrator
pub struct Builder {
    config: Config,
    output_dir: PathBuf,
    project_dir: PathBuf,
    /// Build dependency tracker
    tracker: SharedTracker,
    /// Cached dependency info from previous build
    cached_deps: Option<CachedDeps>,
    /// Cached global data from last build
    cached_global_data: Option<serde_json::Value>,
    /// Cached page definitions from last build
    cached_pages: Option<Vec<PageDef>>,
}

impl Builder {
    pub fn new(config: Config, output_dir: PathBuf, project_dir: PathBuf) -> Self {
        // Load cached deps from previous build
        let cache_path = project_dir.join(CACHE_FILE);
        let cached_deps = CachedDeps::load(&cache_path);
        if cached_deps.is_some() {
            debug!("Loaded cached dependency info from {:?}", cache_path);
        }

        // Get the tracker from config (it was created during config loading)
        let tracker = config.tracker().clone();

        Self {
            config,
            output_dir,
            project_dir,
            tracker,
            cached_deps,
            cached_global_data: None,
            cached_pages: None,
        }
    }

    /// Create a new builder with a fresh tracker (for full rebuilds)
    pub fn new_with_tracker(project_dir: PathBuf, output_dir: PathBuf) -> Result<Self> {
        let tracker = Arc::new(BuildTracker::new());
        let config = Config::load_with_tracker(&project_dir, tracker.clone())?;

        // Load cached deps from previous build
        let cache_path = project_dir.join(CACHE_FILE);
        let cached_deps = CachedDeps::load(&cache_path);
        if cached_deps.is_some() {
            debug!("Loaded cached dependency info from {:?}", cache_path);
        }

        Ok(Self {
            config,
            output_dir,
            project_dir,
            tracker,
            cached_deps,
            cached_global_data: None,
            cached_pages: None,
        })
    }

    /// Resolve a path relative to the project directory
    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.project_dir.join(path)
        }
    }

    pub fn build(&mut self) -> Result<()> {
        info!("Starting build");
        debug!("Output directory: {:?}", self.output_dir);
        debug!("Project directory: {:?}", self.project_dir);

        // Stage 1: Clean output directory
        trace!("Stage 1: Cleaning output directory");
        self.clean()?;

        // Run before_build hook (after clean, so it can write to output_dir)
        trace!("Running before_build hook");
        self.config.call_before_build()?;

        // Stage 2: Call data() to get global data
        trace!("Stage 2: Calling data() function");
        let global_data = self.config.call_data()?;
        debug!("Global data loaded");

        // Stage 3: Call pages(global) to get page definitions
        trace!("Stage 3: Calling pages() function");
        let pages = self.config.call_pages(&global_data)?;
        debug!("Found {} pages to generate", pages.len());

        // Cache for incremental builds
        self.cached_global_data = Some(global_data.clone());
        self.cached_pages = Some(pages.clone());

        // Stage 4: Load templates
        trace!("Stage 5: Loading templates");
        let templates = Templates::new(
            &self.resolve_path(&self.config.paths.templates),
            Some(self.tracker.clone()),
        )?;

        // Stage 6: Render all pages in parallel
        trace!("Stage 6: Rendering {} pages", pages.len());
        let pipeline = Pipeline::from_config(&self.config);
        self.render_pages(&pages, &global_data, &templates, &pipeline)?;

        info!("Build complete: {} pages generated", pages.len());
        rs_print!("Generated {} pages", pages.len());

        // Run after_build hook
        trace!("Running after_build hook");
        self.config.call_after_build()?;

        // Merge all thread-local tracking data and save
        self.tracker.merge_all_threads();
        self.save_cached_deps()?;

        Ok(())
    }

    /// Save tracked dependencies to cache file
    fn save_cached_deps(&self) -> Result<()> {
        let cache_path = self.project_dir.join(CACHE_FILE);
        let deps = CachedDeps::from_tracker(&self.tracker);
        deps.save(&cache_path)
            .with_context(|| format!("Failed to save dependency cache to {:?}", cache_path))?;
        debug!(
            "Saved dependency cache: {} reads, {} writes",
            deps.reads.len(),
            deps.writes.len()
        );
        Ok(())
    }

    /// Get files that have changed since last build
    pub fn get_changed_files(&self) -> Vec<PathBuf> {
        match &self.cached_deps {
            Some(cached) => self.tracker.get_changed_files(cached),
            None => Vec::new(), // No cache means full rebuild needed
        }
    }

    /// Check if a full rebuild is needed (no cache or config changed)
    pub fn needs_full_rebuild(&self) -> bool {
        self.cached_deps.is_none()
    }

    /// Check if a file was tracked as a dependency in the last build
    /// This includes both explicit reads (via Lua API) and implicit refs (via HTML/markdown)
    pub fn is_tracked_file(&self, path: &Path) -> bool {
        if let Some(ref cached) = self.cached_deps {
            // Canonicalize path for comparison
            let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            // Check if it was read via Lua API (copy_file, read_file, etc.)
            if cached.reads.contains_key(&path) {
                return true;
            }
            // Check if it's referenced in any page's HTML/markdown
            if cached.asset_to_pages.contains_key(&path) {
                return true;
            }
            false
        } else {
            // No cache, assume all files are relevant
            true
        }
    }

    /// Check if any tracked files have changed since last build
    pub fn has_tracked_changes(&self) -> bool {
        if let Some(ref cached) = self.cached_deps {
            !self.tracker.get_changed_files(cached).is_empty()
        } else {
            true // No cache means we need to build
        }
    }

    fn clean(&self) -> Result<()> {
        if self.output_dir.exists() {
            debug!("Removing existing output directory: {:?}", self.output_dir);
            fs::remove_dir_all(&self.output_dir).with_context(|| {
                format!("Failed to clean output directory: {:?}", self.output_dir)
            })?;
        }
        trace!("Creating output directories");
        fs::create_dir_all(&self.output_dir)?;
        fs::create_dir_all(self.output_dir.join("static"))?;
        Ok(())
    }

    /// Remove pages that existed in the old build but not in the new one
    fn remove_stale_pages(&self, old_pages: &[PageDef], new_pages: &[PageDef]) -> Result<()> {
        use std::collections::HashSet;

        // Collect new page paths
        let new_paths: HashSet<&str> = new_pages.iter().map(|p| p.path.as_str()).collect();

        // Find and remove stale pages
        for old_page in old_pages {
            if !new_paths.contains(old_page.path.as_str()) {
                let relative_path = old_page.path.trim_matches('/');

                // Check if path has a file extension
                let has_extension = relative_path.contains('.')
                    && !relative_path.ends_with('/')
                    && relative_path
                        .rsplit('/')
                        .next()
                        .map(|s| s.contains('.'))
                        .unwrap_or(false);

                let file_path = if has_extension {
                    self.output_dir.join(relative_path)
                } else if relative_path.is_empty() {
                    self.output_dir.join("index.html")
                } else {
                    self.output_dir.join(relative_path).join("index.html")
                };

                if file_path.exists() {
                    rs_print!("  Removed: {}", old_page.path);
                    fs::remove_file(&file_path)?;

                    // Try to remove empty parent directory
                    if let Some(parent) = file_path.parent()
                        && parent != self.output_dir
                        && parent.read_dir()?.next().is_none()
                    {
                        let _ = fs::remove_dir(parent);
                    }
                }
            }
        }

        Ok(())
    }

    fn render_pages(
        &self,
        pages: &[PageDef],
        global_data: &serde_json::Value,
        templates: &Templates,
        pipeline: &Pipeline,
    ) -> Result<()> {
        // Render all pages in parallel
        pages
            .par_iter()
            .try_for_each(|page| self.render_single_page(page, global_data, templates, pipeline))?;

        Ok(())
    }

    fn render_single_page(
        &self,
        page: &PageDef,
        global_data: &serde_json::Value,
        templates: &Templates,
        pipeline: &Pipeline,
    ) -> Result<()> {
        trace!("Rendering page: {}", page.path);

        // Process content through markdown pipeline if provided
        let html_content = if let Some(ref markdown) = page.content {
            let ctx = TransformContext {
                config: &self.config,
                current_path: &self.project_dir,
                base_url: &self.config.site.base_url,
            };
            Some(pipeline.process(markdown, &ctx))
        } else {
            page.html.clone()
        };

        // If no template, output html directly (for raw text/xml files)
        let html = if page.template.is_none() {
            html_content.unwrap_or_default()
        } else {
            templates.render_page(&self.config, page, global_data, html_content.as_deref())?
        };

        // Extract asset references from the generated HTML and markdown content
        self.extract_and_record_asset_refs(page, &html);

        // Minify HTML if enabled (default: true)
        let html = if page.minify {
            minify_html(&html)
        } else {
            html
        };

        // Write output file
        let relative_path = page.path.trim_matches('/');

        // Check if path has a file extension (e.g., feed.xml, sitemap.json)
        let has_extension = relative_path.contains('.')
            && !relative_path.ends_with('/')
            && relative_path
                .rsplit('/')
                .next()
                .map(|s| s.contains('.'))
                .unwrap_or(false);

        if has_extension {
            // Write directly to file path (e.g., /feed.xml -> dist/feed.xml)
            let file_path = self.output_dir.join(relative_path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(file_path, html)?;
        } else {
            // Write to directory with index.html (e.g., /about/ -> dist/about/index.html)
            let page_dir = if relative_path.is_empty() {
                self.output_dir.clone()
            } else {
                self.output_dir.join(relative_path)
            };
            fs::create_dir_all(&page_dir)?;
            fs::write(page_dir.join("index.html"), html)?;
        }

        Ok(())
    }

    /// Extract asset references from HTML and markdown, record them in tracker
    fn extract_and_record_asset_refs(&self, page: &PageDef, html: &str) {
        // Extract from HTML
        let mut url_paths: Vec<String> = extract_html_asset_refs(html);

        // Also extract from markdown content if present
        if let Some(ref markdown) = page.content {
            let md_refs = extract_markdown_asset_refs(markdown);
            url_paths.extend(md_refs);
        }

        if url_paths.is_empty() {
            return;
        }

        // Get writes to resolve URL paths to source files
        let writes = self.tracker.get_writes();

        // Convert URL paths to AssetRefs with source resolution
        let asset_refs: Vec<AssetRef> = url_paths
            .into_iter()
            .map(|url_path| {
                let source_path =
                    resolve_url_to_source(&url_path, &self.output_dir, &writes, &self.project_dir);
                AssetRef {
                    url_path,
                    source_path,
                }
            })
            .collect();

        // Record in tracker
        let page_path = PathBuf::from(&page.path);
        self.tracker.record_html_refs(page_path, asset_refs);
    }

    /// Perform an incremental build based on what changed
    /// Uses tracker data to filter changes to only files that were actually used
    pub fn incremental_build(&mut self, changes: &crate::watch::ChangeSet) -> Result<()> {
        debug!("Starting incremental build");
        trace!("Change set: {:?}", changes);

        // Config changed - full rebuild needed (Lua functions may have changed)
        if changes.full_rebuild {
            return self.build();
        }

        // Filter content changes to only files that were tracked as dependencies
        let relevant_content: Vec<PathBuf> = changes
            .content_files
            .iter()
            .filter(|p| {
                let full_path = self.project_dir.join(p);
                let is_tracked = self.is_tracked_file(&full_path);
                if !is_tracked {
                    trace!("Skipping untracked content file: {:?}", p);
                }
                is_tracked
            })
            .map(|p| self.project_dir.join(p))
            .collect();

        // Filter asset changes to only files that were tracked as dependencies
        let relevant_assets: Vec<PathBuf> = changes
            .asset_files
            .iter()
            .filter(|p| {
                let full_path = self.project_dir.join(p);
                let is_tracked = self.is_tracked_file(&full_path);
                if !is_tracked {
                    trace!("Skipping untracked asset file: {:?}", p);
                }
                is_tracked
            })
            .map(|p| self.project_dir.join(p))
            .collect();

        // Log skipped files
        if !changes.content_files.is_empty() && relevant_content.is_empty() {
            debug!(
                "All {} content files were untracked, skipping",
                changes.content_files.len()
            );
        }
        if !changes.asset_files.is_empty() && relevant_assets.is_empty() {
            debug!(
                "All {} asset files were untracked, skipping",
                changes.asset_files.len()
            );
        }

        // Handle asset changes first (before_build runs copy_file, etc.)
        if !relevant_assets.is_empty() {
            debug!(
                "{} tracked assets changed (out of {} total)",
                relevant_assets.len(),
                changes.asset_files.len()
            );
            self.rebuild_assets_only(&relevant_assets)?;
        }

        // Handle CSS changes
        if changes.rebuild_css {
            self.rebuild_css_only()?;
        }

        // Content files changed - try incremental update
        if !relevant_content.is_empty() {
            debug!(
                "{} tracked content files changed (out of {} total)",
                relevant_content.len(),
                changes.content_files.len()
            );
            return self.rebuild_content_only(&relevant_content);
        }

        // Template changes - re-render affected pages with cached data (skip Lua calls)
        if changes.has_template_changes() {
            for path in &changes.template_files {
                rs_print!("  Changed: {}", path.display());
            }
            return self.rebuild_templates_only(&changes.template_files);
        }

        Ok(())
    }

    /// Rebuild content - use incremental update if available, otherwise full data reload
    fn rebuild_content_only(&mut self, changed_paths: &[PathBuf]) -> Result<()> {
        debug!(
            "Content-only rebuild for {} changed files",
            changed_paths.len()
        );

        // Print changed files
        for path in changed_paths {
            if let Ok(rel) = path.strip_prefix(&self.project_dir) {
                rs_print!("  Changed: {}", rel.display());
            } else {
                rs_print!("  Changed: {}", path.display());
            }
        }

        // Try incremental update if update_data function exists and we have cached data
        let global_data = if self.config.has_update_data() && self.cached_global_data.is_some() {
            debug!("Using incremental update_data()");
            let cached = self.cached_global_data.as_ref().unwrap();
            // Convert absolute paths to relative paths for Lua
            let relative_paths: Vec<PathBuf> = changed_paths
                .iter()
                .filter_map(|p| {
                    p.strip_prefix(&self.project_dir)
                        .ok()
                        .map(|r| r.to_path_buf())
                })
                .collect();
            self.config.call_update_data(cached, &relative_paths)?
        } else {
            debug!("Using full data() reload");
            self.config.call_data()?
        };

        let pages = self.config.call_pages(&global_data)?;

        // Remove stale pages that no longer exist in the new page list
        if let Some(ref old_pages) = self.cached_pages {
            self.remove_stale_pages(old_pages, &pages)?;
        }

        // Update cache
        self.cached_global_data = Some(global_data.clone());
        self.cached_pages = Some(pages.clone());

        // Reload templates and re-render
        let templates = Templates::new(
            &self.resolve_path(&self.config.paths.templates),
            Some(self.tracker.clone()),
        )?;
        let pipeline = Pipeline::from_config(&self.config);
        self.render_pages(&pages, &global_data, &templates, &pipeline)?;

        // Merge thread-local tracking data and save
        self.tracker.merge_all_threads();
        self.save_cached_deps()?;

        rs_print!("Re-rendered {} pages (content changed)", pages.len());
        Ok(())
    }

    /// Rebuild only by re-rendering templates with cached data
    fn rebuild_templates_only(
        &mut self,
        changed_template_files: &std::collections::HashSet<PathBuf>,
    ) -> Result<()> {
        let (global_data, all_pages) = match (&self.cached_global_data, &self.cached_pages) {
            (Some(data), Some(pages)) => (data.clone(), pages.clone()),
            _ => {
                // No cache available, do a full build to populate it
                log::info!("No cached data available, performing full build");
                return self.build();
            }
        };

        // Reload templates and get dependency graph
        let template_dir = self.resolve_path(&self.config.paths.templates);
        let templates = Templates::new(&template_dir, Some(self.tracker.clone()))?;
        let deps = templates.deps();

        // Find all affected templates (transitively)
        let mut affected_templates = std::collections::HashSet::new();
        for changed_path in changed_template_files {
            // Find template name from path
            if let Some(template_name) = deps.find_template_by_path(changed_path) {
                let transitive = deps.get_affected_templates(template_name);
                affected_templates.extend(transitive);
            } else if let Ok(rel_path) = changed_path.strip_prefix(&template_dir) {
                // Try relative path as template name
                let template_name = rel_path.to_string_lossy().to_string();
                let transitive = deps.get_affected_templates(&template_name);
                affected_templates.extend(transitive);
            }
        }

        debug!("Affected templates: {:?}", affected_templates);

        // Filter pages to only those using affected templates
        let pages_to_rebuild: Vec<_> = all_pages
            .iter()
            .filter(|page| {
                if let Some(ref template) = page.template {
                    affected_templates.contains(template)
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        if pages_to_rebuild.is_empty() {
            rs_print!("No pages affected by template changes");
            return Ok(());
        }

        debug!(
            "Template rebuild: {} of {} pages affected",
            pages_to_rebuild.len(),
            all_pages.len()
        );

        let pipeline = Pipeline::from_config(&self.config);

        // Re-render only affected pages with cached data
        self.render_pages(&pages_to_rebuild, &global_data, &templates, &pipeline)?;

        rs_print!(
            "Re-rendered {} of {} pages (templates changed)",
            pages_to_rebuild.len(),
            all_pages.len()
        );
        Ok(())
    }

    /// Rebuild CSS by calling before_build hook (CSS is now handled via Lua)
    fn rebuild_css_only(&self) -> Result<()> {
        rs_print!("  Changed: styles");
        self.config.call_before_build()?;
        rs_print!("Rebuilt CSS");
        Ok(())
    }

    /// Rebuild assets by calling before_build hook (re-copies static files)
    fn rebuild_assets_only(&self, changed_paths: &[PathBuf]) -> Result<()> {
        for path in changed_paths {
            if let Ok(rel) = path.strip_prefix(&self.project_dir) {
                rs_print!("  Changed: {}", rel.display());
            } else {
                rs_print!("  Changed: {}", path.display());
            }
        }
        self.config.call_before_build()?;
        rs_print!("Rebuilt {} assets", changed_paths.len());
        Ok(())
    }

    /// Reload config from disk
    pub fn reload_config(&mut self) -> Result<()> {
        debug!("Reloading config from {:?}", self.project_dir);
        self.config = crate::config::Config::load(&self.project_dir)?;
        // Clear cache since Lua functions might produce different output
        self.cached_global_data = None;
        self.cached_pages = None;
        info!("Config reloaded successfully");
        Ok(())
    }

    /// Get a reference to the current config
    pub fn config(&self) -> &Config {
        &self.config
    }
}

/// Minify HTML content with OXC-based inline JS minification
fn minify_html(html: &str) -> String {
    // First, minify inline JS with OXC (minify-js has bugs)
    let html = minify_inline_js(html);

    let cfg = minify_html::Cfg {
        minify_js: false,
        minify_css: true,
        ..Default::default()
    };
    let minified = minify_html::minify(html.as_bytes(), &cfg);
    String::from_utf8(minified).unwrap_or_else(|_| html.to_string())
}

/// Minify inline <script> tags using OXC
fn minify_inline_js(html: &str) -> String {
    use oxc_allocator::Allocator;
    use oxc_codegen::{Codegen, CodegenOptions};
    use oxc_minifier::{CompressOptions, MangleOptions, Minifier, MinifierOptions};
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    use regex::Regex;

    let re = Regex::new(r"(?s)(<script(?:\s[^>]*)?>)(.*?)(</script>)").unwrap();

    re.replace_all(html, |caps: &regex::Captures| {
        let open_tag = &caps[1];
        let content = &caps[2];
        let close_tag = &caps[3];

        // Skip external scripts (src=) or empty scripts
        if open_tag.contains("src=") || content.trim().is_empty() {
            return format!("{}{}{}", open_tag, content, close_tag);
        }

        // Try to minify with OXC
        let allocator = Allocator::default();
        let source_type = SourceType::mjs();
        let ret = Parser::new(&allocator, content, source_type).parse();

        if !ret.errors.is_empty() {
            // Parse error - return original
            return format!("{}{}{}", open_tag, content, close_tag);
        }

        let mut program = ret.program;
        let options = MinifierOptions {
            mangle: Some(MangleOptions::default()),
            compress: Some(CompressOptions::default()),
        };

        Minifier::new(options).minify(&allocator, &mut program);
        let minified = Codegen::new()
            .with_options(CodegenOptions::minify())
            .build(&program)
            .code;

        format!("{}{}{}", open_tag, minified, close_tag)
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minify_html_basic() {
        let input = "<html>  <body>   <p>Hello</p>  </body>  </html>";
        let result = minify_html(input);
        assert!(result.len() <= input.len());
        assert!(result.contains("Hello"));
    }

    #[test]
    fn test_minify_html_preserves_pre() {
        let input = "<pre>  code  with  spaces  </pre>";
        let result = minify_html(input);
        // Pre tags should preserve whitespace
        assert!(result.contains("code  with  spaces"));
    }

    #[test]
    fn test_minify_inline_js_basic() {
        let input = r#"<script>
            function hello() {
                console.log("hi");
            }
        </script>"#;
        let result = minify_inline_js(input);
        assert!(
            !result.contains('\n') || result.matches('\n').count() < input.matches('\n').count()
        );
        assert!(result.contains("<script>"));
        assert!(result.contains("</script>"));
    }

    #[test]
    fn test_minify_inline_js_skips_external() {
        let input = r#"<script src="/js/app.js"></script>"#;
        let result = minify_inline_js(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_minify_inline_js_skips_empty() {
        let input = "<script></script>";
        let result = minify_inline_js(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_minify_inline_js_multiple_scripts() {
        // Use console.log to prevent DCE
        let input = r#"<script>console.log(1);</script><script>console.log(2);</script>"#;
        let result = minify_inline_js(input);
        assert!(
            result.contains("console.log(1)") && result.contains("console.log(2)"),
            "Result: {}",
            result
        );
    }

    #[test]
    fn test_minify_inline_js_preserves_on_parse_error() {
        let input = "<script>function { broken</script>";
        let result = minify_inline_js(input);
        // Should preserve original on parse error
        assert!(result.contains("function { broken"));
    }

    #[test]
    fn test_minify_inline_js_with_attributes() {
        // Use console.log to prevent DCE
        let input = r#"<script type="text/javascript">console.log(1);</script>"#;
        let result = minify_inline_js(input);
        assert!(result.contains(r#"type="text/javascript""#));
    }

    #[test]
    fn test_minify_html_with_inline_js() {
        // Use console.log to prevent DCE
        let input = r#"<html><head><script>console.log(true);</script></head></html>"#;
        let result = minify_html(input);
        // Should minify JS (true -> !0)
        assert!(
            result.contains("!0") || result.contains("true"),
            "Result: {}",
            result
        );
    }

    #[test]
    fn test_minify_html_css_minification() {
        let input = r#"<style>  body  {  color:  red;  }  </style>"#;
        let result = minify_html(input);
        assert!(result.len() < input.len());
    }
}
