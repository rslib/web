//! Markdown rendering with plugin-based AST transformations
//!
//! Usage:
//! ```lua
//! -- Simple (uses default plugins)
//! local html = rs.markdown.render(content)
//!
//! -- With custom plugins
//! local html = rs.markdown.render(content, {
//!   plugins = rs.markdown.plugins(
//!     rs.markdown.plugins.default({ lazy_images = false }),
//!     my_custom_plugin
//!   ),
//! })
//! ```

use mlua::{Function, Lua, MetaMethod, Result, Table, Value};
use pulldown_cmark::{Alignment, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};

use crate::tracker::{SharedTracker, hash_str};

/// Register the rs.markdown module
pub fn register(lua: &Lua, rs_module: &Table, tracker: SharedTracker) -> Result<()> {
    let markdown_module = lua.create_table()?;

    // Create plugins table (callable via __call metatable)
    let plugins_table = create_plugins_table(lua)?;
    markdown_module.set("plugins", plugins_table)?;

    // rs.markdown.render(content, opts?)
    let tracker_clone = tracker.clone();
    let render_fn = lua.create_function(move |lua, args: mlua::Variadic<Value>| {
        let content = args
            .first()
            .and_then(|v| v.as_string().and_then(|s| s.to_str().ok()))
            .ok_or_else(|| mlua::Error::external("render requires content string"))?
            .to_string();

        let opts: Option<Table> = args.get(1).and_then(|v| match v {
            Value::Table(t) => Some(t.clone()),
            _ => None,
        });

        // Strip frontmatter if present (delimited by ---)
        let content = strip_frontmatter(&content);

        // Get plugins from opts, or use defaults
        let plugins: Option<Table> = if let Some(ref opts) = opts {
            opts.get("plugins").ok()
        } else {
            None
        };

        // If no plugins specified, use fast path with memoization
        if plugins.is_none() {
            let content_hash = hash_str(&content);

            // Check memo cache first
            if let Some(cached) = tracker_clone.memo_get("markdown_render", content_hash)
                && let Ok(html) = String::from_utf8(cached)
            {
                return Ok(Value::String(lua.create_string(&html)?));
            }

            // Parse and apply default plugins
            let mut ast = parse_to_ast(lua, &content)?;
            ast = apply_default_plugins(lua, ast)?;
            let html_output = ast_to_html(lua, &ast)?;

            // Store in memo cache
            tracker_clone.memo_set(
                "markdown_render",
                content_hash,
                html_output.as_bytes().to_vec(),
            );

            return Ok(Value::String(lua.create_string(&html_output)?));
        }

        // Parse markdown to AST
        let mut ast = parse_to_ast(lua, &content)?;

        // Apply plugins in order (pipeline)
        let plugins = plugins.unwrap();
        for pair in plugins.pairs::<i64, Value>() {
            let (_, plugin) = pair?;
            if let Value::Function(plugin_fn) = plugin {
                let result: Value = plugin_fn.call(ast.clone())?;
                if let Value::Table(new_ast) = result {
                    ast = new_ast;
                }
            }
        }

        // Convert AST to HTML
        let html_output = ast_to_html(lua, &ast)?;

        Ok(Value::String(lua.create_string(&html_output)?))
    })?;
    markdown_module.set("render", render_fn)?;

    rs_module.set("markdown", markdown_module)?;
    Ok(())
}

/// Create the plugins table with __call metatable for concatenation
fn create_plugins_table(lua: &Lua) -> Result<Table> {
    let plugins = lua.create_table()?;

    // plugins.default(opts?) - returns array of default plugins with optional disabling
    let default_fn = lua.create_function(|lua, opts: Option<Table>| {
        let result = lua.create_table()?;
        let mut idx = 1;

        let lazy_images_enabled = opts
            .as_ref()
            .and_then(|o| o.get::<bool>("lazy_images").ok())
            .unwrap_or(true);
        let heading_anchors_enabled = opts
            .as_ref()
            .and_then(|o| o.get::<bool>("heading_anchors").ok())
            .unwrap_or(true);
        let external_links_enabled = opts
            .as_ref()
            .and_then(|o| o.get::<bool>("external_links").ok())
            .unwrap_or(true);

        if lazy_images_enabled {
            let plugin = create_lazy_images_plugin(lua, None)?;
            result.set(idx, plugin)?;
            idx += 1;
        }
        if heading_anchors_enabled {
            let plugin = create_heading_anchors_plugin(lua, None)?;
            result.set(idx, plugin)?;
            idx += 1;
        }
        if external_links_enabled {
            let plugin = create_external_links_plugin(lua, None)?;
            result.set(idx, plugin)?;
        }

        Ok(result)
    })?;
    plugins.set("default", default_fn)?;

    // plugins.lazy_images(opts?) - factory returning plugin function
    let lazy_images_fn =
        lua.create_function(|lua, opts: Option<Table>| create_lazy_images_plugin(lua, opts))?;
    plugins.set("lazy_images", lazy_images_fn)?;

    // plugins.heading_anchors(opts?) - factory returning plugin function
    let heading_anchors_fn =
        lua.create_function(|lua, opts: Option<Table>| create_heading_anchors_plugin(lua, opts))?;
    plugins.set("heading_anchors", heading_anchors_fn)?;

    // plugins.external_links(opts?) - factory returning plugin function
    let external_links_fn =
        lua.create_function(|lua, opts: Option<Table>| create_external_links_plugin(lua, opts))?;
    plugins.set("external_links", external_links_fn)?;

    // Set metatable with __call for concatenation
    let metatable = lua.create_table()?;
    let call_fn = lua.create_function(|lua, args: mlua::Variadic<Value>| {
        let result = lua.create_table()?;
        let mut idx = 1;

        // Skip first arg (self - the plugins table)
        for arg in args.iter().skip(1) {
            match arg {
                Value::Function(f) => {
                    result.set(idx, f.clone())?;
                    idx += 1;
                }
                Value::Table(t) => {
                    // Flatten tables
                    for pair in t.clone().pairs::<i64, Value>() {
                        let (_, v) = pair?;
                        if let Value::Function(f) = v {
                            result.set(idx, f)?;
                            idx += 1;
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(result)
    })?;
    metatable.set(MetaMethod::Call.name(), call_fn)?;
    plugins.set_metatable(Some(metatable))?;

    Ok(plugins)
}

/// Create lazy_images plugin function
fn create_lazy_images_plugin(lua: &Lua, _opts: Option<Table>) -> Result<Function> {
    lua.create_function(|lua, ast: Table| {
        let result = lua.create_table()?;
        let mut result_idx = 1;
        let len = ast.len()?;

        let mut i = 1;
        while i <= len {
            let event: Table = ast.get(i)?;
            let event_type: String = event.get("type")?;

            if event_type == "start" {
                let tag: String = event.get("tag").unwrap_or_default();
                if tag == "image" {
                    // Collect image info
                    let url: String = event.get("url").unwrap_or_default();
                    let title: String = event.get("title").unwrap_or_default();

                    // Look ahead for alt text
                    let mut alt = String::new();
                    let mut j = i + 1;
                    while j <= len {
                        let next_event: Table = ast.get(j)?;
                        let next_type: String = next_event.get("type")?;
                        if next_type == "text" {
                            alt = next_event.get("content").unwrap_or_default();
                        } else if next_type == "end" {
                            let next_tag: String = next_event.get("tag").unwrap_or_default();
                            if next_tag == "image" {
                                break;
                            }
                        }
                        j += 1;
                    }

                    // Emit HTML with lazy loading
                    let html_event = lua.create_table()?;
                    html_event.set("type", "html")?;
                    html_event.set(
                        "content",
                        format!(
                            r#"<img src="{}" alt="{}" loading="lazy" decoding="async"{}>"#,
                            html_escape(&url),
                            html_escape(&alt),
                            if title.is_empty() {
                                String::new()
                            } else {
                                format!(r#" title="{}""#, html_escape(&title))
                            }
                        ),
                    )?;
                    result.set(result_idx, html_event)?;
                    result_idx += 1;

                    // Skip to after the end image event
                    i = j + 1;
                    continue;
                }
            }

            // Pass through unchanged
            result.set(result_idx, event)?;
            result_idx += 1;
            i += 1;
        }

        Ok(result)
    })
}

/// Create heading_anchors plugin function
fn create_heading_anchors_plugin(lua: &Lua, _opts: Option<Table>) -> Result<Function> {
    lua.create_function(|lua, ast: Table| {
        let result = lua.create_table()?;
        let mut result_idx = 1;
        let len = ast.len()?;

        // Track heading slugs for deduplication
        let mut slug_counts: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();

        let mut i = 1;
        while i <= len {
            let event: Table = ast.get(i)?;
            let event_type: String = event.get("type")?;

            if event_type == "start" {
                let tag: String = event.get("tag").unwrap_or_default();
                if tag == "heading" {
                    let level: u8 = event.get("level").unwrap_or(1);

                    // Collect heading text
                    let mut heading_text = String::new();
                    let mut j = i + 1;
                    while j <= len {
                        let next_event: Table = ast.get(j)?;
                        let next_type: String = next_event.get("type")?;
                        if next_type == "text" {
                            let content: String = next_event.get("content").unwrap_or_default();
                            heading_text.push_str(&content);
                        } else if next_type == "end" {
                            let next_tag: String = next_event.get("tag").unwrap_or_default();
                            if next_tag == "heading" {
                                break;
                            }
                        }
                        j += 1;
                    }

                    // Generate slug
                    let base_slug = slugify(&heading_text);
                    let count = slug_counts.entry(base_slug.clone()).or_insert(0);
                    let slug = if *count > 0 {
                        format!("{}-{}", base_slug, count)
                    } else {
                        base_slug
                    };
                    *count += 1;

                    // Emit start with ID
                    let start_html = lua.create_table()?;
                    start_html.set("type", "html")?;
                    start_html.set("content", format!(r#"<h{} id="{}">"#, level, slug))?;
                    result.set(result_idx, start_html)?;
                    result_idx += 1;

                    // Copy content events
                    for k in (i + 1)..j {
                        let content_event: Table = ast.get(k)?;
                        result.set(result_idx, content_event)?;
                        result_idx += 1;
                    }

                    // Emit end
                    let end_html = lua.create_table()?;
                    end_html.set("type", "html")?;
                    end_html.set("content", format!("</h{}>", level))?;
                    result.set(result_idx, end_html)?;
                    result_idx += 1;

                    // Skip to after the end heading event
                    i = j + 1;
                    continue;
                }
            }

            // Pass through unchanged
            result.set(result_idx, event)?;
            result_idx += 1;
            i += 1;
        }

        Ok(result)
    })
}

/// Create external_links plugin function
fn create_external_links_plugin(lua: &Lua, _opts: Option<Table>) -> Result<Function> {
    lua.create_function(|lua, ast: Table| {
        let result = lua.create_table()?;
        let mut result_idx = 1;
        let len = ast.len()?;

        let mut i = 1;
        while i <= len {
            let event: Table = ast.get(i)?;
            let event_type: String = event.get("type")?;

            if event_type == "start" {
                let tag: String = event.get("tag").unwrap_or_default();
                if tag == "link" {
                    let url: String = event.get("url").unwrap_or_default();
                    let title: String = event.get("title").unwrap_or_default();

                    if is_external_url(&url) {
                        // Emit HTML for external link
                        let html_event = lua.create_table()?;
                        html_event.set("type", "html")?;
                        html_event.set(
                            "content",
                            format!(
                                r#"<a href="{}" target="_blank" rel="noopener noreferrer"{}>"#,
                                html_escape(&url),
                                if title.is_empty() {
                                    String::new()
                                } else {
                                    format!(r#" title="{}""#, html_escape(&title))
                                }
                            ),
                        )?;
                        result.set(result_idx, html_event)?;
                        result_idx += 1;

                        // Copy content until end link
                        let mut j = i + 1;
                        while j <= len {
                            let next_event: Table = ast.get(j)?;
                            let next_type: String = next_event.get("type")?;

                            if next_type == "end" {
                                let next_tag: String = next_event.get("tag").unwrap_or_default();
                                if next_tag == "link" {
                                    // Emit closing tag
                                    let end_html = lua.create_table()?;
                                    end_html.set("type", "html")?;
                                    end_html.set("content", "</a>")?;
                                    result.set(result_idx, end_html)?;
                                    result_idx += 1;
                                    break;
                                }
                            }

                            result.set(result_idx, next_event)?;
                            result_idx += 1;
                            j += 1;
                        }

                        i = j + 1;
                        continue;
                    }
                }
            }

            // Pass through unchanged
            result.set(result_idx, event)?;
            result_idx += 1;
            i += 1;
        }

        Ok(result)
    })
}

/// Apply default plugins to AST
fn apply_default_plugins(lua: &Lua, ast: Table) -> Result<Table> {
    let lazy_images = create_lazy_images_plugin(lua, None)?;
    let heading_anchors = create_heading_anchors_plugin(lua, None)?;
    let external_links = create_external_links_plugin(lua, None)?;

    let ast = lazy_images.call::<Table>(ast)?;
    let ast = heading_anchors.call::<Table>(ast)?;
    let ast = external_links.call::<Table>(ast)?;

    Ok(ast)
}

/// Parse markdown content to Lua AST (array of event tables)
fn parse_to_ast(lua: &Lua, content: &str) -> Result<Table> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(content, options);
    let ast = lua.create_table()?;
    let mut idx = 1;

    for event in parser {
        let event_table = lua.create_table()?;

        match &event {
            Event::Text(text) => {
                event_table.set("type", "text")?;
                event_table.set("content", text.to_string())?;
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                event_table.set("type", "html")?;
                event_table.set("content", html.to_string())?;
            }
            Event::Code(code) => {
                event_table.set("type", "code")?;
                event_table.set("content", code.to_string())?;
            }
            Event::SoftBreak => {
                event_table.set("type", "softbreak")?;
            }
            Event::HardBreak => {
                event_table.set("type", "hardbreak")?;
            }
            Event::Rule => {
                event_table.set("type", "rule")?;
            }
            Event::Start(tag) => {
                event_table.set("type", "start")?;
                set_tag_info(lua, &event_table, tag)?;
            }
            Event::End(tag_end) => {
                event_table.set("type", "end")?;
                set_tag_end_info(&event_table, tag_end)?;
            }
            Event::FootnoteReference(label) => {
                event_table.set("type", "footnote_ref")?;
                event_table.set("label", label.to_string())?;
            }
            Event::TaskListMarker(checked) => {
                event_table.set("type", "task_marker")?;
                event_table.set("checked", *checked)?;
            }
            Event::DisplayMath(math) => {
                event_table.set("type", "display_math")?;
                event_table.set("content", math.to_string())?;
            }
            Event::InlineMath(math) => {
                event_table.set("type", "inline_math")?;
                event_table.set("content", math.to_string())?;
            }
        }

        ast.set(idx, event_table)?;
        idx += 1;
    }

    Ok(ast)
}

/// Convert Lua AST back to HTML
fn ast_to_html(lua: &Lua, ast: &Table) -> Result<String> {
    let mut events: Vec<Event<'static>> = Vec::new();

    for pair in ast.clone().pairs::<i64, Table>() {
        let (_, event_table) = pair?;
        if let Some(event) = lua_table_to_event(lua, &event_table)? {
            events.push(event);
        }
    }

    let mut html_output = String::new();
    html::push_html(&mut html_output, events.into_iter());

    Ok(html_output)
}

/// Strip frontmatter from content
fn strip_frontmatter(content: &str) -> String {
    if let Some(stripped) = content.strip_prefix("---")
        && let Some(end) = stripped.find("---")
    {
        return stripped[end + 3..].trim_start().to_string();
    }
    content.to_string()
}

/// Convert text to URL-friendly slug
fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c.is_whitespace() || c == '-' || c == '_' {
                '-'
            } else {
                ' '
            }
        })
        .filter(|c| *c != ' ')
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Check if URL is external
fn is_external_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// Escape HTML special characters
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// =============================================================================
// AST conversion helpers
// =============================================================================

/// Set tag info on Lua event table for Start events
fn set_tag_info(lua: &Lua, event_table: &Table, tag: &Tag<'_>) -> Result<()> {
    match tag {
        Tag::Paragraph => {
            event_table.set("tag", "paragraph")?;
        }
        Tag::Heading {
            level,
            id,
            classes,
            attrs: _,
        } => {
            event_table.set("tag", "heading")?;
            let level_num = match level {
                HeadingLevel::H1 => 1,
                HeadingLevel::H2 => 2,
                HeadingLevel::H3 => 3,
                HeadingLevel::H4 => 4,
                HeadingLevel::H5 => 5,
                HeadingLevel::H6 => 6,
            };
            event_table.set("level", level_num)?;
            if let Some(id) = id {
                event_table.set("id", id.to_string())?;
            }
            if !classes.is_empty() {
                let classes_table = lua.create_table()?;
                for (i, class) in classes.iter().enumerate() {
                    classes_table.set(i + 1, class.to_string())?;
                }
                event_table.set("classes", classes_table)?;
            }
        }
        Tag::List(start) => {
            event_table.set("tag", "list")?;
            if let Some(n) = start {
                event_table.set("ordered", true)?;
                event_table.set("start", *n)?;
            } else {
                event_table.set("ordered", false)?;
            }
        }
        Tag::Item => {
            event_table.set("tag", "item")?;
        }
        Tag::BlockQuote(_) => {
            event_table.set("tag", "blockquote")?;
        }
        Tag::CodeBlock(kind) => {
            event_table.set("tag", "code_block")?;
            match kind {
                pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                    event_table.set("fenced", true)?;
                    if !lang.is_empty() {
                        event_table.set("language", lang.to_string())?;
                    }
                }
                pulldown_cmark::CodeBlockKind::Indented => {
                    event_table.set("fenced", false)?;
                }
            }
        }
        Tag::Link {
            link_type: _,
            dest_url,
            title,
            id,
        } => {
            event_table.set("tag", "link")?;
            event_table.set("url", dest_url.to_string())?;
            if !title.is_empty() {
                event_table.set("title", title.to_string())?;
            }
            if !id.is_empty() {
                event_table.set("id", id.to_string())?;
            }
        }
        Tag::Image {
            link_type: _,
            dest_url,
            title,
            id,
        } => {
            event_table.set("tag", "image")?;
            event_table.set("url", dest_url.to_string())?;
            if !title.is_empty() {
                event_table.set("title", title.to_string())?;
            }
            if !id.is_empty() {
                event_table.set("id", id.to_string())?;
            }
        }
        Tag::Emphasis => {
            event_table.set("tag", "emphasis")?;
        }
        Tag::Strong => {
            event_table.set("tag", "strong")?;
        }
        Tag::Strikethrough => {
            event_table.set("tag", "strikethrough")?;
        }
        Tag::Table(alignments) => {
            event_table.set("tag", "table")?;
            let align_table = lua.create_table()?;
            for (i, align) in alignments.iter().enumerate() {
                let align_str = match align {
                    Alignment::None => "none",
                    Alignment::Left => "left",
                    Alignment::Center => "center",
                    Alignment::Right => "right",
                };
                align_table.set(i + 1, align_str)?;
            }
            event_table.set("alignments", align_table)?;
        }
        Tag::TableHead => {
            event_table.set("tag", "table_head")?;
        }
        Tag::TableRow => {
            event_table.set("tag", "table_row")?;
        }
        Tag::TableCell => {
            event_table.set("tag", "table_cell")?;
        }
        Tag::FootnoteDefinition(label) => {
            event_table.set("tag", "footnote_definition")?;
            event_table.set("label", label.to_string())?;
        }
        Tag::HtmlBlock => {
            event_table.set("tag", "html_block")?;
        }
        Tag::MetadataBlock(_) => {
            event_table.set("tag", "metadata_block")?;
        }
        Tag::DefinitionList => {
            event_table.set("tag", "definition_list")?;
        }
        Tag::DefinitionListTitle => {
            event_table.set("tag", "definition_list_title")?;
        }
        Tag::DefinitionListDefinition => {
            event_table.set("tag", "definition_list_definition")?;
        }
        Tag::Superscript => {
            event_table.set("tag", "superscript")?;
        }
        Tag::Subscript => {
            event_table.set("tag", "subscript")?;
        }
    }
    Ok(())
}

/// Set tag info on Lua event table for End events
fn set_tag_end_info(event_table: &Table, tag_end: &TagEnd) -> Result<()> {
    let tag_name = match tag_end {
        TagEnd::Paragraph => "paragraph",
        TagEnd::Heading(level) => {
            event_table.set("level", *level as u8)?;
            "heading"
        }
        TagEnd::List(ordered) => {
            event_table.set("ordered", *ordered)?;
            "list"
        }
        TagEnd::Item => "item",
        TagEnd::BlockQuote(_) => "blockquote",
        TagEnd::CodeBlock => "code_block",
        TagEnd::Link => "link",
        TagEnd::Image => "image",
        TagEnd::Emphasis => "emphasis",
        TagEnd::Strong => "strong",
        TagEnd::Strikethrough => "strikethrough",
        TagEnd::Table => "table",
        TagEnd::TableHead => "table_head",
        TagEnd::TableRow => "table_row",
        TagEnd::TableCell => "table_cell",
        TagEnd::FootnoteDefinition => "footnote_definition",
        TagEnd::HtmlBlock => "html_block",
        TagEnd::MetadataBlock(_) => "metadata_block",
        TagEnd::DefinitionList => "definition_list",
        TagEnd::DefinitionListTitle => "definition_list_title",
        TagEnd::DefinitionListDefinition => "definition_list_definition",
        TagEnd::Superscript => "superscript",
        TagEnd::Subscript => "subscript",
    };
    event_table.set("tag", tag_name)?;
    Ok(())
}

/// Convert a Lua table back to a pulldown_cmark Event
fn lua_table_to_event(_lua: &Lua, table: &Table) -> Result<Option<Event<'static>>> {
    let event_type: String = table.get("type")?;

    match event_type.as_str() {
        "text" => {
            let content: String = table.get("content").unwrap_or_default();
            Ok(Some(Event::Text(CowStr::Boxed(content.into_boxed_str()))))
        }
        "html" => {
            let content: String = table.get("content").unwrap_or_default();
            Ok(Some(Event::Html(CowStr::Boxed(content.into_boxed_str()))))
        }
        "code" => {
            let content: String = table.get("content").unwrap_or_default();
            Ok(Some(Event::Code(CowStr::Boxed(content.into_boxed_str()))))
        }
        "softbreak" => Ok(Some(Event::SoftBreak)),
        "hardbreak" => Ok(Some(Event::HardBreak)),
        "rule" => Ok(Some(Event::Rule)),
        "start" => {
            let tag_name: String = table.get("tag").unwrap_or_default();
            let tag = match tag_name.as_str() {
                "paragraph" => Tag::Paragraph,
                "heading" => {
                    let level: u8 = table.get("level").unwrap_or(1);
                    let level = match level {
                        1 => HeadingLevel::H1,
                        2 => HeadingLevel::H2,
                        3 => HeadingLevel::H3,
                        4 => HeadingLevel::H4,
                        5 => HeadingLevel::H5,
                        _ => HeadingLevel::H6,
                    };
                    let id: Option<String> = table.get("id").ok();
                    Tag::Heading {
                        level,
                        id: id.map(|s| CowStr::Boxed(s.into_boxed_str())),
                        classes: vec![],
                        attrs: vec![],
                    }
                }
                "list" => {
                    let ordered: bool = table.get("ordered").unwrap_or(false);
                    if ordered {
                        let start: u64 = table.get("start").unwrap_or(1);
                        Tag::List(Some(start))
                    } else {
                        Tag::List(None)
                    }
                }
                "item" => Tag::Item,
                "blockquote" => Tag::BlockQuote(None),
                "code_block" => {
                    let fenced: bool = table.get("fenced").unwrap_or(true);
                    if fenced {
                        let lang: String = table.get("language").unwrap_or_default();
                        Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(CowStr::Boxed(
                            lang.into_boxed_str(),
                        )))
                    } else {
                        Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Indented)
                    }
                }
                "link" => {
                    let url: String = table.get("url").unwrap_or_default();
                    let title: String = table.get("title").unwrap_or_default();
                    Tag::Link {
                        link_type: pulldown_cmark::LinkType::Inline,
                        dest_url: CowStr::Boxed(url.into_boxed_str()),
                        title: CowStr::Boxed(title.into_boxed_str()),
                        id: CowStr::Borrowed(""),
                    }
                }
                "image" => {
                    let url: String = table.get("url").unwrap_or_default();
                    let title: String = table.get("title").unwrap_or_default();
                    Tag::Image {
                        link_type: pulldown_cmark::LinkType::Inline,
                        dest_url: CowStr::Boxed(url.into_boxed_str()),
                        title: CowStr::Boxed(title.into_boxed_str()),
                        id: CowStr::Borrowed(""),
                    }
                }
                "emphasis" => Tag::Emphasis,
                "strong" => Tag::Strong,
                "strikethrough" => Tag::Strikethrough,
                "table" => Tag::Table(vec![]),
                "table_head" => Tag::TableHead,
                "table_row" => Tag::TableRow,
                "table_cell" => Tag::TableCell,
                _ => return Ok(None),
            };
            Ok(Some(Event::Start(tag)))
        }
        "end" => {
            let tag_name: String = table.get("tag").unwrap_or_default();
            let tag_end = match tag_name.as_str() {
                "paragraph" => TagEnd::Paragraph,
                "heading" => {
                    let level: u8 = table.get("level").unwrap_or(1);
                    let level = match level {
                        1 => HeadingLevel::H1,
                        2 => HeadingLevel::H2,
                        3 => HeadingLevel::H3,
                        4 => HeadingLevel::H4,
                        5 => HeadingLevel::H5,
                        _ => HeadingLevel::H6,
                    };
                    TagEnd::Heading(level)
                }
                "list" => {
                    let ordered: bool = table.get("ordered").unwrap_or(false);
                    TagEnd::List(ordered)
                }
                "item" => TagEnd::Item,
                "blockquote" => TagEnd::BlockQuote(None),
                "code_block" => TagEnd::CodeBlock,
                "link" => TagEnd::Link,
                "image" => TagEnd::Image,
                "emphasis" => TagEnd::Emphasis,
                "strong" => TagEnd::Strong,
                "strikethrough" => TagEnd::Strikethrough,
                "table" => TagEnd::Table,
                "table_head" => TagEnd::TableHead,
                "table_row" => TagEnd::TableRow,
                "table_cell" => TagEnd::TableCell,
                _ => return Ok(None),
            };
            Ok(Some(Event::End(tag_end)))
        }
        "footnote_ref" => {
            let label: String = table.get("label").unwrap_or_default();
            Ok(Some(Event::FootnoteReference(CowStr::Boxed(
                label.into_boxed_str(),
            ))))
        }
        "task_marker" => {
            let checked: bool = table.get("checked").unwrap_or(false);
            Ok(Some(Event::TaskListMarker(checked)))
        }
        _ => Ok(None),
    }
}
