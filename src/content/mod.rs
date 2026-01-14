mod frontmatter;
mod page;
mod post;

pub use page::Page;
pub use post::{ContentType, Post};

// Re-export for tests
#[cfg(test)]
pub use frontmatter::Frontmatter;

use crate::config::{PathsConfig, SectionsConfig};
use anyhow::Result;
use ignore::WalkBuilder;
use log::{debug, trace};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

/// Default patterns to exclude (common non-content files)
const DEFAULT_EXCLUDE_PATTERNS: &[&str] = &[
    r"^README\.md$",
    r"^LICENSE\.md$",
    r"^CHANGELOG\.md$",
    r"^CONTRIBUTING\.md$",
    r"^CODE_OF_CONDUCT\.md$",
    r"^\.", // Hidden files/directories
];

/// Matcher for excluding files and directories based on regex patterns
pub struct ExcludeMatcher {
    patterns: Vec<Regex>,
}

impl ExcludeMatcher {
    /// Create a new exclude matcher from PathsConfig
    pub fn from_config(paths: &PathsConfig) -> Result<Self> {
        let mut patterns = Vec::new();

        // Add default patterns if enabled
        if paths.exclude_defaults {
            for pattern in DEFAULT_EXCLUDE_PATTERNS {
                patterns.push(Regex::new(pattern)?);
            }
        }

        // Add user-specified patterns
        for pattern in &paths.exclude {
            patterns.push(Regex::new(pattern)?);
        }

        Ok(Self { patterns })
    }

    /// Check if a name (file or directory) should be excluded
    pub fn is_excluded(&self, name: &str) -> bool {
        self.patterns.iter().any(|p| p.is_match(name))
    }
}

/// A section is a subdirectory containing posts (e.g., blog, projects, notes)
#[derive(Debug)]
pub struct Section {
    pub name: String,
    pub posts: Vec<Post>,
}

/// Content holds the home page, root pages, and all discovered sections
#[derive(Debug)]
pub struct Content {
    pub home: Option<Page>,
    /// Root-level pages (e.g., 404.md, about.md) excluding the home page
    pub root_pages: Vec<Page>,
    pub sections: HashMap<String, Section>,
}

/// Discover all content files based on paths config
/// If base_dir is provided, paths are resolved relative to it
pub fn discover_content(
    paths: &PathsConfig,
    sections_config: &SectionsConfig,
    base_dir: Option<&Path>,
) -> Result<Content> {
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

    // Create exclude matcher from config
    let exclude_matcher = ExcludeMatcher::from_config(paths)?;

    // Built-in excluded directories (styles, static, templates)
    let builtin_excluded: Vec<&str> = vec![&paths.styles, &paths.static_files, &paths.templates];
    trace!("Built-in excluded directories: {:?}", builtin_excluded);

    // Load home page
    let home_path = content_dir.join(&paths.home);
    let home = if home_path.exists() {
        trace!("Loading home page from {:?}", home_path);
        Some(Page::from_file(&home_path)?)
    } else {
        trace!("No home page found at {:?}", home_path);
        None
    };

    // Discover root-level pages (markdown files in content root, excluding home)
    let home_file_name = Path::new(&paths.home)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("index.md");

    let root_page_paths: Vec<_> = if paths.respect_gitignore {
        WalkBuilder::new(&content_dir)
            .max_depth(Some(1))
            .hidden(false)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let file_name = e.path().file_name().and_then(|n| n.to_str()).unwrap_or("");
                e.depth() == 1
                    && e.path().is_file()
                    && e.path()
                        .extension()
                        .is_some_and(|ext| ext == "md" || ext == "html" || ext == "htm")
                    && file_name != home_file_name
                    && !exclude_matcher.is_excluded(file_name)
            })
            .map(|e| e.into_path())
            .collect()
    } else {
        WalkDir::new(&content_dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let file_name = e.path().file_name().and_then(|n| n.to_str()).unwrap_or("");
                e.path().is_file()
                    && e.path()
                        .extension()
                        .is_some_and(|ext| ext == "md" || ext == "html" || ext == "htm")
                    && file_name != home_file_name
                    && !exclude_matcher.is_excluded(file_name)
            })
            .map(|e| e.into_path())
            .collect()
    };

    let mut root_pages = Vec::new();
    for page_path in root_page_paths {
        trace!("Loading root page from {:?}", page_path);
        let page = Page::from_file(&page_path)?;
        root_pages.push(page);
    }
    debug!("Loaded {} root pages", root_pages.len());

    // Discover all sections (subdirectories)
    let mut sections = HashMap::new();

    // Collect section paths using appropriate walker
    let section_paths: Vec<_> = if paths.respect_gitignore {
        WalkBuilder::new(content_dir)
            .max_depth(Some(1))
            .hidden(false) // Don't skip hidden files by default
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let dir_name = e.path().file_name().and_then(|n| n.to_str()).unwrap_or("");
                e.depth() == 1 && e.path().is_dir() && !exclude_matcher.is_excluded(dir_name)
            })
            .map(|e| e.into_path())
            .collect()
    } else {
        WalkDir::new(content_dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let dir_name = e.path().file_name().and_then(|n| n.to_str()).unwrap_or("");
                e.path().is_dir() && !exclude_matcher.is_excluded(dir_name)
            })
            .map(|e| e.into_path())
            .collect()
    };

    // Process each section
    for path in section_paths {
        process_section(
            &path,
            &builtin_excluded,
            &exclude_matcher,
            &mut sections,
            paths,
            sections_config,
        )?;
    }

    debug!(
        "Content discovery complete: {} sections found, {} root pages",
        sections.len(),
        root_pages.len()
    );
    Ok(Content {
        home,
        root_pages,
        sections,
    })
}

/// Process a section directory and add it to sections map
fn process_section(
    path: &Path,
    builtin_excluded: &[&str],
    exclude_matcher: &ExcludeMatcher,
    sections: &mut HashMap<String, Section>,
    paths: &PathsConfig,
    sections_config: &SectionsConfig,
) -> Result<()> {
    let section_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    // Skip built-in excluded directories (styles, static, templates)
    if builtin_excluded
        .iter()
        .any(|ex| section_name == *ex || path.ends_with(ex))
    {
        trace!("Skipping built-in excluded section: {}", section_name);
        return Ok(());
    }

    trace!("Processing section: {}", section_name);

    // Check if this section uses directory iteration
    let iterate_mode = sections_config
        .sections
        .get(&section_name)
        .map(|c| c.iterate.as_str())
        .unwrap_or("files");

    let mut posts = if iterate_mode == "directories" {
        // Directory-based iteration: each subdirectory becomes a post
        process_section_directories(path, &section_name, exclude_matcher, paths)?
    } else {
        // File-based iteration (default): find .md/.html files
        process_section_files(path, &section_name, exclude_matcher, paths)?
    };

    // Sort posts by date (newest first), then by slug for undated posts
    posts.sort_by(|a, b| match (&b.frontmatter.date, &a.frontmatter.date) {
        (Some(d1), Some(d2)) => d1.cmp(d2),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.slug().cmp(b.slug()),
    });

    if !posts.is_empty() {
        debug!(
            "Section '{}': {} posts loaded (mode: {})",
            section_name,
            posts.len(),
            iterate_mode
        );
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

/// Process section using file-based iteration (default)
/// Finds .md/.html files directly in the section directory
fn process_section_files(
    path: &Path,
    section_name: &str,
    exclude_matcher: &ExcludeMatcher,
    paths: &PathsConfig,
) -> Result<Vec<Post>> {
    // Collect content file paths (markdown and HTML) using appropriate walker
    let post_paths: Vec<_> = if paths.respect_gitignore {
        WalkBuilder::new(path)
            .max_depth(Some(1))
            .hidden(false)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let file_name = e.path().file_name().and_then(|n| n.to_str()).unwrap_or("");
                e.depth() == 1
                    && e.path()
                        .extension()
                        .is_some_and(|ext| ext == "md" || ext == "html" || ext == "htm")
                    && !exclude_matcher.is_excluded(file_name)
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
                let file_name = e.path().file_name().and_then(|n| n.to_str()).unwrap_or("");
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "md" || ext == "html" || ext == "htm")
                    && !exclude_matcher.is_excluded(file_name)
            })
            .map(|e| e.into_path())
            .collect()
    };

    // Load posts
    let mut posts = Vec::new();
    for post_path in post_paths {
        let post = Post::from_file_with_section(&post_path, section_name)?;
        if !post.frontmatter.draft.unwrap_or(false) {
            posts.push(post);
        }
    }

    Ok(posts)
}

/// Process section using directory-based iteration
/// Each subdirectory becomes a post with source_dir set
fn process_section_directories(
    path: &Path,
    section_name: &str,
    exclude_matcher: &ExcludeMatcher,
    paths: &PathsConfig,
) -> Result<Vec<Post>> {
    use crate::content::frontmatter::Frontmatter;
    use crate::content::post::ContentType;

    // Collect subdirectory paths
    let dir_paths: Vec<_> = if paths.respect_gitignore {
        WalkBuilder::new(path)
            .max_depth(Some(1))
            .hidden(false)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let dir_name = e.path().file_name().and_then(|n| n.to_str()).unwrap_or("");
                e.depth() == 1 && e.path().is_dir() && !exclude_matcher.is_excluded(dir_name)
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
                let dir_name = e.path().file_name().and_then(|n| n.to_str()).unwrap_or("");
                e.path().is_dir() && !exclude_matcher.is_excluded(dir_name)
            })
            .map(|e| e.into_path())
            .collect()
    };

    let mut posts = Vec::new();
    for dir_path in dir_paths {
        let slug = dir_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string();

        trace!(
            "Creating directory-based post: {} in section {}",
            slug, section_name
        );

        // Create a minimal Post with source_dir set
        // Templates will use Tera functions to load data from the directory
        let post = Post {
            file_slug: slug.clone(),
            section: section_name.to_string(),
            frontmatter: Frontmatter {
                title: slug.clone(), // Default title is the directory name
                description: None,
                date: None,
                tags: None,
                draft: None,
                image: None,
                template: None,
                slug: Some(slug),
                permalink: None,
                encrypted: false,
                password: None,
            },
            content: String::new(),
            html: String::new(),
            reading_time: 0,
            word_count: 0,
            encrypted_content: None,
            has_encrypted_blocks: false,
            content_type: ContentType::Markdown,
            source_path: dir_path.clone(),
            source_dir: Some(dir_path),
        };

        posts.push(post);
    }

    Ok(posts)
}
