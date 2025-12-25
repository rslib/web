use pulldown_cmark::{CowStr, Event, HeadingLevel, Tag, TagEnd};

use super::AstTransform;
use crate::markdown::TransformContext;

/// Transform that adds anchor IDs to headings
pub struct HeadingAnchorsTransform;

impl HeadingAnchorsTransform {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HeadingAnchorsTransform {
    fn default() -> Self {
        Self::new()
    }
}

impl AstTransform for HeadingAnchorsTransform {
    fn name(&self) -> &'static str {
        "heading_anchors"
    }

    fn priority(&self) -> i32 {
        60
    }

    fn transform<'a>(&self, events: Vec<Event<'a>>, _ctx: &TransformContext<'_>) -> Vec<Event<'a>> {
        let mut result = Vec::with_capacity(events.len() + 20);
        let mut in_heading = false;
        let mut heading_level: Option<HeadingLevel> = None;
        let mut heading_text = String::new();
        let mut heading_counts = std::collections::HashMap::new();

        for event in events {
            match &event {
                Event::Start(Tag::Heading { level, .. }) => {
                    in_heading = true;
                    heading_level = Some(*level);
                    heading_text.clear();
                    // Push the start event - we'll replace it later
                    result.push(event);
                }
                Event::Text(text) if in_heading => {
                    heading_text.push_str(text);
                    result.push(event);
                }
                Event::End(TagEnd::Heading(_)) if in_heading => {
                    in_heading = false;

                    // Generate slug
                    let base_slug = slugify(&heading_text);

                    // Handle duplicate slugs
                    let count = heading_counts.entry(base_slug.clone()).or_insert(0);
                    let slug = if *count > 0 {
                        format!("{}-{}", base_slug, count)
                    } else {
                        base_slug
                    };
                    *count += 1;

                    if let Some(level) = heading_level.take() {
                        // Find and replace the Start event with HTML that has an ID
                        let start_idx = result
                            .iter()
                            .rposition(|e| matches!(e, Event::Start(Tag::Heading { .. })));

                        if let Some(idx) = start_idx {
                            result[idx] = Event::Html(CowStr::from(format!(
                                r#"<h{} id="{}">"#,
                                heading_level_to_num(level),
                                slug
                            )));
                            result.push(Event::Html(CowStr::from(format!(
                                r#"</h{}>"#,
                                heading_level_to_num(level)
                            ))));
                        } else {
                            result.push(event);
                        }
                    }
                }
                _ => {
                    result.push(event);
                }
            }
        }

        result
    }
}

fn slugify(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c.is_whitespace() || c == '-' || c == '_' {
                '-'
            } else {
                ' ' // Will be filtered out
            }
        })
        .filter(|c| *c != ' ')
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn heading_level_to_num(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
