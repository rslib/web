//! Text processing module (rs.text)
//!
//! Text operations:
//! - rs.text.slugify, rs.text.word_count, rs.text.reading_time, etc.

use mlua::{Lua, Result, Table};

/// Create the text module table
pub fn create_module(lua: &Lua) -> Result<Table> {
    let text = lua.create_table()?;

    // slugify(text) - Convert text to URL-friendly slug
    let slugify_fn = lua.create_function(|_, text: String| {
        let slug = text
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>();
        let mut result = String::new();
        let mut last_was_dash = false;
        for c in slug.chars() {
            if c == '-' {
                if !last_was_dash && !result.is_empty() {
                    result.push(c);
                    last_was_dash = true;
                }
            } else {
                result.push(c);
                last_was_dash = false;
            }
        }
        Ok(result.trim_end_matches('-').to_string())
    })?;
    text.set("slugify", slugify_fn)?;

    // word_count(text) - Count words in text
    let word_count_fn = lua.create_function(|_, text: String| {
        let count = text.split_whitespace().count();
        Ok(count as i64)
    })?;
    text.set("word_count", word_count_fn)?;

    // reading_time(text, wpm?) - Calculate reading time in minutes
    let reading_time_fn = lua.create_function(|_, (text, wpm): (String, Option<i64>)| {
        let words_per_minute = wpm.unwrap_or(200) as f64;
        let word_count = text.split_whitespace().count() as f64;
        let minutes = (word_count / words_per_minute).ceil() as i64;
        Ok(std::cmp::max(1, minutes))
    })?;
    text.set("reading_time", reading_time_fn)?;

    // truncate(text, len, suffix?) - Truncate text with optional suffix
    let truncate_fn =
        lua.create_function(|_, (text, len, suffix): (String, usize, Option<String>)| {
            let suffix = suffix.unwrap_or_else(|| "...".to_string());
            if text.chars().count() <= len {
                Ok(text)
            } else {
                let truncated: String = text
                    .chars()
                    .take(len.saturating_sub(suffix.len()))
                    .collect();
                Ok(format!("{}{}", truncated.trim_end(), suffix))
            }
        })?;
    text.set("truncate", truncate_fn)?;

    // url_encode(str) - URL encode a string
    let url_encode_fn =
        lua.create_function(|_, text: String| Ok(urlencoding::encode(&text).to_string()))?;
    text.set("url_encode", url_encode_fn)?;

    // url_decode(str) - URL decode a string
    let url_decode_fn = lua.create_function(|_, text: String| {
        Ok(urlencoding::decode(&text)
            .map(|s| s.to_string())
            .unwrap_or(text))
    })?;
    text.set("url_decode", url_decode_fn)?;

    Ok(text)
}
