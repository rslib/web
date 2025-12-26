use anyhow::{Context, Result};
use log::{debug, info, trace};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

use crate::assets::{
    ImageConfig, build_css, copy_single_static_file, copy_static_files, optimize_images,
    optimize_single_image,
};
use crate::config::Config;
use crate::content::{Content, ContentType, Post, discover_content};
use crate::encryption::{encrypt_content, resolve_password};
use crate::links::LinkGraph;
use crate::markdown::{
    Pipeline, TransformContext, extract_encrypted_blocks, extract_html_encrypted_blocks,
    replace_placeholders,
};
use crate::rss::generate_rss;
use crate::templates::Templates;
use crate::text::{format_home_text, format_post_text};
use crate::watch::ChangeSet;

/// Main build orchestrator
pub struct Builder {
    config: Config,
    output_dir: PathBuf,
    project_dir: PathBuf,
}

impl Builder {
    pub fn new(config: Config, output_dir: PathBuf, project_dir: PathBuf) -> Self {
        Self {
            config,
            output_dir,
            project_dir,
        }
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

        // Stage 2: Discover and load content
        trace!("Stage 2: Discovering content");
        let content = self.load_content()?;
        debug!(
            "Found {} sections with {} total posts",
            content.sections.len(),
            content
                .sections
                .values()
                .map(|s| s.posts.len())
                .sum::<usize>()
        );

        // Stage 3: Process assets
        trace!("Stage 3: Processing assets");
        self.process_assets()?;

        // Stage 4: Load templates (needed for HTML content processing)
        trace!("Stage 4: Loading templates");
        let templates = Templates::new(&self.resolve_path(&self.config.paths.templates))?;

        // Stage 5: Process content through pipeline (markdown) or Tera (HTML)
        trace!("Stage 5: Processing content through pipeline");
        let pipeline = Pipeline::from_config(&self.config);
        let content = self.process_content(content, &pipeline, &templates)?;

        // Stage 6: Render and write HTML
        trace!("Stage 6: Rendering HTML");
        self.render_html(&content, &templates)?;

        // Stage 7: Render text output (if enabled)
        if self.config.text.enabled {
            trace!("Stage 7: Rendering text output");
            self.render_text(&content)?;
        }

        let total_posts: usize = content.sections.values().map(|s| s.posts.len()).sum();
        info!(
            "Build complete: {} posts in {} sections",
            total_posts,
            content.sections.len()
        );
        println!(
            "Generated {} posts in {} sections",
            total_posts,
            content.sections.len()
        );

        Ok(())
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

    fn load_content(&self) -> Result<Content> {
        discover_content(&self.config.paths, Some(&self.project_dir))
    }

    fn process_assets(&self) -> Result<()> {
        let static_dir = self.output_dir.join("static");
        let paths = &self.config.paths;

        // Build CSS
        debug!("Building CSS from {:?}", self.resolve_path(&paths.styles));
        build_css(
            &self.resolve_path(&paths.styles),
            &static_dir.join(&self.config.build.css_output),
            self.config.build.minify_css,
        )?;

        // Optimize images
        debug!(
            "Optimizing images (quality: {}, scale: {})",
            self.config.images.quality, self.config.images.scale_factor
        );
        let image_config = ImageConfig {
            quality: self.config.images.quality,
            scale_factor: self.config.images.scale_factor,
        };
        optimize_images(
            &self.resolve_path(&paths.static_files),
            &static_dir,
            &image_config,
        )?;

        // Copy other static files
        debug!(
            "Copying static files from {:?}",
            self.resolve_path(&paths.static_files)
        );
        copy_static_files(&self.resolve_path(&paths.static_files), &static_dir)?;

        Ok(())
    }

    fn process_content(
        &self,
        mut content: Content,
        pipeline: &Pipeline,
        templates: &Templates,
    ) -> Result<Content> {
        let paths = &self.config.paths;

        // Process home page
        if let Some(page) = content.home.take() {
            let home_path = self.resolve_path(&paths.content).join(&paths.home);
            let ctx = TransformContext {
                config: &self.config,
                current_path: &home_path,
                base_url: &self.config.site.base_url,
            };
            let html = pipeline.process(&page.content, &ctx);
            content.home = Some(page.with_html(html));
        }

        // Process all posts
        content
            .sections
            .par_iter_mut()
            .try_for_each(|(_, section)| {
                let section_name = &section.name;
                section.posts.par_iter_mut().try_for_each(|post| {
                    self.process_single_post(post, section_name, pipeline, paths, templates)
                })
            })?;

        Ok(content)
    }

    /// Process a single post through the markdown pipeline (or Tera for HTML) and encryption
    fn process_single_post(
        &self,
        post: &mut crate::content::Post,
        section_name: &str,
        pipeline: &Pipeline,
        paths: &crate::config::PathsConfig,
        templates: &Templates,
    ) -> Result<()> {
        trace!(
            "Processing post: {} ({})",
            post.frontmatter.title, section_name
        );

        // Handle HTML content files - process through Tera
        if post.content_type == ContentType::Html {
            trace!("Post is HTML content, processing through Tera");
            return self.process_html_post(post, templates);
        }

        // Markdown processing
        let path = self
            .resolve_path(&paths.content)
            .join(section_name)
            .join(format!("{}.md", post.file_slug));
        let ctx = TransformContext {
            config: &self.config,
            current_path: &path,
            base_url: &self.config.site.base_url,
        };

        // Check if post should be fully encrypted
        if post.frontmatter.encrypted {
            debug!("Encrypting post: {}", post.frontmatter.title);
            let html = pipeline.process(&post.content, &ctx);
            let password = resolve_password(
                &self.config.encryption,
                post.frontmatter.password.as_deref(),
            )
            .with_context(|| {
                format!(
                    "Failed to resolve password for encrypted post: {}",
                    post.frontmatter.title
                )
            })?;

            let encrypted = encrypt_content(&html, &password)
                .with_context(|| format!("Failed to encrypt post: {}", post.frontmatter.title))?;

            post.encrypted_content = Some(encrypted);
            post.html = String::new();
        } else {
            // Check for partial encryption (:::encrypted blocks)
            let preprocess_result = extract_encrypted_blocks(&post.content);

            if preprocess_result.blocks.is_empty() {
                // No encrypted blocks, process normally
                post.html = pipeline.process(&post.content, &ctx);
            } else {
                debug!(
                    "Found {} encrypted blocks in post: {}",
                    preprocess_result.blocks.len(),
                    post.frontmatter.title
                );
                // Process main content with placeholders
                let main_html = pipeline.process(&preprocess_result.markdown, &ctx);

                // Process and encrypt each block
                let encrypted_blocks: Result<Vec<_>> = preprocess_result
                    .blocks
                    .par_iter()
                    .map(|block| {
                        // Use block-specific password if provided, otherwise fall back to global
                        let block_password = if let Some(ref pw) = block.password {
                            pw.clone()
                        } else {
                            resolve_password(
                                &self.config.encryption,
                                post.frontmatter.password.as_deref(),
                            )
                            .with_context(|| {
                                format!(
                                    "Failed to resolve password for block {} in post: {}",
                                    block.id, post.frontmatter.title
                                )
                            })?
                        };

                        // Render block content through pipeline
                        let block_html = pipeline.process(&block.content, &ctx);

                        // Encrypt the rendered HTML
                        let encrypted = encrypt_content(&block_html, &block_password)
                            .with_context(|| {
                                format!(
                                    "Failed to encrypt block {} in post: {}",
                                    block.id, post.frontmatter.title
                                )
                            })?;

                        Ok((
                            block.id,
                            encrypted.ciphertext,
                            encrypted.salt,
                            encrypted.nonce,
                            block.password.is_some(),
                        ))
                    })
                    .collect();

                // Replace placeholders with encrypted HTML
                post.html = replace_placeholders(&main_html, &encrypted_blocks?, post.slug());
                post.has_encrypted_blocks = true;
            }
        }

        Ok(())
    }

    /// Process an HTML content file through Tera templating with encryption support
    fn process_html_post(&self, post: &mut Post, templates: &Templates) -> Result<()> {
        // Render content through Tera first
        let rendered_html = templates.render_html_content(&self.config, post)?;

        // Check if post should be fully encrypted
        if post.frontmatter.encrypted {
            let password = resolve_password(
                &self.config.encryption,
                post.frontmatter.password.as_deref(),
            )
            .with_context(|| {
                format!(
                    "Failed to resolve password for encrypted HTML post: {}",
                    post.frontmatter.title
                )
            })?;

            let encrypted = encrypt_content(&rendered_html, &password).with_context(|| {
                format!("Failed to encrypt HTML post: {}", post.frontmatter.title)
            })?;

            post.encrypted_content = Some(encrypted);
            post.html = String::new();
        } else {
            // Check for partial encryption (<encrypted> blocks)
            let preprocess_result = extract_html_encrypted_blocks(&rendered_html);

            if preprocess_result.blocks.is_empty() {
                // No encrypted blocks, use rendered HTML as-is
                post.html = rendered_html;
            } else {
                // Process and encrypt each block
                let encrypted_blocks: Result<Vec<_>> = preprocess_result
                    .blocks
                    .iter()
                    .map(|block| {
                        // Use block-specific password if provided, otherwise fall back to global
                        let block_password = if let Some(ref pw) = block.password {
                            pw.clone()
                        } else {
                            resolve_password(
                                &self.config.encryption,
                                post.frontmatter.password.as_deref(),
                            )
                            .with_context(|| {
                                format!(
                                    "Failed to resolve password for block {} in HTML post: {}",
                                    block.id, post.frontmatter.title
                                )
                            })?
                        };

                        // Encrypt the block content (already rendered through Tera)
                        let encrypted = encrypt_content(&block.content, &block_password)
                            .with_context(|| {
                                format!(
                                    "Failed to encrypt block {} in HTML post: {}",
                                    block.id, post.frontmatter.title
                                )
                            })?;

                        Ok((
                            block.id,
                            encrypted.ciphertext,
                            encrypted.salt,
                            encrypted.nonce,
                            block.password.is_some(),
                        ))
                    })
                    .collect();

                // Replace placeholders with encrypted HTML
                post.html = replace_placeholders(
                    &preprocess_result.markdown,
                    &encrypted_blocks?,
                    post.slug(),
                );
                post.has_encrypted_blocks = true;
            }
        }

        Ok(())
    }

    fn render_html(&self, content: &Content, templates: &Templates) -> Result<()> {
        // Build link graph for backlinks
        debug!("Building link graph for backlinks");
        let link_graph = LinkGraph::build(&self.config, content);
        trace!("Link graph built");

        // Generate graph if enabled
        if self.config.graph.enabled {
            debug!("Generating graph visualization");
            let graph_data = link_graph.to_graph_data();

            // Write graph.json for visualization
            let graph_json = serde_json::to_string(&graph_data)?;
            fs::write(self.output_dir.join("graph.json"), graph_json)?;

            // Render graph page
            let graph_dir = self.output_dir.join(&self.config.graph.path);
            fs::create_dir_all(&graph_dir)?;
            let graph_html = templates.render_graph(&self.config, &graph_data)?;
            fs::write(graph_dir.join("index.html"), graph_html)?;
        }

        // Render home page
        if let Some(home_page) = &content.home {
            let html = templates.render_home(&self.config, home_page, content)?;
            fs::write(self.output_dir.join("index.html"), html)?;
        }

        // Render posts for each section
        content.sections.par_iter().try_for_each(|(_, section)| {
            section.posts.par_iter().try_for_each(|post| {
                // Use resolved URL to determine output path
                let url = post.url(&self.config);
                // Convert URL to file path: /blog/2024/01/hello/ -> blog/2024/01/hello
                let relative_path = url.trim_matches('/');
                let post_dir = self.output_dir.join(relative_path);
                fs::create_dir_all(&post_dir)?;
                let html = templates.render_post(&self.config, post, &link_graph)?;
                fs::write(post_dir.join("index.html"), html)?;
                Ok::<_, anyhow::Error>(())
            })
        })?;

        // Generate RSS feed
        if self.config.rss.enabled {
            debug!("Generating RSS feed");
            self.generate_rss(content)?;
        }

        Ok(())
    }

    fn generate_rss(&self, content: &Content) -> Result<()> {
        trace!("Building RSS feed");
        let rss_config = &self.config.rss;

        // Collect posts from specified sections (or all if empty)
        let mut posts: Vec<&Post> = content
            .sections
            .iter()
            .filter(|(name, _)| {
                rss_config.sections.is_empty() || rss_config.sections.contains(name)
            })
            .flat_map(|(_, section)| section.posts.iter())
            .filter(|post| !post.frontmatter.encrypted) // Exclude fully encrypted posts
            .filter(|post| {
                // Optionally exclude posts with encrypted blocks
                !rss_config.exclude_encrypted_blocks || !post.has_encrypted_blocks
            })
            .collect();

        // Sort by date (newest first)
        posts.sort_by(|a, b| b.frontmatter.date.cmp(&a.frontmatter.date));

        // Limit number of items
        posts.truncate(rss_config.limit);

        let rss_xml = generate_rss(&self.config, &posts);
        fs::write(self.output_dir.join(&rss_config.filename), rss_xml)?;

        Ok(())
    }

    /// Generate plain text versions of posts for curl-friendly access
    fn render_text(&self, content: &Content) -> Result<()> {
        let text_config = &self.config.text;
        let base_url = &self.config.site.base_url;

        // Render home page text if enabled
        if text_config.include_home
            && let Some(home_page) = &content.home
        {
            let text = format_home_text(
                &self.config.site.title,
                &self.config.site.description,
                &home_page.html,
                base_url,
            );
            fs::write(self.output_dir.join("index.txt"), text)?;
        }

        // Render posts for each section in parallel
        content
            .sections
            .par_iter()
            .try_for_each(|(section_name, section)| {
                // Check if this section should be included
                if !text_config.sections.is_empty() && !text_config.sections.contains(section_name)
                {
                    return Ok::<_, anyhow::Error>(());
                }

                section.posts.par_iter().try_for_each(|post| {
                    // Skip encrypted posts if configured
                    if text_config.exclude_encrypted
                        && (post.frontmatter.encrypted || post.has_encrypted_blocks)
                    {
                        return Ok::<_, anyhow::Error>(());
                    }

                    let url = post.url(&self.config);
                    let relative_path = url.trim_matches('/');
                    let post_dir = self.output_dir.join(relative_path);

                    // Format date for display
                    let date_str = post
                        .frontmatter
                        .date
                        .map(|d| d.format("%Y-%m-%d").to_string());

                    let tags = post.frontmatter.tags.as_deref().unwrap_or(&[]);

                    // For fully encrypted posts, use placeholder content
                    let content = if post.frontmatter.encrypted {
                        "[This post is encrypted - visit web version to decrypt]"
                    } else {
                        &post.html
                    };

                    let text = format_post_text(
                        &post.frontmatter.title,
                        date_str.as_deref(),
                        post.frontmatter.description.as_deref(),
                        tags,
                        post.reading_time,
                        content,
                        &url,
                        base_url,
                    );

                    fs::write(post_dir.join("index.txt"), text)?;
                    Ok::<_, anyhow::Error>(())
                })
            })?;

        // Count text files generated
        let text_count: usize = content
            .sections
            .iter()
            .filter(|(name, _)| {
                text_config.sections.is_empty() || text_config.sections.contains(name)
            })
            .flat_map(|(_, section)| section.posts.iter())
            .filter(|post| {
                !text_config.exclude_encrypted
                    || (!post.frontmatter.encrypted && !post.has_encrypted_blocks)
            })
            .count();

        println!("Generated {} text files", text_count);

        Ok(())
    }

    /// Perform an incremental build based on what changed
    pub fn incremental_build(&mut self, changes: &ChangeSet) -> Result<()> {
        debug!("Starting incremental build");
        trace!("Change set: {:?}", changes);

        // If full rebuild is needed, just do a regular build
        if changes.full_rebuild {
            info!("Full rebuild required");
            return self.build();
        }

        // Handle CSS-only changes (fastest path)
        if changes.rebuild_css
            && !changes.reload_templates
            && !changes.rebuild_home
            && changes.content_files.is_empty()
        {
            self.rebuild_css_only()?;

            // Also handle any static/image changes
            self.process_static_changes(changes)?;
            return Ok(());
        }

        // Handle static file changes without content rebuild
        if !changes.reload_templates
            && !changes.rebuild_home
            && changes.content_files.is_empty()
            && !changes.rebuild_css
        {
            self.process_static_changes(changes)?;
            return Ok(());
        }

        // For template or content changes, we need to rebuild content
        let content = self.load_content()?;
        let templates = Templates::new(&self.resolve_path(&self.config.paths.templates))?;
        let pipeline = Pipeline::from_config(&self.config);

        // Process all content (could be optimized further for single-file changes)
        let content = self.process_content(content, &pipeline, &templates)?;

        // Render HTML
        self.render_html(&content, &templates)?;

        // Render text if enabled
        if self.config.text.enabled {
            self.render_text(&content)?;
        }

        // Handle any CSS changes
        if changes.rebuild_css {
            self.rebuild_css_only()?;
        }

        // Handle static/image changes
        self.process_static_changes(changes)?;

        let total_posts: usize = content.sections.values().map(|s| s.posts.len()).sum();
        println!(
            "Rebuilt {} posts in {} sections",
            total_posts,
            content.sections.len()
        );

        Ok(())
    }

    /// Rebuild only CSS
    fn rebuild_css_only(&self) -> Result<()> {
        let static_dir = self.output_dir.join("static");
        build_css(
            &self.resolve_path(&self.config.paths.styles),
            &static_dir.join(&self.config.build.css_output),
            self.config.build.minify_css,
        )?;
        println!("Rebuilt CSS");
        Ok(())
    }

    /// Process static file and image changes
    fn process_static_changes(&self, changes: &ChangeSet) -> Result<()> {
        let static_dir = self.output_dir.join("static");
        let source_static = self.resolve_path(&self.config.paths.static_files);

        let image_config = ImageConfig {
            quality: self.config.images.quality,
            scale_factor: self.config.images.scale_factor,
        };

        // Process changed images
        for rel_path in &changes.image_files {
            let src = source_static.join(rel_path.as_path());
            let dest = static_dir.join(rel_path.as_path());

            if src.exists() {
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                optimize_single_image(&src, &dest, &image_config)?;
                println!("Optimized image: {}", rel_path.display());
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
        let config_path = self.project_dir.join("config.toml");
        debug!("Reloading config from {:?}", config_path);
        self.config = crate::config::Config::load(&config_path)?;
        info!("Config reloaded successfully");
        Ok(())
    }

    /// Get a reference to the current config
    pub fn config(&self) -> &Config {
        &self.config
    }
}
