//! Content processing - html_to_text, rss_date, link/image extraction
//!
//! Note: Markdown rendering has been moved to rs.markdown module

use crate::tracker::{SharedTracker, hash_str};
use mlua::{Lua, Result, Table, Value};

/// Register content-related Lua functions on the module table
pub fn register(lua: &Lua, module: &Table, tracker: SharedTracker) -> Result<()> {
    // rss_date(date_string) - Format date for RSS (RFC 2822)
    let rss_date_fn = lua.create_function(|lua, date_str: String| {
        // Try to parse various date formats
        let date = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .or_else(|_| chrono::NaiveDate::parse_from_str(&date_str, "%Y/%m/%d"))
            .or_else(|_| chrono::NaiveDate::parse_from_str(&date_str, "%d-%m-%Y"));

        match date {
            Ok(d) => {
                let datetime = d.and_hms_opt(0, 0, 0).unwrap();
                let formatted = datetime.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
                Ok(Value::String(lua.create_string(&formatted)?))
            }
            Err(_) => Ok(Value::Nil),
        }
    })?;
    module.set("rss_date", rss_date_fn)?;

    // extract_links_markdown(content) - Extract links from markdown content (including wikilinks)
    let extract_links_md_fn = lua.create_function(|lua, content: String| {
        use pulldown_cmark::{Event, Parser, Tag};
        use regex::Regex;

        let mut links = Vec::new();

        // Extract regular markdown links using pulldown_cmark
        let parser = Parser::new(&content);
        for event in parser {
            if let Event::Start(Tag::Link { dest_url, .. }) = event {
                links.push(dest_url.to_string());
            }
        }

        // Extract wikilinks: [[page]] or [[page|display text]]
        let wikilink_re = Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap();
        for cap in wikilink_re.captures_iter(&content) {
            if let Some(page) = cap.get(1) {
                // Convert wikilink to URL path (e.g., "My Page" -> "/my-page/")
                let slug = page.as_str().trim().to_lowercase().replace(' ', "-");
                links.push(format!("/{}/", slug));
            }
        }

        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        links.retain(|link| seen.insert(link.clone()));

        let result = lua.create_table()?;
        for (i, link) in links.iter().enumerate() {
            result.set(i + 1, link.as_str())?;
        }
        Ok(Value::Table(result))
    })?;
    module.set("extract_links_markdown", extract_links_md_fn)?;

    // extract_links_html(content) - Extract links from HTML content
    let extract_links_html_fn = lua.create_function(|lua, content: String| {
        use regex::Regex;

        // Match href="..." and href='...'
        let re = Regex::new(r#"href=["']([^"']+)["']"#).unwrap();
        let mut links: Vec<String> = re
            .captures_iter(&content)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect();

        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        links.retain(|link| seen.insert(link.clone()));

        let result = lua.create_table()?;
        for (i, link) in links.iter().enumerate() {
            result.set(i + 1, link.as_str())?;
        }
        Ok(Value::Table(result))
    })?;
    module.set("extract_links_html", extract_links_html_fn)?;

    // html_to_text(html) - Convert HTML to beautifully formatted plain text
    let tracker_clone = tracker.clone();
    let html_to_text_fn = lua.create_function(move |lua, html: String| {
        let content_hash = hash_str(&html);

        // Check memo cache first
        if let Some(cached) = tracker_clone.memo_get("html_to_text", content_hash)
            && let Ok(text) = String::from_utf8(cached)
        {
            return Ok(Value::String(lua.create_string(&text)?));
        }

        let text = crate::text::html_to_text(&html);

        // Store in memo cache
        tracker_clone.memo_set("html_to_text", content_hash, text.as_bytes().to_vec());

        Ok(Value::String(lua.create_string(&text)?))
    })?;
    module.set("html_to_text", html_to_text_fn)?;

    // extract_images_markdown(content) - Extract image paths from markdown
    let extract_images_md_fn = lua.create_function(|lua, content: String| {
        use regex::Regex;

        let mut images: Vec<String> = Vec::new();

        // Match ![alt](path) - standard markdown images
        let md_re = Regex::new(r"!\[[^\]]*\]\(([^)]+)\)").unwrap();
        for cap in md_re.captures_iter(&content) {
            if let Some(path) = cap.get(1) {
                images.push(path.as_str().to_string());
            }
        }

        // Match ![[image.png]] - wikilink style images
        let wiki_re = Regex::new(r"!\[\[([^\]|]+)(?:\|[^\]]+)?\]\]").unwrap();
        for cap in wiki_re.captures_iter(&content) {
            if let Some(path) = cap.get(1) {
                images.push(path.as_str().trim().to_string());
            }
        }

        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        images.retain(|img| seen.insert(img.clone()));

        let result = lua.create_table()?;
        for (i, img) in images.iter().enumerate() {
            result.set(i + 1, img.as_str())?;
        }
        Ok(Value::Table(result))
    })?;
    module.set("extract_images_markdown", extract_images_md_fn)?;

    // extract_images_html(content) - Extract image paths from HTML
    let extract_images_html_fn = lua.create_function(|lua, content: String| {
        use regex::Regex;

        // Match src="..." and src='...' in img tags
        let re = Regex::new(r#"<img[^>]*src=["']([^"']+)["']"#).unwrap();
        let mut images: Vec<String> = re
            .captures_iter(&content)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect();

        // Also match srcset
        let srcset_re = Regex::new(r#"srcset=["']([^"']+)["']"#).unwrap();
        for cap in srcset_re.captures_iter(&content) {
            if let Some(srcset) = cap.get(1) {
                // srcset can have multiple images: "img1.jpg 1x, img2.jpg 2x"
                for part in srcset.as_str().split(',') {
                    if let Some(path) = part.split_whitespace().next() {
                        images.push(path.to_string());
                    }
                }
            }
        }

        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        images.retain(|img| seen.insert(img.clone()));

        let result = lua.create_table()?;
        for (i, img) in images.iter().enumerate() {
            result.set(i + 1, img.as_str())?;
        }
        Ok(Value::Table(result))
    })?;
    module.set("extract_images_html", extract_images_html_fn)?;

    Ok(())
}
