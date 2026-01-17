//! Text processing - slugify, word_count, reading_time, format_date, hash, path utils

use chrono::Datelike;
use mlua::{Lua, Result, Table, Value};
use std::path::Path;

/// Register text processing functions on the module table
pub fn register(lua: &Lua, module: &Table) -> Result<()> {
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
    module.set("slugify", slugify_fn)?;

    // word_count(text) - Count words in text
    let word_count_fn = lua.create_function(|_, text: String| {
        let count = text.split_whitespace().count();
        Ok(count as i64)
    })?;
    module.set("word_count", word_count_fn)?;

    // reading_time(text, wpm?) - Calculate reading time in minutes
    let reading_time_fn = lua.create_function(|_, (text, wpm): (String, Option<i64>)| {
        let words_per_minute = wpm.unwrap_or(200) as f64;
        let word_count = text.split_whitespace().count() as f64;
        let minutes = (word_count / words_per_minute).ceil() as i64;
        Ok(std::cmp::max(1, minutes))
    })?;
    module.set("reading_time", reading_time_fn)?;

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
    module.set("truncate", truncate_fn)?;

    // strip_tags(html) - Remove HTML tags (simple version)
    let strip_tags_fn = lua.create_function(|_, html: String| {
        let re = regex::Regex::new(r"<[^>]*>").unwrap();
        let text = re.replace_all(&html, "");
        // Decode common entities
        let text = text
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&nbsp;", " ");
        Ok(text.to_string())
    })?;
    module.set("strip_tags", strip_tags_fn)?;

    // format_date(date, format) - Format a date string
    // date can be "YYYY-MM-DD" or table {year, month, day}
    let format_date_fn = lua.create_function(|lua, (date, format): (Value, String)| {
        let parsed = match &date {
            Value::String(s) => {
                let s = s.to_str().map_err(mlua::Error::external)?;
                chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()
            }
            Value::Table(t) => {
                let year: i32 = t.get("year").unwrap_or(2000);
                let month: u32 = t.get("month").unwrap_or(1);
                let day: u32 = t.get("day").unwrap_or(1);
                chrono::NaiveDate::from_ymd_opt(year, month, day)
            }
            _ => None,
        };

        match parsed {
            Some(d) => {
                let formatted = d.format(&format).to_string();
                Ok(Value::String(lua.create_string(&formatted)?))
            }
            None => Ok(Value::Nil),
        }
    })?;
    module.set("format_date", format_date_fn)?;

    // parse_date(str) - Parse date string to table {year, month, day}
    let parse_date_fn = lua.create_function(|lua, date_str: String| {
        // Try common formats
        let formats = ["%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y", "%Y/%m/%d"];
        for fmt in &formats {
            if let Ok(d) = chrono::NaiveDate::parse_from_str(&date_str, fmt) {
                let result = lua.create_table()?;
                result.set("year", d.year())?;
                result.set("month", d.month())?;
                result.set("day", d.day())?;
                return Ok(Value::Table(result));
            }
        }
        Ok(Value::Nil)
    })?;
    module.set("parse_date", parse_date_fn)?;

    // join_path(...) - Join path segments
    let join_path_fn = lua.create_function(|_, parts: mlua::Variadic<String>| {
        let mut path = std::path::PathBuf::new();
        for part in parts {
            path.push(part);
        }
        Ok(path.to_string_lossy().to_string())
    })?;
    module.set("join_path", join_path_fn)?;

    // basename(path) - Get file name from path
    let basename_fn = lua.create_function(|_, path: String| {
        Ok(Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string())
    })?;
    module.set("basename", basename_fn)?;

    // dirname(path) - Get directory from path
    let dirname_fn = lua.create_function(|_, path: String| {
        Ok(Path::new(&path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string())
    })?;
    module.set("dirname", dirname_fn)?;

    // extension(path) - Get file extension
    let extension_fn = lua.create_function(|_, path: String| {
        Ok(Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string())
    })?;
    module.set("extension", extension_fn)?;

    // hash(content) - Hash content (xxHash64)
    let hash_fn = lua.create_function(|_, content: String| {
        use std::hash::{Hash, Hasher};
        let mut hasher = twox_hash::XxHash64::with_seed(0);
        content.hash(&mut hasher);
        Ok(format!("{:x}", hasher.finish()))
    })?;
    module.set("hash", hash_fn)?;

    // hash_file(path) - Hash file contents
    let hash_file_fn = lua.create_function(|lua, path: String| {
        use std::hash::Hasher;
        match std::fs::read(&path) {
            Ok(content) => {
                let mut hasher = twox_hash::XxHash64::with_seed(0);
                hasher.write(&content);
                let hash_str = format!("{:x}", hasher.finish());
                Ok(Value::String(lua.create_string(&hash_str)?))
            }
            Err(_) => Ok(Value::Nil),
        }
    })?;
    module.set("hash_file", hash_file_fn)?;

    // url_encode(str) - URL encode a string
    let url_encode_fn =
        lua.create_function(|_, text: String| Ok(urlencoding::encode(&text).to_string()))?;
    module.set("url_encode", url_encode_fn)?;

    // url_decode(str) - URL decode a string
    let url_decode_fn = lua.create_function(|_, text: String| {
        Ok(urlencoding::decode(&text)
            .map(|s| s.to_string())
            .unwrap_or(text))
    })?;
    module.set("url_decode", url_decode_fn)?;

    Ok(())
}
