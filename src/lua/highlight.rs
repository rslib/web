//! Syntax highlighting module (rs.highlight)
//!
//! Provides server-side syntax highlighting using syntect.
//! Outputs HTML with CSS class names for styling.

use crate::lua::async_io::{AsyncIOTask, runtime};
use mlua::{Lua, Result, Table};
use once_cell::sync::Lazy;
use syntect::highlighting::ThemeSet;
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;

// Load syntax definitions and themes once
static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

/// Language name aliases (common names -> syntect names)
fn get_syntax_name(lang: &str) -> &str {
    match lang.to_lowercase().as_str() {
        "py" | "python" | "python3" => "Python",
        "rs" | "rust" => "Rust",
        "js" | "javascript" => "JavaScript",
        "ts" | "typescript" => "TypeScript",
        "c" => "C",
        "cpp" | "c++" | "cxx" => "C++",
        "h" | "hpp" => "C++",
        "hs" | "haskell" => "Haskell",
        "rb" | "ruby" => "Ruby",
        "go" | "golang" => "Go",
        "java" => "Java",
        "sh" | "bash" | "shell" => "Bourne Again Shell (bash)",
        "zsh" => "Bourne Again Shell (bash)",
        "json" => "JSON",
        "yaml" | "yml" => "YAML",
        "toml" => "TOML",
        "xml" => "XML",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "sql" => "SQL",
        "md" | "markdown" => "Markdown",
        "lua" => "Lua",
        "vim" => "VimL",
        "dockerfile" => "Dockerfile",
        "make" | "makefile" => "Makefile",
        "tex" | "latex" => "LaTeX",
        "r" => "R",
        "scala" => "Scala",
        "kotlin" | "kt" => "Kotlin",
        "swift" => "Swift",
        "objc" | "objective-c" => "Objective-C",
        "php" => "PHP",
        "perl" | "pl" => "Perl",
        "elixir" | "ex" => "Elixir",
        "erlang" | "erl" => "Erlang",
        "clojure" | "clj" => "Clojure",
        "lisp" | "el" => "Lisp",
        "scheme" | "scm" => "Lisp",
        "ocaml" | "ml" => "OCaml",
        "fsharp" | "fs" => "F#",
        "csharp" | "cs" => "C#",
        "diff" | "patch" => "Diff",
        "ini" | "conf" => "INI",
        "nginx" => "nginx",
        "asm" | "assembly" => "Assembly (x86_64)",
        _ => lang,
    }
}

/// Highlight code and return HTML with CSS classes (sync version)
/// This is also used by the Tera filter
pub fn highlight_code_sync(code: &str, language: &str) -> String {
    let syntax_name = get_syntax_name(language);

    // Try to find syntax by name
    let syntax = SYNTAX_SET
        .find_syntax_by_name(syntax_name)
        .or_else(|| SYNTAX_SET.find_syntax_by_extension(language))
        .or_else(|| SYNTAX_SET.find_syntax_by_token(language))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    let mut html_generator =
        ClassedHTMLGenerator::new_with_class_style(syntax, &SYNTAX_SET, ClassStyle::Spaced);

    for line in syntect::util::LinesWithEndings::from(code) {
        // Ignore errors for invalid UTF-8 or parsing issues
        let _ = html_generator.parse_html_for_line_which_includes_newline(line);
    }

    html_generator.finalize()
}

/// Get list of available syntax names
fn list_syntaxes() -> Vec<String> {
    SYNTAX_SET
        .syntaxes()
        .iter()
        .map(|s| s.name.clone())
        .collect()
}

/// Get list of available theme names
fn list_themes() -> Vec<String> {
    THEME_SET.themes.keys().cloned().collect()
}

/// Generate CSS for a specific theme
fn generate_theme_css(theme_name: &str) -> Option<String> {
    let theme = THEME_SET.themes.get(theme_name)?;
    syntect::html::css_for_theme_with_class_style(theme, ClassStyle::Spaced).ok()
}

/// Create the highlight module
pub fn create_module(lua: &Lua) -> Result<Table> {
    let module = lua.create_table()?;

    // rs.highlight.highlight(code, language) -> AsyncIOTask that resolves to html
    let highlight_fn = lua.create_function(|_, (code, language): (String, String)| {
        let handle = runtime().spawn(async move {
            // Use spawn_blocking for CPU-intensive work
            let result = tokio::task::spawn_blocking(move || highlight_code_sync(&code, &language))
                .await
                .map_err(|e| format!("Highlight task panicked: {}", e))?;
            Ok(result)
        });

        Ok(AsyncIOTask::from_string_handle(handle))
    })?;
    module.set("highlight", highlight_fn)?;

    // Alias: rs.highlight.code(code, language) -> AsyncIOTask
    module.set("code", module.get::<mlua::Function>("highlight")?)?;

    // rs.highlight.highlight_sync(code, language) -> html (blocking version)
    let highlight_sync_fn = lua.create_function(|_, (code, language): (String, String)| {
        Ok(highlight_code_sync(&code, &language))
    })?;
    module.set("highlight_sync", highlight_sync_fn)?;

    // rs.highlight.syntaxes() -> list of available syntax names
    let syntaxes_fn = lua.create_function(|_, ()| Ok(list_syntaxes()))?;
    module.set("syntaxes", syntaxes_fn)?;

    // rs.highlight.themes() -> list of available theme names
    let themes_fn = lua.create_function(|_, ()| Ok(list_themes()))?;
    module.set("themes", themes_fn)?;

    // rs.highlight.theme_css(theme_name) -> CSS string for the theme
    let theme_css_fn =
        lua.create_function(|_, theme_name: String| Ok(generate_theme_css(&theme_name)))?;
    module.set("theme_css", theme_css_fn)?;

    Ok(module)
}
