mod frontmatter;
mod page;
mod post;

pub use page::Page;
pub use post::{ContentType, Post};

// Re-export for tests
#[cfg(test)]
pub use frontmatter::Frontmatter;

use crate::config::PathsConfig;
use anyhow::Result;
use ignore::WalkBuilder;
use log::{debug, trace};
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

/// A section is a subdirectory containing posts (e.g., blog, projects, notes)
#[derive(Debug)]
pub struct Section {
    pub name: String,
    pub posts: Vec<Post>,
}

/// Content holds the home page and all discovered sections
#[derive(Debug)]
pub struct Content {
    pub home: Option<Page>,
    pub sections: HashMap<String, Section>,
}

/// Discover all content files based on paths config
/// If base_dir is provided, paths are resolved relative to it
pub fn discover_content(paths: &PathsConfig, base_dir: Option<&Path>) -> Result<Content> {
    debug!("Discovering content from {:?}", paths.content);
    let content_path = Path::new(&paths.content);
    let content_dir = if let Some(base) = base_dir {
        if content_path.is_absolute() {
            content_path.to_path_buf()
        } else {
            base.join(content_path)
        }
    } else {
        content_path.to_path_buf()
    };
    trace!("Content directory resolved to: {:?}", content_dir);

    // Build list of excluded directories (built-in + user-specified)
    let mut excluded: Vec<&str> = vec![&paths.styles, &paths.static_files, &paths.templates];
    excluded.extend(paths.exclude.iter().map(|s| s.as_str()));
    trace!("Excluded directories: {:?}", excluded);

    // Load home page
    let home_path = content_dir.join(&paths.home);
    let home = if home_path.exists() {
        trace!("Loading home page from {:?}", home_path);
        Some(Page::from_file(&home_path)?)
    } else {
        trace!("No home page found at {:?}", home_path);
        None
    };

    // Discover all sections (subdirectories)
    let mut sections = HashMap::new();

    // Collect section paths using appropriate walker
    let section_paths: Vec<_> = if paths.respect_gitignore {
        WalkBuilder::new(content_dir)
            .max_depth(Some(1))
            .hidden(false) // Don't skip hidden files by default
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.depth() == 1 && e.path().is_dir())
            .map(|e| e.into_path())
            .collect()
    } else {
        WalkDir::new(content_dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.into_path())
            .collect()
    };

    // Process each section
    for path in section_paths {
        process_section(&path, &excluded, &mut sections, paths)?;
    }

    debug!(
        "Content discovery complete: {} sections found",
        sections.len()
    );
    Ok(Content { home, sections })
}

/// Process a section directory and add it to sections map
fn process_section(
    path: &Path,
    excluded: &[&str],
    sections: &mut HashMap<String, Section>,
    paths: &PathsConfig,
) -> Result<()> {
    let section_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    // Skip excluded directories
    if excluded
        .iter()
        .any(|ex| section_name == *ex || path.ends_with(ex))
    {
        trace!("Skipping excluded section: {}", section_name);
        return Ok(());
    }

    trace!("Processing section: {}", section_name);

    // Collect content file paths (markdown and HTML) using appropriate walker
    let post_paths: Vec<_> = if paths.respect_gitignore {
        WalkBuilder::new(path)
            .max_depth(Some(1))
            .hidden(false)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.depth() == 1
                    && e.path()
                        .extension()
                        .is_some_and(|ext| ext == "md" || ext == "html" || ext == "htm")
            })
            .map(|e| e.into_path())
            .collect()
    } else {
        WalkDir::new(path)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "md" || ext == "html" || ext == "htm")
            })
            .map(|e| e.into_path())
            .collect()
    };

    // Load posts (can be parallelized with rayon if needed)
    let mut posts = Vec::new();
    for post_path in post_paths {
        let post = Post::from_file_with_section(&post_path, &section_name)?;
        if !post.frontmatter.draft.unwrap_or(false) {
            posts.push(post);
        }
    }

    // Sort posts by date (newest first)
    posts.sort_by(|a, b| b.frontmatter.date.cmp(&a.frontmatter.date));

    if !posts.is_empty() {
        debug!("Section '{}': {} posts loaded", section_name, posts.len());
        sections.insert(
            section_name.clone(),
            Section {
                name: section_name,
                posts,
            },
        );
    } else {
        trace!("Section '{}': no posts found", section_name);
    }

    Ok(())
}
