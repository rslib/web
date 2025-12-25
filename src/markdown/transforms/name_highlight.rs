use pulldown_cmark::{CowStr, Event};

use super::AstTransform;
use crate::config::HighlightConfig;
use crate::markdown::TransformContext;

/// Transform that highlights configured names with a CSS class
pub struct NameHighlightTransform {
    config: HighlightConfig,
}

impl NameHighlightTransform {
    pub fn new(config: HighlightConfig) -> Self {
        Self { config }
    }
}

impl AstTransform for NameHighlightTransform {
    fn name(&self) -> &'static str {
        "name_highlight"
    }

    fn priority(&self) -> i32 {
        80 // Run before external links transform
    }

    fn transform<'a>(&self, events: Vec<Event<'a>>, _ctx: &TransformContext<'_>) -> Vec<Event<'a>> {
        if self.config.names.is_empty() {
            return events;
        }

        let mut result = Vec::with_capacity(events.len());

        for event in events {
            match &event {
                Event::Text(text) => {
                    let highlighted = self.highlight_names(text);
                    if highlighted != text.as_ref() {
                        result.push(Event::Html(CowStr::from(highlighted)));
                    } else {
                        result.push(event);
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

impl NameHighlightTransform {
    fn highlight_names(&self, text: &str) -> String {
        let mut result = text.to_string();

        for name in &self.config.names {
            if result.contains(name) {
                let replacement = format!(r#"<span class="{}">{}</span>"#, self.config.class, name);
                result = result.replace(name, &replacement);
            }
        }

        result
    }
}
