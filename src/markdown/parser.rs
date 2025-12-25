use pulldown_cmark::{Event, Options, Parser};

/// Parse markdown content into a vector of events
pub fn parse_markdown(content: &str) -> Vec<Event<'_>> {
    let options = Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_HEADING_ATTRIBUTES;

    let parser = Parser::new_ext(content, options);
    parser.collect()
}

/// Render events to HTML
pub fn events_to_html<'a>(events: impl Iterator<Item = Event<'a>>) -> String {
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, events);
    html_output
}
