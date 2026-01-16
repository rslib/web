use glob::glob;
use pulldown_cmark::{Options, Parser, html};
use std::collections::HashMap;
use std::path::PathBuf;
use tera::{Function, Value};

use crate::lua::SharedTracker;
use crate::text::html_to_text;

/// Expand ~ to home directory
fn expand_tilde(path: &str) -> PathBuf {
    path.strip_prefix("~/")
        .and_then(|stripped| dirs::home_dir().map(|home| home.join(stripped)))
        .unwrap_or_else(|| PathBuf::from(path))
}

/// load_json(path) - Load and parse a JSON file
/// Returns the parsed JSON value, or Null if file doesn't exist or is invalid
pub fn make_load_json(tracker: Option<SharedTracker>) -> impl Function {
    move |args: &HashMap<String, Value>| -> tera::Result<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("load_json requires 'path' argument"))?;

        let path = expand_tilde(path);

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Ok(Value::Null),
        };

        // Track file read (canonicalize path for consistent matching)
        if let Some(ref tracker) = tracker {
            let canonical = path.canonicalize().unwrap_or(path);
            tracker.record_read(canonical, content.as_bytes());
        }

        match serde_json::from_str(&content) {
            Ok(v) => Ok(v),
            Err(_) => Ok(Value::Null),
        }
    }
}

/// read_file(path) - Read a file as text
/// Returns the file content as string, or Null if file doesn't exist
pub fn make_read_file(tracker: Option<SharedTracker>) -> impl Function {
    move |args: &HashMap<String, Value>| -> tera::Result<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("read_file requires 'path' argument"))?;

        let path = expand_tilde(path);

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                // Track file read (canonicalize path for consistent matching)
                if let Some(ref tracker) = tracker {
                    let canonical = path.canonicalize().unwrap_or(path);
                    tracker.record_read(canonical, content.as_bytes());
                }
                Ok(Value::String(content))
            }
            Err(_) => Ok(Value::Null),
        }
    }
}

/// read_markdown(path) - Read a Markdown file and return as HTML
/// Returns the rendered HTML as string, or Null if file doesn't exist
pub fn make_read_markdown(tracker: Option<SharedTracker>) -> impl Function {
    move |args: &HashMap<String, Value>| -> tera::Result<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("read_markdown requires 'path' argument"))?;

        let path = expand_tilde(path);

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                // Track file read (canonicalize path for consistent matching)
                if let Some(ref tracker) = tracker {
                    let canonical = path.canonicalize().unwrap_or(path);
                    tracker.record_read(canonical, content.as_bytes());
                }

                let options = Options::all();
                let parser = Parser::new_ext(&content, options);
                let mut html_output = String::new();
                html::push_html(&mut html_output, parser);
                Ok(Value::String(html_output))
            }
            Err(_) => Ok(Value::Null),
        }
    }
}

/// list_files(path, pattern?) - List files with metadata
/// Returns array of objects: [{path, name, stem, ext}, ...]
/// Optional pattern argument supports glob syntax (e.g., "solution.*", "*.py")
pub fn make_list_files(_tracker: Option<SharedTracker>) -> impl Function {
    move |args: &HashMap<String, Value>| -> tera::Result<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("list_files requires 'path' argument"))?;

        let base_path = expand_tilde(path);

        // Get optional pattern, default to "*" (all files)
        let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("*");

        // Build glob pattern
        let glob_pattern = base_path.join(pattern);
        let glob_str = glob_pattern.to_string_lossy();

        // Note: Directory listings are not tracked - changes to directory contents
        // will trigger full rebuilds via the file watcher

        let mut files: Vec<Value> = Vec::new();

        if let Ok(entries) = glob(&glob_str) {
            for entry in entries.flatten() {
                // Skip directories
                if entry.is_dir() {
                    continue;
                }

                let name = entry
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                let stem = entry
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                let ext = entry
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();

                let file_path = entry.to_string_lossy().to_string();

                let mut file_obj = serde_json::Map::new();
                file_obj.insert("path".to_string(), Value::String(file_path));
                file_obj.insert("name".to_string(), Value::String(name));
                file_obj.insert("stem".to_string(), Value::String(stem));
                file_obj.insert("ext".to_string(), Value::String(ext));

                files.push(Value::Object(file_obj));
            }
        }

        // Sort by name for consistent ordering
        files.sort_by(|a, b| {
            let name_a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let name_b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
            name_a.cmp(name_b)
        });

        Ok(Value::Array(files))
    }
}

/// list_dirs(path) - List subdirectories
/// Returns array of directory names (strings)
pub fn make_list_dirs(_tracker: Option<SharedTracker>) -> impl Function {
    move |args: &HashMap<String, Value>| -> tera::Result<Value> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("list_dirs requires 'path' argument"))?;

        let base_path = expand_tilde(path);

        // Note: Directory listings are not tracked - changes to directory contents
        // will trigger full rebuilds via the file watcher

        let mut dirs: Vec<Value> = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&base_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    // Skip non-directories and hidden directories
                    if path.is_dir() && !name.starts_with('.') {
                        dirs.push(Value::String(name.to_string()));
                    }
                }
            }
        }

        // Sort for consistent ordering
        dirs.sort_by(|a, b| {
            let name_a = a.as_str().unwrap_or("");
            let name_b = b.as_str().unwrap_or("");
            name_a.cmp(name_b)
        });

        Ok(Value::Array(dirs))
    }
}

/// markdown filter - Convert markdown text to HTML
pub fn markdown_filter(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    let text = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("markdown filter requires a string"))?;

    let options = Options::all();
    let parser = Parser::new_ext(text, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    Ok(Value::String(html_output))
}

/// html_to_text filter - Convert HTML to beautifully formatted plain text
pub fn html_to_text_filter(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    let html = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("html_to_text filter requires a string"))?;

    Ok(Value::String(html_to_text(html)))
}

/// linebreaks filter - Convert newlines to <br> tags and double newlines to paragraphs
/// Also supports basic markdown: **bold**, *label*, `code`
pub fn linebreaks_filter(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    let text = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("linebreaks filter requires a string"))?;

    // Split by double newlines for paragraphs
    let paragraphs: Vec<String> = text
        .split("\n\n")
        .map(|p| {
            let p = p.replace('\n', "<br>");
            // Convert **bold** to <strong> (must be before single *)
            let p = convert_bold(&p);
            // Convert *label* to <span class="label">
            let p = convert_label(&p);
            // Convert `code` to <code>
            convert_code(&p)
        })
        .collect();

    let html = format!("<p>{}</p>", paragraphs.join("</p><p>"));
    Ok(Value::String(html))
}

/// Convert **text** to <strong>text</strong>
#[allow(clippy::while_let_on_iterator)]
fn convert_bold(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '*' && chars.peek() == Some(&'*') {
            chars.next(); // consume second *
            // Find closing **
            let mut bold_text = String::new();
            let mut found_close = false;
            while let Some(bc) = chars.next() {
                if bc == '*' && chars.peek() == Some(&'*') {
                    chars.next(); // consume second *
                    found_close = true;
                    break;
                }
                bold_text.push(bc);
            }
            if found_close {
                result.push_str(&format!("<strong>{}</strong>", bold_text));
            } else {
                result.push_str("**");
                result.push_str(&bold_text);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert *text* to <span class="label">text</span>
#[allow(clippy::while_let_on_iterator)]
fn convert_label(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '*' {
            // Find closing *
            let mut label_text = String::new();
            let mut found_close = false;
            while let Some(lc) = chars.next() {
                if lc == '*' {
                    found_close = true;
                    break;
                }
                label_text.push(lc);
            }
            if found_close {
                result.push_str(&format!("<span class=\"label\">{}</span>", label_text));
            } else {
                result.push('*');
                result.push_str(&label_text);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert `text` to <code>text</code>
#[allow(clippy::while_let_on_iterator)]
fn convert_code(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '`' {
            // Find closing `
            let mut code_text = String::new();
            let mut found_close = false;
            while let Some(cc) = chars.next() {
                if cc == '`' {
                    found_close = true;
                    break;
                }
                code_text.push(cc);
            }
            if found_close {
                result.push_str(&format!("<code>{}</code>", code_text));
            } else {
                result.push('`');
                result.push_str(&code_text);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Register all data functions with Tera
pub fn register_data_functions(tera: &mut tera::Tera, tracker: Option<SharedTracker>) {
    tera.register_function("load_json", make_load_json(tracker.clone()));
    tera.register_function("read_file", make_read_file(tracker.clone()));
    tera.register_function("read_markdown", make_read_markdown(tracker.clone()));
    tera.register_function("list_files", make_list_files(tracker.clone()));
    tera.register_function("list_dirs", make_list_dirs(tracker));
    tera.register_filter("markdown", markdown_filter);
    tera.register_filter("linebreaks", linebreaks_filter);
    tera.register_filter("html_to_text", html_to_text_filter);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use tera::Function;

    #[test]
    fn test_expand_tilde() {
        let path = expand_tilde("~/test");
        assert!(path.to_string_lossy().contains("test"));
        assert!(!path.to_string_lossy().starts_with("~"));

        let path = expand_tilde("/absolute/path");
        assert_eq!(path, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_load_json() {
        let dir = tempdir().unwrap();
        let json_path = dir.path().join("test.json");
        fs::write(&json_path, r#"{"name": "test", "value": 42}"#).unwrap();

        let func = make_load_json(None);
        let mut args = HashMap::new();
        args.insert(
            "path".to_string(),
            Value::String(json_path.to_string_lossy().to_string()),
        );

        let result = func.call(&args).unwrap();
        assert_eq!(result.get("name").unwrap().as_str().unwrap(), "test");
        assert_eq!(result.get("value").unwrap().as_i64().unwrap(), 42);
    }

    #[test]
    fn test_load_json_missing_file() {
        let func = make_load_json(None);
        let mut args = HashMap::new();
        args.insert(
            "path".to_string(),
            Value::String("/nonexistent/path.json".to_string()),
        );

        let result = func.call(&args).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn test_read_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "Hello, World!").unwrap();

        let func = make_read_file(None);
        let mut args = HashMap::new();
        args.insert(
            "path".to_string(),
            Value::String(file_path.to_string_lossy().to_string()),
        );

        let result = func.call(&args).unwrap();
        assert_eq!(result.as_str().unwrap(), "Hello, World!");
    }

    #[test]
    fn test_read_file_missing() {
        let func = make_read_file(None);
        let mut args = HashMap::new();
        args.insert(
            "path".to_string(),
            Value::String("/nonexistent/file.txt".to_string()),
        );

        let result = func.call(&args).unwrap();
        assert!(result.is_null());
    }

    #[test]
    fn test_list_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("solution.py"), "print('hello')").unwrap();
        fs::write(dir.path().join("solution.cpp"), "int main(){}").unwrap();
        fs::write(dir.path().join("README.md"), "# Hello").unwrap();

        let func = make_list_files(None);
        let mut args = HashMap::new();
        args.insert(
            "path".to_string(),
            Value::String(dir.path().to_string_lossy().to_string()),
        );

        let result = func.call(&args).unwrap();
        let files = result.as_array().unwrap();
        assert_eq!(files.len(), 3);

        // Test with pattern
        args.insert(
            "pattern".to_string(),
            Value::String("solution.*".to_string()),
        );
        let result = func.call(&args).unwrap();
        let files = result.as_array().unwrap();
        assert_eq!(files.len(), 2);

        // Check file object structure
        let file = &files[0];
        assert!(file.get("path").is_some());
        assert!(file.get("name").is_some());
        assert!(file.get("stem").is_some());
        assert!(file.get("ext").is_some());
    }

    #[test]
    fn test_list_dirs() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("subdir1")).unwrap();
        fs::create_dir(dir.path().join("subdir2")).unwrap();
        fs::create_dir(dir.path().join(".hidden")).unwrap();
        fs::write(dir.path().join("file.txt"), "not a dir").unwrap();

        let func = make_list_dirs(None);
        let mut args = HashMap::new();
        args.insert(
            "path".to_string(),
            Value::String(dir.path().to_string_lossy().to_string()),
        );

        let result = func.call(&args).unwrap();
        let dirs = result.as_array().unwrap();
        assert_eq!(dirs.len(), 2); // Should exclude .hidden
        assert!(dirs.contains(&Value::String("subdir1".to_string())));
        assert!(dirs.contains(&Value::String("subdir2".to_string())));
    }
}
