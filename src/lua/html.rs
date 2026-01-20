//! HTML module (rs.html)

use crate::tracker::{SharedTracker, hash_str};
use mlua::{Lua, Result, Table, Value};

pub fn create_module(lua: &Lua, tracker: SharedTracker) -> Result<Table> {
    let html = lua.create_table()?;

    // to_text(html) - Convert HTML to plain text
    let tracker_clone = tracker.clone();
    let to_text_fn = lua.create_function(move |lua, html_content: String| {
        let content_hash = hash_str(&html_content);

        if let Some(cached) = tracker_clone.memo_get("html_to_text", content_hash)
            && let Ok(text) = String::from_utf8(cached)
        {
            return Ok(Value::String(lua.create_string(&text)?));
        }

        let text = crate::text::html_to_text(&html_content);
        tracker_clone.memo_set("html_to_text", content_hash, text.as_bytes().to_vec());

        Ok(Value::String(lua.create_string(&text)?))
    })?;
    html.set("to_text", to_text_fn)?;

    // strip_tags(html) - Remove HTML tags (simple version)
    let strip_tags_fn = lua.create_function(|_, html_content: String| {
        let re = regex::Regex::new(r"<[^>]*>").unwrap();
        let text = re.replace_all(&html_content, "");
        let text = text
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&nbsp;", " ");
        Ok(text.to_string())
    })?;
    html.set("strip_tags", strip_tags_fn)?;

    // extract_links(content) - Extract links from HTML
    let extract_links_fn = lua.create_function(|lua, content: String| {
        use regex::Regex;

        let re = Regex::new(r#"href=["']([^"']+)["']"#).unwrap();
        let mut links: Vec<String> = re
            .captures_iter(&content)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect();

        let mut seen = std::collections::HashSet::new();
        links.retain(|link| seen.insert(link.clone()));

        let result = lua.create_table()?;
        for (i, link) in links.iter().enumerate() {
            result.set(i + 1, link.as_str())?;
        }
        Ok(Value::Table(result))
    })?;
    html.set("extract_links", extract_links_fn)?;

    // extract_images(content) - Extract image paths from HTML
    let extract_images_fn = lua.create_function(|lua, content: String| {
        use regex::Regex;

        let re = Regex::new(r#"<img[^>]*src=["']([^"']+)["']"#).unwrap();
        let mut images: Vec<String> = re
            .captures_iter(&content)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect();

        let srcset_re = Regex::new(r#"srcset=["']([^"']+)["']"#).unwrap();
        for cap in srcset_re.captures_iter(&content) {
            if let Some(srcset) = cap.get(1) {
                for part in srcset.as_str().split(',') {
                    if let Some(path) = part.split_whitespace().next() {
                        images.push(path.to_string());
                    }
                }
            }
        }

        let mut seen = std::collections::HashSet::new();
        images.retain(|img| seen.insert(img.clone()));

        let result = lua.create_table()?;
        for (i, img) in images.iter().enumerate() {
            result.set(i + 1, img.as_str())?;
        }
        Ok(Value::Table(result))
    })?;
    html.set("extract_images", extract_images_fn)?;

    Ok(html)
}
