use anyhow::{Context, Result};
use log::{debug, info, trace};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::assets::copy_static_files;
use crate::config::{Config, PageDef};
use crate::lua::{BuildTracker, CachedDeps, SharedTracker};
use crate::markdown::{Pipeline, TransformContext};
use crate::templates::Templates;

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

        // Stage 4: Process assets
        trace!("Stage 4: Processing assets");
        self.process_assets()?;

        // Stage 5: Load templates
        trace!("Stage 5: Loading templates");
        let templates = Templates::new(&self.resolve_path(&self.config.paths.templates))?;

        // Stage 6: Render all pages in parallel
        trace!("Stage 6: Rendering {} pages", pages.len());
        let pipeline = Pipeline::from_config(&self.config);
        self.render_pages(&pages, &global_data, &templates, &pipeline)?;

        info!("Build complete: {} pages generated", pages.len());
        println!("Generated {} pages", pages.len());

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
    pub fn is_tracked_file(&self, path: &Path) -> bool {
        if let Some(ref cached) = self.cached_deps {
            // Canonicalize path for comparison
            let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            cached.reads.contains_key(&path)
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

    fn process_assets(&self) -> Result<()> {
        let static_dir = self.output_dir.join("static");
        let paths = &self.config.paths;

        // Copy static files (CSS and images are handled explicitly via Lua hooks)
        debug!(
            "Copying static files from {:?}",
            self.resolve_path(&paths.static_files)
        );
        copy_static_files(&self.resolve_path(&paths.static_files), &static_dir)?;

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
        let relevant_changes: Vec<PathBuf> = changes
            .content_files
            .iter()
            .filter(|p| {
                let full_path = self.project_dir.join(p);
                let is_tracked = self.is_tracked_file(&full_path);
                if !is_tracked {
                    trace!("Skipping untracked file: {:?}", p);
                }
                is_tracked
            })
            .map(|p| self.project_dir.join(p))
            .collect();

        // Content files changed - try incremental update
        if !relevant_changes.is_empty() {
            debug!(
                "{} tracked content files changed (out of {} total)",
                relevant_changes.len(),
                changes.content_files.len()
            );
            return self.rebuild_content_only(&relevant_changes);
        } else if !changes.content_files.is_empty() {
            debug!(
                "All {} changed files were untracked, skipping rebuild",
                changes.content_files.len()
            );
        }

        // Template-only changes - re-render with cached data (skip Lua calls)
        if changes.reload_templates {
            return self.rebuild_templates_only();
        }

        // Handle CSS-only changes
        if changes.rebuild_css {
            self.rebuild_css_only()?;
        }

        // Handle static/image changes
        self.process_static_changes(changes)?;

        Ok(())
    }

    /// Rebuild content - use incremental update if available, otherwise full data reload
    fn rebuild_content_only(&mut self, changed_paths: &[PathBuf]) -> Result<()> {
        debug!(
            "Content-only rebuild for {} changed files",
            changed_paths.len()
        );

        // Try incremental update if update_data function exists and we have cached data
        let global_data = if self.config.has_update_data() && self.cached_global_data.is_some() {
            debug!("Using incremental update_data()");
            let cached = self.cached_global_data.as_ref().unwrap();
            self.config.call_update_data(cached, changed_paths)?
        } else {
            debug!("Using full data() reload");
            self.config.call_data()?
        };

        let pages = self.config.call_pages(&global_data)?;

        // Update cache
        self.cached_global_data = Some(global_data.clone());
        self.cached_pages = Some(pages.clone());

        // Reload templates and re-render
        let templates = Templates::new(&self.resolve_path(&self.config.paths.templates))?;
        let pipeline = Pipeline::from_config(&self.config);
        self.render_pages(&pages, &global_data, &templates, &pipeline)?;

        // Merge thread-local tracking data and save
        self.tracker.merge_all_threads();
        self.save_cached_deps()?;

        println!("Re-rendered {} pages (content changed)", pages.len());
        Ok(())
    }

    /// Rebuild only by re-rendering templates with cached data
    fn rebuild_templates_only(&self) -> Result<()> {
        let (global_data, pages) = match (&self.cached_global_data, &self.cached_pages) {
            (Some(data), Some(pages)) => (data, pages),
            _ => {
                // No cache available, need full rebuild
                return Err(anyhow::anyhow!(
                    "No cached data available for template rebuild"
                ));
            }
        };

        debug!("Template-only rebuild with {} cached pages", pages.len());

        // Reload templates
        let templates = Templates::new(&self.resolve_path(&self.config.paths.templates))?;
        let pipeline = Pipeline::from_config(&self.config);

        // Re-render all pages with cached data
        self.render_pages(pages, global_data, &templates, &pipeline)?;

        println!("Re-rendered {} pages (templates only)", pages.len());
        Ok(())
    }

    /// Rebuild CSS by calling before_build hook (CSS is now handled via Lua)
    fn rebuild_css_only(&self) -> Result<()> {
        self.config.call_before_build()?;
        println!("Rebuilt assets (via before_build hook)");
        Ok(())
    }

    /// Process static file and image changes (just copies, optimization handled via Lua hooks)
    fn process_static_changes(&self, changes: &crate::watch::ChangeSet) -> Result<()> {
        use crate::assets::copy_single_static_file;

        let static_dir = self.output_dir.join("static");
        let source_static = self.resolve_path(&self.config.paths.static_files);

        // Process changed images (just copy, optimization handled via Lua hooks)
        for rel_path in &changes.image_files {
            let src = source_static.join(rel_path.as_path());
            let dest = static_dir.join(rel_path.as_path());

            if src.exists() {
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                copy_single_static_file(&src, &dest)?;
                println!("Copied image: {}", rel_path.display());
            }
        }

        // Process changed static files
        for rel_path in &changes.static_files {
            let src = source_static.join(rel_path.as_path());
            let dest = static_dir.join(rel_path.as_path());

            if src.exists() {
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                copy_single_static_file(&src, &dest)?;
                println!("Copied static file: {}", rel_path.display());
            }
        }

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
