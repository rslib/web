//! SEO module (rs.seo)
//!
//! Provides:
//! - rs.seo.sitemap(options) - Generate XML sitemap

use crate::lua::async_io::{AsyncIOTask, runtime};
use crate::tracker::SharedTracker;
use mlua::{Lua, Result, Table, Value};
use std::path::{Path, PathBuf};

/// Create the rs.seo module table
pub fn create_module(lua: &Lua, project_root: &Path, tracker: SharedTracker) -> Result<Table> {
    let seo_module = lua.create_table()?;
    let root = project_root.to_path_buf();

    // rs.seo.sitemap(options) - Generate XML sitemap (async)
    // options: { base_url, pages, output, default_changefreq?, default_priority? }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let sitemap_fn = lua.create_function(move |lua, options: Table| {
        let opts = SitemapOptions::from_lua(&options, lua)?;
        let output = resolve_output_path(&opts.output, &root_clone);
        let tracker = tracker_clone.clone();

        let handle = runtime().spawn(async move {
            let sitemap = generate_sitemap(&opts);
            write_output_async(&output, &sitemap, &tracker).await
        });

        Ok(AsyncIOTask::new(handle))
    })?;
    seo_module.set("sitemap", sitemap_fn)?;

    // rs.seo.robots(options) - Generate robots.txt (async)
    // options: { sitemap_url?, allow?, disallow?, user_agent?, output }
    let root_clone = root.clone();
    let tracker_clone = tracker.clone();
    let robots_fn = lua.create_function(move |lua, options: Table| {
        let opts = RobotsOptions::from_lua(&options, lua)?;
        let output = resolve_output_path(&opts.output, &root_clone);
        let tracker = tracker_clone.clone();

        let handle = runtime().spawn(async move {
            let robots = generate_robots(&opts);
            write_output_async(&output, &robots, &tracker).await
        });

        Ok(AsyncIOTask::new(handle))
    })?;
    seo_module.set("robots", robots_fn)?;

    Ok(seo_module)
}

/// Options for sitemap generation
#[derive(Debug, Clone)]
struct SitemapOptions {
    base_url: String,
    pages: Vec<SitemapPage>,
    output: String,
    default_changefreq: Option<String>,
    default_priority: Option<f64>,
    exclude: Vec<String>,
}

#[derive(Debug, Clone)]
struct SitemapPage {
    path: String,
    lastmod: Option<String>,
    changefreq: Option<String>,
    priority: Option<f64>,
}

impl SitemapOptions {
    fn from_lua(options: &Table, _lua: &Lua) -> mlua::Result<Self> {
        let base_url: String = options
            .get("base_url")
            .map_err(|_| mlua::Error::external("seo.sitemap: 'base_url' is required"))?;

        let output: String = options
            .get("output")
            .map_err(|_| mlua::Error::external("seo.sitemap: 'output' is required"))?;

        let default_changefreq: Option<String> = options.get("default_changefreq").ok();
        let default_priority: Option<f64> = options.get("default_priority").ok();
        let exclude = parse_string_array(options, "exclude");

        // Parse pages array
        let mut pages = Vec::new();
        if let Ok(pages_table) = options.get::<Table>("pages") {
            for (_, page_value) in pages_table.pairs::<i64, Value>().flatten() {
                match page_value {
                    // Simple string path
                    Value::String(s) => {
                        if let Ok(path) = s.to_str() {
                            pages.push(SitemapPage {
                                path: path.to_string(),
                                lastmod: None,
                                changefreq: None,
                                priority: None,
                            });
                        }
                    }
                    // Table with path and optional metadata
                    Value::Table(t) => {
                        if let Ok(path) = t.get::<String>("path") {
                            pages.push(SitemapPage {
                                path,
                                lastmod: t.get("lastmod").ok(),
                                changefreq: t.get("changefreq").ok(),
                                priority: t.get("priority").ok(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(Self {
            base_url,
            pages,
            output,
            default_changefreq,
            default_priority,
            exclude,
        })
    }
}

/// Check if a path matches any exclude pattern
fn is_excluded(path: &str, exclude: &[String]) -> bool {
    exclude.iter().any(|pattern| {
        if pattern.contains('*') {
            // Simple glob matching
            let regex_pattern = pattern.replace('.', r"\.").replace('*', ".*");
            regex::Regex::new(&format!("^{}$", regex_pattern))
                .map(|re| re.is_match(path))
                .unwrap_or(false)
        } else {
            path == pattern || path.starts_with(pattern)
        }
    })
}

/// Generate the sitemap XML content
fn generate_sitemap(opts: &SitemapOptions) -> String {
    let mut xml = String::new();
    xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#);
    xml.push('\n');

    let base_url = opts.base_url.trim_end_matches('/');

    for page in &opts.pages {
        // Check if page should be excluded
        let path = if page.path.starts_with('/') {
            page.path.clone()
        } else {
            format!("/{}", page.path)
        };

        if is_excluded(&path, &opts.exclude) {
            continue;
        }

        xml.push_str("  <url>\n");

        // loc (required)
        xml.push_str(&format!("    <loc>{}{}</loc>\n", base_url, path));

        // lastmod (optional)
        if let Some(lastmod) = &page.lastmod {
            xml.push_str(&format!("    <lastmod>{}</lastmod>\n", lastmod));
        }

        // changefreq (optional, use default if not specified)
        let changefreq = page
            .changefreq
            .as_ref()
            .or(opts.default_changefreq.as_ref());
        if let Some(freq) = changefreq {
            xml.push_str(&format!("    <changefreq>{}</changefreq>\n", freq));
        }

        // priority (optional, use default if not specified)
        let priority = page.priority.or(opts.default_priority);
        if let Some(p) = priority {
            xml.push_str(&format!("    <priority>{:.1}</priority>\n", p));
        }

        xml.push_str("  </url>\n");
    }

    xml.push_str("</urlset>\n");
    xml
}

/// Options for robots.txt generation
#[derive(Debug, Clone)]
struct RobotsOptions {
    sitemap_url: Option<String>,
    user_agent: String,
    allow: Vec<String>,
    disallow: Vec<String>,
    output: String,
}

impl RobotsOptions {
    fn from_lua(options: &Table, _lua: &Lua) -> mlua::Result<Self> {
        let output: String = options
            .get("output")
            .map_err(|_| mlua::Error::external("seo.robots: 'output' is required"))?;

        let sitemap_url: Option<String> = options.get("sitemap_url").ok();
        let user_agent: String = options
            .get("user_agent")
            .unwrap_or_else(|_| "*".to_string());

        let allow = parse_string_array(options, "allow");
        let disallow = parse_string_array(options, "disallow");

        Ok(Self {
            sitemap_url,
            user_agent,
            allow,
            disallow,
            output,
        })
    }
}

fn parse_string_array(options: &Table, key: &str) -> Vec<String> {
    options
        .get::<Table>(key)
        .ok()
        .map(|t| {
            t.pairs::<i64, String>()
                .filter_map(|p| p.ok())
                .map(|(_, v)| v)
                .collect()
        })
        .unwrap_or_default()
}

/// Generate robots.txt content
fn generate_robots(opts: &RobotsOptions) -> String {
    let mut content = String::new();

    content.push_str(&format!("User-agent: {}\n", opts.user_agent));

    for path in &opts.allow {
        content.push_str(&format!("Allow: {}\n", path));
    }

    for path in &opts.disallow {
        content.push_str(&format!("Disallow: {}\n", path));
    }

    if let Some(sitemap) = &opts.sitemap_url {
        content.push('\n');
        content.push_str(&format!("Sitemap: {}\n", sitemap));
    }

    content
}

fn resolve_output_path(output_path: &str, root: &Path) -> PathBuf {
    if Path::new(output_path).is_absolute() {
        PathBuf::from(output_path)
    } else {
        root.join(output_path)
    }
}

async fn write_output_async(
    output: &Path,
    content: &str,
    tracker: &SharedTracker,
) -> std::result::Result<(), String> {
    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    tokio::fs::write(output, content)
        .await
        .map_err(|e| format!("Failed to write: {}", e))?;

    let canonical = output
        .canonicalize()
        .unwrap_or_else(|_| output.to_path_buf());
    tracker.record_write_async(canonical, content.as_bytes());
    Ok(())
}
