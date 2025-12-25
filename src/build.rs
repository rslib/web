use anyhow::{Context, Result};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

use crate::assets::{ImageConfig, build_css, copy_static_files, optimize_images};
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

/// Main build orchestrator
pub struct Builder {
    config: Config,
    output_dir: PathBuf,
}

impl Builder {
    pub fn new(config: Config, output_dir: PathBuf) -> Self {
        Self { config, output_dir }
    }

    pub fn build(&mut self) -> Result<()> {
        // Stage 1: Clean output directory
        self.clean()?;

        // Stage 2: Discover and load content
        let content = self.load_content()?;

        // Stage 3: Process assets
        self.process_assets()?;

        // Stage 4: Load templates (needed for HTML content processing)
        let templates = Templates::new(&self.config.paths.templates)?;

        // Stage 5: Process content through pipeline (markdown) or Tera (HTML)
        let pipeline = Pipeline::from_config(&self.config);
        let content = self.process_content(content, &pipeline, &templates)?;

        // Stage 6: Render and write HTML
        self.render_html(&content, &templates)?;

        let total_posts: usize = content.sections.values().map(|s| s.posts.len()).sum();
        println!(
            "Generated {} posts in {} sections",
            total_posts,
            content.sections.len()
        );

        Ok(())
    }

    fn clean(&self) -> Result<()> {
        if self.output_dir.exists() {
            fs::remove_dir_all(&self.output_dir).with_context(|| {
                format!("Failed to clean output directory: {:?}", self.output_dir)
            })?;
        }
        fs::create_dir_all(&self.output_dir)?;
        fs::create_dir_all(self.output_dir.join("static"))?;
        Ok(())
    }

    fn load_content(&self) -> Result<Content> {
        discover_content(&self.config.paths)
    }

    fn process_assets(&self) -> Result<()> {
        let static_dir = self.output_dir.join("static");
        let paths = &self.config.paths;

        // Build CSS
        build_css(
            Path::new(&paths.styles),
            &static_dir.join("rs.css"),
            self.config.build.minify_css,
        )?;

        // Optimize images
        let image_config = ImageConfig {
            quality: self.config.images.quality,
            scale_factor: self.config.images.scale_factor,
        };
        optimize_images(Path::new(&paths.static_files), &static_dir, &image_config)?;

        // Copy other static files
        copy_static_files(Path::new(&paths.static_files), &static_dir)?;

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
            let home_path = format!("{}/{}", paths.content, paths.home);
            let ctx = TransformContext {
                config: &self.config,
                current_path: Path::new(&home_path),
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
        // Handle HTML content files - process through Tera
        if post.content_type == ContentType::Html {
            return self.process_html_post(post, templates);
        }

        // Markdown processing
        let path_str = format!("{}/{}/{}.md", paths.content, section_name, post.file_slug);
        let path = PathBuf::from(&path_str);
        let ctx = TransformContext {
            config: &self.config,
            current_path: &path,
            base_url: &self.config.site.base_url,
        };

        // Check if post should be fully encrypted
        if post.frontmatter.encrypted {
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
        let link_graph = LinkGraph::build(&self.config, content);

        // Generate graph if enabled
        if self.config.graph.enabled {
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
            self.generate_rss(content)?;
        }

        Ok(())
    }

    fn generate_rss(&self, content: &Content) -> Result<()> {
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
}
