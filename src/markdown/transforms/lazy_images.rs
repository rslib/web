use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

use super::AstTransform;
use crate::markdown::TransformContext;

/// Transform that adds lazy loading attributes to images
pub struct LazyImagesTransform;

impl AstTransform for LazyImagesTransform {
    fn name(&self) -> &'static str {
        "lazy_images"
    }

    fn priority(&self) -> i32 {
        50
    }

    fn transform<'a>(&self, events: Vec<Event<'a>>, _ctx: &TransformContext<'_>) -> Vec<Event<'a>> {
        let mut result = Vec::with_capacity(events.len() + 10);
        let mut in_image = false;
        let mut image_info: Option<(CowStr<'a>, CowStr<'a>, CowStr<'a>)> = None;

        for event in events {
            match &event {
                Event::Start(Tag::Image {
                    link_type,
                    dest_url,
                    title,
                    id,
                }) => {
                    in_image = true;
                    image_info = Some((dest_url.clone(), title.clone(), id.clone()));
                    // We'll emit a custom HTML instead
                    let _ = link_type; // Suppress unused warning
                }
                Event::End(TagEnd::Image) if in_image => {
                    in_image = false;
                    if let Some((dest_url, title, _id)) = image_info.take() {
                        // Convert to WebP path if it's a local image
                        let src = if !dest_url.starts_with("http") && !dest_url.ends_with(".webp") {
                            // Replace extension with .webp for local images
                            if let Some(pos) = dest_url.rfind('.') {
                                format!("{}.webp", &dest_url[..pos])
                            } else {
                                dest_url.to_string()
                            }
                        } else {
                            dest_url.to_string()
                        };

                        let html = format!(
                            r#"<img src="{}" alt="{}" loading="lazy" decoding="async">"#,
                            src,
                            html_escape(&title)
                        );
                        result.push(Event::Html(CowStr::from(html)));
                    }
                }
                Event::Text(text) if in_image => {
                    // This is the alt text, update image_info
                    if let Some((dest, _title, id)) = image_info.take() {
                        image_info = Some((dest, text.clone(), id));
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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
