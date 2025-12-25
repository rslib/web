use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

use super::AstTransform;
use crate::markdown::TransformContext;

/// Transform that adds rel="noopener" and target="_blank" to external links
pub struct ExternalLinksTransform;

impl AstTransform for ExternalLinksTransform {
    fn name(&self) -> &'static str {
        "external_links"
    }

    fn priority(&self) -> i32 {
        70
    }

    fn transform<'a>(&self, events: Vec<Event<'a>>, _ctx: &TransformContext<'_>) -> Vec<Event<'a>> {
        let mut result = Vec::with_capacity(events.len());
        let mut in_external_link = false;

        for event in events {
            match &event {
                Event::Start(Tag::Link {
                    dest_url, title, ..
                }) => {
                    if is_external_url(dest_url) {
                        in_external_link = true;
                        // Emit custom HTML for external link start
                        let html = format!(
                            r#"<a href="{}" target="_blank" rel="noopener noreferrer"{}>"#,
                            html_escape(dest_url),
                            if title.is_empty() {
                                String::new()
                            } else {
                                format!(r#" title="{}""#, html_escape(title))
                            }
                        );
                        result.push(Event::Html(CowStr::from(html)));
                    } else {
                        result.push(event);
                    }
                }
                Event::End(TagEnd::Link) if in_external_link => {
                    in_external_link = false;
                    result.push(Event::Html(CowStr::from("</a>")));
                }
                _ => {
                    result.push(event);
                }
            }
        }

        result
    }
}

fn is_external_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
