use pulldown_cmark::{Event, Options, Parser, Tag};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

use crate::config::Config;
use crate::content::Content;

/// Represents a link from one post to another
#[derive(Debug, Clone)]
pub struct BacklinkInfo {
    /// URL of the source post
    pub url: String,
    /// Title of the source post
    pub title: String,
    /// Section the source post belongs to
    pub section: String,
}

/// Node in the graph (a post)
#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub title: String,
    pub section: String,
    pub url: String,
}

/// Edge in the graph (a link between posts)
#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

/// JSON-serializable graph for visualization
#[derive(Debug, Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Graph of internal links between posts
#[derive(Debug, Default)]
pub struct LinkGraph {
    /// Map from target URL -> list of posts that link to it
    backlinks: HashMap<String, Vec<BacklinkInfo>>,
    /// All edges (source -> target)
    edges: Vec<(String, String)>,
    /// All nodes
    nodes: HashMap<String, GraphNode>,
}

impl LinkGraph {
    /// Build link graph from all content
    pub fn build(config: &Config, content: &Content) -> Self {
        // First, build a set of all valid internal URLs and nodes
        let mut valid_urls: HashSet<String> = HashSet::new();
        let mut nodes: HashMap<String, GraphNode> = HashMap::new();

        for section in content.sections.values() {
            for post in &section.posts {
                let url = post.url(config);
                let normalized = normalize_url(&url);
                valid_urls.insert(normalized.clone());
                nodes.insert(
                    normalized.clone(),
                    GraphNode {
                        id: normalized,
                        title: post.frontmatter.title.clone(),
                        section: post.section.clone(),
                        url,
                    },
                );
            }
        }

        // Extract links from all posts
        let mut backlinks: HashMap<String, Vec<BacklinkInfo>> = HashMap::new();
        let mut edges: Vec<(String, String)> = Vec::new();

        for section in content.sections.values() {
            for post in &section.posts {
                let source_url = normalize_url(&post.url(config));
                let source_info = BacklinkInfo {
                    url: post.url(config),
                    title: post.frontmatter.title.clone(),
                    section: post.section.clone(),
                };

                // Extract all internal links from this post's content
                let links = extract_internal_links(&post.content, &valid_urls);

                for target_url in links {
                    // Track backlinks
                    backlinks
                        .entry(target_url.clone())
                        .or_default()
                        .push(source_info.clone());

                    // Track edges
                    edges.push((source_url.clone(), target_url));
                }
            }
        }

        Self {
            backlinks,
            edges,
            nodes,
        }
    }

    /// Get backlinks for a given URL
    pub fn backlinks_for(&self, url: &str) -> &[BacklinkInfo] {
        let normalized = normalize_url(url);
        self.backlinks
            .get(&normalized)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Export full graph data for JSON visualization
    pub fn to_graph_data(&self) -> GraphData {
        GraphData {
            nodes: self.nodes.values().cloned().collect(),
            edges: self
                .edges
                .iter()
                .map(|(source, target)| GraphEdge {
                    source: source.clone(),
                    target: target.clone(),
                })
                .collect(),
        }
    }

    /// Export graph data for a specific post (local neighborhood)
    /// Includes: the post itself, posts it links to, and posts that link to it
    pub fn local_graph_for(&self, url: &str) -> GraphData {
        let normalized = normalize_url(url);

        // Collect connected node IDs
        let mut connected_ids: HashSet<String> = HashSet::new();
        connected_ids.insert(normalized.clone());

        // Posts this one links to (outgoing)
        for (source, target) in &self.edges {
            if source == &normalized {
                connected_ids.insert(target.clone());
            }
        }

        // Posts that link to this one (incoming/backlinks)
        for (source, target) in &self.edges {
            if target == &normalized {
                connected_ids.insert(source.clone());
            }
        }

        // Filter nodes and edges
        let nodes: Vec<GraphNode> = self
            .nodes
            .values()
            .filter(|n| connected_ids.contains(&n.id))
            .cloned()
            .collect();

        let edges: Vec<GraphEdge> = self
            .edges
            .iter()
            .filter(|(s, t)| connected_ids.contains(s) && connected_ids.contains(t))
            .map(|(source, target)| GraphEdge {
                source: source.clone(),
                target: target.clone(),
            })
            .collect();

        GraphData { nodes, edges }
    }
}

/// Extract internal links from markdown content
fn extract_internal_links(content: &str, valid_urls: &HashSet<String>) -> Vec<String> {
    let options = Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_HEADING_ATTRIBUTES;

    let parser = Parser::new_ext(content, options);
    let mut links = Vec::new();

    for event in parser {
        if let Event::Start(Tag::Link { dest_url, .. }) = event {
            let url = dest_url.to_string();

            // Skip external links and anchors
            if url.starts_with("http://")
                || url.starts_with("https://")
                || url.starts_with('#')
                || url.starts_with("mailto:")
            {
                continue;
            }

            // Normalize and check if it's a valid internal link
            let normalized = normalize_url(&url);
            if valid_urls.contains(&normalized) {
                links.push(normalized);
            }
        }
    }

    links
}

/// Normalize URL for comparison (strip trailing slash, anchors)
pub fn normalize_url(url: &str) -> String {
    let url = url.split('#').next().unwrap_or(url); // Remove anchor
    let url = url.trim_matches('/'); // Remove leading/trailing slashes
    format!("/{}/", url) // Consistent format: /path/
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url() {
        assert_eq!(normalize_url("/blog/post/"), "/blog/post/");
        assert_eq!(normalize_url("/blog/post"), "/blog/post/");
        assert_eq!(normalize_url("blog/post/"), "/blog/post/");
        assert_eq!(normalize_url("blog/post"), "/blog/post/");
        assert_eq!(normalize_url("/blog/post#section"), "/blog/post/");
        assert_eq!(normalize_url("/blog/post/#section"), "/blog/post/");
    }

    #[test]
    fn test_extract_internal_links() {
        let valid_urls: HashSet<String> =
            vec!["/blog/post-1/".to_string(), "/blog/post-2/".to_string()]
                .into_iter()
                .collect();

        let content = r#"
Check out [post 1](/blog/post-1/) and [post 2](/blog/post-2).
Also see [external](https://example.com) and [anchor](#section).
And [invalid](/blog/post-3/) link.
"#;

        let links = extract_internal_links(content, &valid_urls);
        assert_eq!(links.len(), 2);
        assert!(links.contains(&"/blog/post-1/".to_string()));
        assert!(links.contains(&"/blog/post-2/".to_string()));
    }

    #[test]
    fn test_graph_data_serialization() {
        let graph = GraphData {
            nodes: vec![GraphNode {
                id: "/blog/test/".to_string(),
                title: "Test Post".to_string(),
                section: "blog".to_string(),
                url: "/blog/test/".to_string(),
            }],
            edges: vec![GraphEdge {
                source: "/blog/a/".to_string(),
                target: "/blog/b/".to_string(),
            }],
        };

        let json = serde_json::to_string(&graph).unwrap();
        assert!(json.contains("\"id\":\"/blog/test/\""));
        assert!(json.contains("\"source\":\"/blog/a/\""));
    }
}
