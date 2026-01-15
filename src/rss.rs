use crate::config::Config;
use crate::content::Post;

/// Generate RSS 2.0 feed XML
pub fn generate_rss(config: &Config, posts: &[&Post]) -> String {
    let mut items = String::new();

    for post in posts {
        let url = format!("{}{}", config.site.base_url, post.url(config));
        let pub_date = post
            .frontmatter
            .date
            .map(|d| d.format("%a, %d %b %Y 00:00:00 GMT").to_string())
            .unwrap_or_default();

        let description = post.frontmatter.description.clone().unwrap_or_default();

        items.push_str(&format!(
            r#"    <item>
      <title>{}</title>
      <link>{}</link>
      <guid>{}</guid>
      <pubDate>{}</pubDate>
      <description><![CDATA[{}]]></description>
    </item>
"#,
            escape_xml(&post.frontmatter.title),
            url,
            url,
            pub_date,
            description
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>{}</title>
    <link>{}</link>
    <description>{}</description>
    <language>en-us</language>
    <atom:link href="{}/rss.xml" rel="self" type="application/rss+xml"/>
{}  </channel>
</rss>
"#,
        escape_xml(&config.site.title),
        config.site.base_url,
        escape_xml(&config.site.description),
        config.site.base_url,
        items
    )
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("Hello & World"), "Hello &amp; World");
        assert_eq!(escape_xml("<script>"), "&lt;script&gt;");
        assert_eq!(escape_xml("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(escape_xml("it's"), "it&apos;s");
    }

    #[test]
    fn test_generate_rss_structure() {
        let config = make_test_config();
        let posts: Vec<&Post> = vec![];

        let rss = generate_rss(&config, &posts);

        assert!(rss.starts_with("<?xml version=\"1.0\""));
        assert!(rss.contains("<rss version=\"2.0\""));
        assert!(rss.contains("<channel>"));
        assert!(rss.contains("</channel>"));
        assert!(rss.contains("</rss>"));
        assert!(rss.contains("<title>Test Site</title>"));
        assert!(rss.contains("<link>https://example.com</link>"));
    }

    #[test]
    fn test_generate_rss_with_post() {
        let config = make_test_config();
        let post = make_test_post("Test Post", "A test description");
        let posts: Vec<&Post> = vec![&post];

        let rss = generate_rss(&config, &posts);

        assert!(rss.contains("<item>"));
        assert!(rss.contains("<title>Test Post</title>"));
        assert!(rss.contains("<description><![CDATA[A test description]]></description>"));
    }

    #[test]
    fn test_generate_rss_escapes_xml() {
        let config = make_test_config();
        let post = make_test_post("Post & Title", "Description with <tags>");
        let posts: Vec<&Post> = vec![&post];

        let rss = generate_rss(&config, &posts);

        assert!(rss.contains("<title>Post &amp; Title</title>"));
    }

    fn make_test_config() -> Config {
        Config::from_data(crate::config::ConfigData {
            site: crate::config::SiteConfig {
                title: "Test Site".to_string(),
                description: "A test site".to_string(),
                base_url: "https://example.com".to_string(),
                author: "Test Author".to_string(),
            },
            seo: crate::config::SeoConfig {
                twitter_handle: None,
                default_og_image: None,
            },
            build: crate::config::BuildConfig {
                output_dir: "dist".to_string(),
                minify_css: false,
                css_output: "rs.css".to_string(),
            },
            images: crate::config::ImagesConfig {
                quality: 85.0,
                scale_factor: 1.0,
            },
            highlight: Default::default(),
            paths: Default::default(),
            templates: Default::default(),
            permalinks: Default::default(),
            encryption: Default::default(),
            graph: Default::default(),
            rss: Default::default(),
            text: Default::default(),
            sections: Default::default(),
        })
    }

    fn make_test_post(title: &str, description: &str) -> Post {
        use crate::content::{ContentType, Frontmatter};
        use std::path::PathBuf;

        Post {
            file_slug: "test-post".to_string(),
            section: "blog".to_string(),
            frontmatter: Frontmatter {
                title: title.to_string(),
                description: Some(description.to_string()),
                date: None,
                tags: None,
                draft: None,
                image: None,
                template: None,
                slug: None,
                permalink: None,
                encrypted: false,
                password: None,
            },
            content: String::new(),
            html: String::new(),
            reading_time: 1,
            word_count: 100,
            encrypted_content: None,
            has_encrypted_blocks: false,
            content_type: ContentType::Markdown,
            source_path: PathBuf::new(),
            source_dir: None,
        }
    }
}
