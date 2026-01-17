//! Markdown context tracking for AST transformation

use mlua::{Lua, Result, Table};

/// Context tracking for markdown transformation
#[derive(Default)]
pub struct MarkdownContext {
    pub in_paragraph: bool,
    pub in_heading: bool,
    pub in_list: bool,
    pub in_list_item: bool,
    pub in_blockquote: bool,
    pub in_link: bool,
    pub in_emphasis: bool,
    pub in_strong: bool,
    pub in_code_block: bool,
    pub in_table: bool,
    pub heading_level: u8,
    pub list_depth: u32,
}

impl MarkdownContext {
    pub fn update_for_event(&mut self, event: &pulldown_cmark::Event<'_>) {
        use pulldown_cmark::{Event, HeadingLevel, Tag};
        if let Event::Start(tag) = event {
            match tag {
                Tag::Paragraph => self.in_paragraph = true,
                Tag::Heading { level, .. } => {
                    self.in_heading = true;
                    self.heading_level = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                }
                Tag::List(_) => {
                    self.in_list = true;
                    self.list_depth += 1;
                }
                Tag::Item => self.in_list_item = true,
                Tag::BlockQuote(_) => self.in_blockquote = true,
                Tag::Link { .. } => self.in_link = true,
                Tag::Emphasis => self.in_emphasis = true,
                Tag::Strong => self.in_strong = true,
                Tag::CodeBlock(_) => self.in_code_block = true,
                Tag::Table(_) => self.in_table = true,
                _ => {}
            }
        }
    }

    pub fn update_after_event(&mut self, event: &pulldown_cmark::Event<'_>) {
        use pulldown_cmark::{Event, TagEnd};
        if let Event::End(tag_end) = event {
            match tag_end {
                TagEnd::Paragraph => self.in_paragraph = false,
                TagEnd::Heading(_) => {
                    self.in_heading = false;
                    self.heading_level = 0;
                }
                TagEnd::List(_) => {
                    self.list_depth = self.list_depth.saturating_sub(1);
                    if self.list_depth == 0 {
                        self.in_list = false;
                    }
                }
                TagEnd::Item => self.in_list_item = false,
                TagEnd::BlockQuote(_) => self.in_blockquote = false,
                TagEnd::Link => self.in_link = false,
                TagEnd::Emphasis => self.in_emphasis = false,
                TagEnd::Strong => self.in_strong = false,
                TagEnd::CodeBlock => self.in_code_block = false,
                TagEnd::Table => self.in_table = false,
                _ => {}
            }
        }
    }

    pub fn to_lua_table(&self, lua: &Lua) -> Result<Table> {
        let t = lua.create_table()?;
        t.set("in_paragraph", self.in_paragraph)?;
        t.set("in_heading", self.in_heading)?;
        t.set("in_list", self.in_list)?;
        t.set("in_list_item", self.in_list_item)?;
        t.set("in_blockquote", self.in_blockquote)?;
        t.set("in_link", self.in_link)?;
        t.set("in_emphasis", self.in_emphasis)?;
        t.set("in_strong", self.in_strong)?;
        t.set("in_code_block", self.in_code_block)?;
        t.set("in_table", self.in_table)?;
        t.set("heading_level", self.heading_level)?;
        t.set("list_depth", self.list_depth)?;
        Ok(t)
    }
}

/// Set tag info on Lua event table for Start events
pub fn set_tag_info(lua: &Lua, event_table: &Table, tag: &pulldown_cmark::Tag<'_>) -> Result<()> {
    use pulldown_cmark::{Alignment, HeadingLevel, Tag};

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
pub fn set_tag_end_info(event_table: &Table, tag_end: &pulldown_cmark::TagEnd) -> Result<()> {
    use pulldown_cmark::TagEnd;

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
pub fn lua_table_to_event(
    _lua: &Lua,
    table: &Table,
) -> Result<Option<pulldown_cmark::Event<'static>>> {
    use pulldown_cmark::{CowStr, Event, HeadingLevel, Tag, TagEnd};

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
