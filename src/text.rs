//! Plain text generation from HTML content
//!
//! Converts HTML posts to plain text format for curl-friendly output.
//! Produces well-formatted text with proper indentation, tables, and structure.

use regex::Regex;
use std::sync::LazyLock;

/// Regex patterns for HTML-to-text conversion
static TAG_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static WHITESPACE_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());
static LEADING_WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+").unwrap());

/// Convert HTML content to plain text with beautiful formatting
pub fn html_to_text(html: &str) -> String {
    let mut text = html.to_string();

    // Replace encrypted blocks with a friendly message
    // Match the entire encrypted-content div structure (outer div with nested decrypt-prompt div)
    let encrypted_re =
        Regex::new(r#"(?s)<div[^>]*class="[^"]*encrypted-content[^"]*"[^>]*>.*?</div>\s*</div>"#)
            .unwrap();
    text = encrypted_re
        .replace_all(
            &text,
            "\n    [Encrypted content - visit web version to decrypt]\n",
        )
        .to_string();

    // Also remove decrypt-prompt divs that might remain
    let decrypt_prompt_re =
        Regex::new(r#"(?s)<div[^>]*class="[^"]*decrypt-prompt[^"]*"[^>]*>.*?</div>"#).unwrap();
    text = decrypt_prompt_re.replace_all(&text, "").to_string();

    // Remove any remaining UI elements from encrypted blocks
    let encrypted_msg_re =
        Regex::new(r#"(?s)<p[^>]*class="[^"]*encrypted-message[^"]*"[^>]*>.*?</p>"#).unwrap();
    text = encrypted_msg_re.replace_all(&text, "").to_string();

    // Remove password inputs and decrypt buttons
    let input_re = Regex::new(r#"<input[^>]*>"#).unwrap();
    text = input_re.replace_all(&text, "").to_string();

    let button_re = Regex::new(r#"(?s)<button[^>]*>.*?</button>"#).unwrap();
    text = button_re.replace_all(&text, "").to_string();

    let label_re = Regex::new(r#"(?s)<label[^>]*>.*?</label>"#).unwrap();
    text = label_re.replace_all(&text, "").to_string();

    // First pass: extract code blocks and tables, replace with placeholders
    // This protects their formatting from whitespace normalization
    let mut code_blocks: Vec<String> = Vec::new();
    let mut tables: Vec<String> = Vec::new();

    // Extract code blocks
    let pre_code_re = Regex::new(r"(?s)<pre[^>]*>\s*<code[^>]*>(.*?)</code>\s*</pre>").unwrap();
    text = pre_code_re
        .replace_all(&text, |caps: &regex::Captures| {
            let formatted = format_code_block(&caps[1]);
            let idx = code_blocks.len();
            code_blocks.push(formatted);
            format!("{{{{CODE_BLOCK_{}}}}}", idx)
        })
        .to_string();

    let pre_only_re = Regex::new(r"(?s)<pre[^>]*>(.*?)</pre>").unwrap();
    text = pre_only_re
        .replace_all(&text, |caps: &regex::Captures| {
            let formatted = format_code_block(&caps[1]);
            let idx = code_blocks.len();
            code_blocks.push(formatted);
            format!("{{{{CODE_BLOCK_{}}}}}", idx)
        })
        .to_string();

    // Extract tables
    let table_re = Regex::new(r"(?s)<table[^>]*>(.*?)</table>").unwrap();
    text = table_re
        .replace_all(&text, |caps: &regex::Captures| {
            let formatted = format_table(&caps[1]);
            let idx = tables.len();
            tables.push(formatted);
            format!("{{{{TABLE_{}}}}}", idx)
        })
        .to_string();

    // Process headings with decorative markers
    text = process_headings(&text);

    // Process blockquotes with indentation
    text = process_blockquotes(&text);

    // Process inline code (not pre blocks)
    let code_re = Regex::new(r"<code[^>]*>(.*?)</code>").unwrap();
    text = code_re
        .replace_all(&text, |caps: &regex::Captures| {
            let content = decode_html_entities(&caps[1]);
            let content = strip_tags(&content);
            format!("`{}`", content.trim())
        })
        .to_string();

    // Process lists with proper indentation
    text = process_lists(&text);

    // Process links - show URL in brackets
    text = process_links(&text);

    // Replace block elements with newlines before stripping tags
    text = text.replace("<br>", "\n");
    text = text.replace("<br/>", "\n");
    text = text.replace("<br />", "\n");
    text = text.replace("</p>", "\n\n");
    text = text.replace("</div>", "\n");

    // Remove script and style content entirely
    let script_re = Regex::new(r"(?s)<script[^>]*>.*?</script>").unwrap();
    let style_re = Regex::new(r"(?s)<style[^>]*>.*?</style>").unwrap();
    text = script_re.replace_all(&text, "").to_string();
    text = style_re.replace_all(&text, "").to_string();

    // Strip all remaining HTML tags
    text = TAG_PATTERN.replace_all(&text, "").to_string();

    // Decode common HTML entities
    text = decode_html_entities(&text);

    // Normalize whitespace (this won't affect our placeholders)
    text = WHITESPACE_PATTERN.replace_all(&text, "\n\n").to_string();
    text = LEADING_WHITESPACE.replace(&text, "").to_string();

    // Clean up multiple spaces within lines, but preserve leading indentation
    let lines: Vec<&str> = text.lines().collect();
    let multi_space = Regex::new(r" {2,}").expect("invalid regex");
    let mut cleaned_lines: Vec<String> = Vec::new();
    for line in lines {
        // Find where leading whitespace ends
        let leading_ws = line.len() - line.trim_start().len();
        let (leading, rest) = line.split_at(leading_ws);
        // Only collapse multiple spaces in the non-leading part
        let cleaned_rest = multi_space.replace_all(rest, " ");
        cleaned_lines.push(format!("{}{}", leading, cleaned_rest));
    }
    text = cleaned_lines.join("\n");

    // Now restore code blocks and tables with their proper formatting
    for (idx, block) in code_blocks.iter().enumerate() {
        text = text.replace(&format!("{{{{CODE_BLOCK_{}}}}}", idx), block);
    }
    for (idx, table) in tables.iter().enumerate() {
        text = text.replace(&format!("{{{{TABLE_{}}}}}", idx), table);
    }

    text.trim().to_string()
}

/// Process headings into visually distinct text format
fn process_headings(html: &str) -> String {
    let mut text = html.to_string();

    // H1: Title with double underline (ASCII)
    let h1_re = Regex::new(r"(?s)<h1[^>]*>(.*?)</h1>").unwrap();
    text = h1_re
        .replace_all(&text, |caps: &regex::Captures| {
            let content = strip_tags(&caps[1]);
            let underline = "=".repeat(content.len().min(72));
            format!("\n\n{}\n{}\n\n", content.to_uppercase(), underline)
        })
        .to_string();

    // H2: Section with single underline (ASCII)
    let h2_re = Regex::new(r"(?s)<h2[^>]*>(.*?)</h2>").unwrap();
    text = h2_re
        .replace_all(&text, |caps: &regex::Captures| {
            let content = strip_tags(&caps[1]);
            let underline = "-".repeat(content.len().min(72));
            format!("\n\n{}\n{}\n\n", content, underline)
        })
        .to_string();

    // H3: With marker prefix
    let h3_re = Regex::new(r"(?s)<h3[^>]*>(.*?)</h3>").unwrap();
    text = h3_re
        .replace_all(&text, |caps: &regex::Captures| {
            let content = strip_tags(&caps[1]);
            format!("\n\n## {}\n\n", content)
        })
        .to_string();

    // H4-H6: With increasing marker prefix
    for (level, marker) in [(4, "###"), (5, "####"), (6, "#####")] {
        let re = Regex::new(&format!(r"(?s)<h{}[^>]*>(.*?)</h{}>", level, level)).unwrap();
        text = re
            .replace_all(&text, |caps: &regex::Captures| {
                let content = strip_tags(&caps[1]);
                format!("\n\n{} {}\n\n", marker, content)
            })
            .to_string();
    }

    text
}

/// Process blockquotes with indentation (ASCII)
fn process_blockquotes(html: &str) -> String {
    let mut text = html.to_string();

    let blockquote_re = Regex::new(r"(?s)<blockquote[^>]*>(.*?)</blockquote>").unwrap();
    text = blockquote_re
        .replace_all(&text, |caps: &regex::Captures| {
            let content = strip_tags(&caps[1]);
            let indented: Vec<String> = content
                .lines()
                .map(|line| {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        "    |".to_string()
                    } else {
                        format!("    | {}", trimmed)
                    }
                })
                .collect();
            format!("\n{}\n", indented.join("\n"))
        })
        .to_string();

    text
}

/// Format a code block with consistent styling (ASCII)
fn format_code_block(html_content: &str) -> String {
    let content = decode_html_entities(html_content);
    let content = strip_tags(&content);

    // Process lines: expand tabs, normalize to ASCII, preserve indentation
    let lines: Vec<String> = content
        .lines()
        .map(|l| {
            let expanded = l.replace('\t', "    ");
            // Convert non-ASCII to spaces, keep ASCII printable chars
            let cleaned: String = expanded
                .chars()
                .map(|c| {
                    if c.is_ascii_graphic() || c == ' ' {
                        c
                    } else {
                        ' '
                    }
                })
                .collect();
            cleaned.trim_end().to_string()
        })
        .collect();

    // Skip empty leading/trailing lines
    let start = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);
    let end = lines
        .iter()
        .rposition(|l| !l.is_empty())
        .map(|i| i + 1)
        .unwrap_or(lines.len());
    let lines = &lines[start..end];

    if lines.is_empty() {
        return String::new();
    }

    // Calculate max width - use len() since we're now pure ASCII
    let max_width = lines.iter().map(|l| l.len()).max().unwrap_or(0).max(20);

    // Build code block with borders (ASCII)
    let mut result = String::new();
    let border_width = max_width + 2;

    result.push_str(&format!("\n    +{}+\n", "-".repeat(border_width)));
    for line in lines {
        let padding = max_width.saturating_sub(line.len());
        result.push_str(&format!("    | {}{} |\n", line, " ".repeat(padding)));
    }
    result.push_str(&format!("    +{}+\n", "-".repeat(border_width)));
    result
}

/// Clean cell content - normalize to ASCII-safe text
fn clean_cell(content: &str) -> String {
    let stripped = strip_tags(content);
    let decoded = decode_html_entities(&stripped);
    // Keep only ASCII printable characters and collapse whitespace
    decoded
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() || c == ' ' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format a single table as simple aligned text (ASCII)
fn format_table(table_html: &str) -> String {
    let row_re = Regex::new(r"(?s)<tr[^>]*>(.*?)</tr>").unwrap();
    let th_re = Regex::new(r"(?s)<th[^>]*>(.*?)</th>").unwrap();
    let td_re = Regex::new(r"(?s)<td[^>]*>(.*?)</td>").unwrap();

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut header_row_idx: Option<usize> = None;

    for row_cap in row_re.captures_iter(table_html) {
        let row_content = &row_cap[1];
        let mut cells: Vec<String> = Vec::new();
        let mut is_header = false;

        // Check for header cells first
        for th_cap in th_re.captures_iter(row_content) {
            cells.push(clean_cell(&th_cap[1]));
            is_header = true;
        }

        // Then regular cells
        for td_cap in td_re.captures_iter(row_content) {
            cells.push(clean_cell(&td_cap[1]));
        }

        if !cells.is_empty() {
            if is_header && header_row_idx.is_none() {
                header_row_idx = Some(rows.len());
            }
            rows.push(cells);
        }
    }

    if rows.is_empty() {
        return String::new();
    }

    // Calculate column widths - use len() since cells are now ASCII
    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut col_widths: Vec<usize> = vec![0; num_cols];

    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(cell.len());
            }
        }
    }

    // Build the table (ASCII)
    let mut result = String::new();
    result.push('\n');

    for (row_idx, row) in rows.iter().enumerate() {
        result.push_str("    ");
        for (i, &width) in col_widths.iter().enumerate() {
            let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
            let padding = width.saturating_sub(cell.len());
            result.push_str(cell);
            result.push_str(&" ".repeat(padding));
            if i < num_cols - 1 {
                result.push_str(" | ");
            }
        }
        result.push('\n');

        // Header separator (ASCII)
        if Some(row_idx) == header_row_idx && row_idx < rows.len() - 1 {
            result.push_str("    ");
            for (i, &width) in col_widths.iter().enumerate() {
                result.push_str(&"-".repeat(width));
                if i < num_cols - 1 {
                    result.push_str("-+-");
                }
            }
            result.push('\n');
        }
    }

    result.push('\n');
    result
}

/// Process lists with proper indentation and markers (supports nesting)
fn process_lists(html: &str) -> String {
    let mut result = String::new();
    let mut pos = 0;

    while pos < html.len() {
        // Look for list start
        if html[pos..].starts_with("<ul") || html[pos..].starts_with("<ol") {
            let is_ordered = html[pos..].starts_with("<ol");
            let list_text = process_list_recursive(&html[pos..], is_ordered, 0);
            result.push('\n');
            result.push_str(&list_text.0);
            result.push('\n');
            pos += list_text.1;
        } else {
            // Copy character as-is
            if let Some(c) = html[pos..].chars().next() {
                result.push(c);
                pos += c.len_utf8();
            } else {
                break;
            }
        }
    }

    result
}

/// Recursively process a list, returning (formatted_text, bytes_consumed)
fn process_list_recursive(html: &str, is_ordered: bool, depth: usize) -> (String, usize) {
    let mut result = String::new();
    let indent = "  ".repeat(depth);
    let bullet_indent = "  ".repeat(depth + 1);

    // Find the opening tag end
    let tag_end = match html.find('>') {
        Some(i) => i + 1,
        None => return (String::new(), html.len()),
    };

    let close_tag = if is_ordered { "</ol>" } else { "</ul>" };
    let mut pos = tag_end;
    let mut item_num = 1;

    while pos < html.len() {
        // Check for list end
        if html[pos..].starts_with(close_tag) {
            pos += close_tag.len();
            break;
        }

        // Check for list item start
        if html[pos..].starts_with("<li") {
            let li_end = match html[pos..].find('>') {
                Some(i) => pos + i + 1,
                None => break,
            };
            pos = li_end;

            // Collect item content until </li>, handling nested lists
            let mut item_content = String::new();
            let mut nested_lists: Vec<String> = Vec::new();

            while pos < html.len() && !html[pos..].starts_with("</li>") {
                // Check for nested list
                if html[pos..].starts_with("<ul") {
                    let nested = process_list_recursive(&html[pos..], false, depth + 1);
                    nested_lists.push(nested.0);
                    pos += nested.1;
                } else if html[pos..].starts_with("<ol") {
                    let nested = process_list_recursive(&html[pos..], true, depth + 1);
                    nested_lists.push(nested.0);
                    pos += nested.1;
                } else if let Some(c) = html[pos..].chars().next() {
                    item_content.push(c);
                    pos += c.len_utf8();
                } else {
                    break;
                }
            }

            // Skip </li>
            if html[pos..].starts_with("</li>") {
                pos += 5;
            }

            // Format the item text (strip tags, clean up)
            let item_text = strip_tags(&item_content).trim().to_string();
            if !item_text.is_empty() {
                let wrapped = wrap_text(&item_text, 68 - (depth * 2));
                for (j, line) in wrapped.lines().enumerate() {
                    if j == 0 {
                        if is_ordered {
                            result.push_str(&format!("{}{}. {}\n", indent, item_num, line));
                        } else {
                            result.push_str(&format!("{}- {}\n", bullet_indent, line));
                        }
                    } else {
                        let extra_indent = if is_ordered { "   " } else { "  " };
                        result.push_str(&format!("{}{}{}\n", bullet_indent, extra_indent, line));
                    }
                }
            }

            // Add nested lists
            for nested in nested_lists {
                result.push_str(&nested);
            }

            item_num += 1;
        } else if let Some(c) = html[pos..].chars().next() {
            // Skip whitespace between items
            pos += c.len_utf8();
        } else {
            break;
        }
    }

    (result, pos)
}

/// Process links to show URL
fn process_links(html: &str) -> String {
    let link_re = Regex::new(r#"<a[^>]*href="([^"]*)"[^>]*>(.*?)</a>"#).unwrap();
    link_re
        .replace_all(html, |caps: &regex::Captures| {
            let url = &caps[1];
            let text = strip_tags(&caps[2]);
            if url.starts_with('#') || url == text {
                text.to_string()
            } else {
                format!("{} [{}]", text, url)
            }
        })
        .to_string()
}

/// Strip HTML tags from a string
fn strip_tags(html: &str) -> String {
    TAG_PATTERN.replace_all(html, "").to_string()
}

/// Wrap text to a maximum width
fn wrap_text(text: &str, max_width: usize) -> String {
    let mut result = String::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            result.push_str(&current_line);
            result.push('\n');
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        result.push_str(&current_line);
    }

    result
}

/// Decode common HTML entities and normalize Unicode
fn decode_html_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace(
            [
                '\u{00A0}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200A}', '\u{202F}', '\u{205F}',
                '\u{3000}',
            ],
            " ",
        ) // Various Unicode spaces -> regular space
        .replace(['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}'], "") // Zero-width characters -> remove
        .replace("&middot;", ".")
        .replace("&bull;", "*")
        .replace("&mdash;", "-")
        .replace("&ndash;", "-")
        .replace("&hellip;", "...")
        .replace("&copy;", "(c)")
        .replace("&reg;", "(R)")
        .replace("&trade;", "(TM)")
        .replace("&#x27;", "'")
        .replace("&#x2F;", "/")
}

/// Clean up text output: trim trailing whitespace from lines, collapse multiple blank lines
fn cleanup_text(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(|line| line.trim_end()).collect();

    // Collapse multiple consecutive blank lines into at most 2
    let mut result = String::new();
    let mut blank_count = 0;

    for line in lines {
        if line.is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                result.push('\n');
            }
        } else {
            blank_count = 0;
            result.push_str(line);
            result.push('\n');
        }
    }

    result.trim_end().to_string()
}

/// Helper to create a boxed line with proper padding (ASCII)
fn box_line(content: &str, width: usize) -> String {
    let content_width = width - 2; // Account for | on each side
    let display_len = content.chars().count();
    let padding = content_width.saturating_sub(display_len);
    format!("|{}{}|\n", content, " ".repeat(padding))
}

/// Helper to create a centered boxed line (ASCII)
fn box_line_centered(content: &str, width: usize) -> String {
    let content_width = width - 2;
    let display_len = content.chars().count();
    let total_padding = content_width.saturating_sub(display_len);
    let left_pad = total_padding / 2;
    let right_pad = total_padding - left_pad;
    format!(
        "|{}{}{}|\n",
        " ".repeat(left_pad),
        content,
        " ".repeat(right_pad)
    )
}

/// Format a post as plain text with metadata header (ASCII)
#[allow(clippy::too_many_arguments)]
pub fn format_post_text(
    title: &str,
    date: Option<&str>,
    description: Option<&str>,
    tags: &[String],
    reading_time: u32,
    content: &str,
    url: &str,
    base_url: &str,
) -> String {
    let mut output = String::new();
    let width = 74; // Total width including borders

    // Top border (ASCII)
    output.push('+');
    output.push_str(&"=".repeat(width - 2));
    output.push_str("+\n");

    // Title (centered)
    let title_lines = wrap_text(title, width - 6);
    for line in title_lines.lines() {
        output.push_str(&box_line_centered(line, width));
    }

    // Separator (ASCII)
    output.push('+');
    output.push_str(&"=".repeat(width - 2));
    output.push_str("+\n");

    // Metadata
    if let Some(date) = date {
        output.push_str(&box_line(&format!("  Date: {}", date), width));
    }

    if !tags.is_empty() {
        let tags_str = tags.join(", ");
        // Wrap just the tags content, then add prefix
        let wrapped = wrap_text(&tags_str, width - 12); // Account for "  Tags: " prefix
        for (i, line) in wrapped.lines().enumerate() {
            if i == 0 {
                output.push_str(&box_line(&format!("  Tags: {}", line), width));
            } else {
                output.push_str(&box_line(&format!("        {}", line), width)); // Align with content
            }
        }
    }

    output.push_str(&box_line(
        &format!("  Reading time: {} min", reading_time),
        width,
    ));

    // Handle long URLs by wrapping if needed
    let full_url = format!("{}{}", base_url, url);
    let url_line = format!("  URL: {}", full_url);
    if url_line.chars().count() <= width - 4 {
        output.push_str(&box_line(&url_line, width));
    } else {
        output.push_str(&box_line("  URL:", width));
        output.push_str(&box_line(&format!("    {}", full_url), width));
    }

    if let Some(desc) = description {
        output.push('+');
        output.push_str(&"-".repeat(width - 2));
        output.push_str("+\n");
        let wrapped = wrap_text(desc, width - 6);
        for line in wrapped.lines() {
            output.push_str(&box_line(&format!("  {}", line), width));
        }
    }

    // Bottom border of header (ASCII)
    output.push('+');
    output.push_str(&"=".repeat(width - 2));
    output.push_str("+\n\n");

    // Content
    output.push_str(&html_to_text(content));
    output.push_str("\n\n");

    // Footer (ASCII)
    output.push_str(&"-".repeat(width));
    output.push('\n');

    cleanup_text(&output)
}

/// Format home page as plain text (ASCII)
pub fn format_home_text(title: &str, description: &str, content: &str, base_url: &str) -> String {
    let mut output = String::new();
    let width = 74;

    // Top border (ASCII)
    output.push('+');
    output.push_str(&"=".repeat(width - 2));
    output.push_str("+\n");

    // Title (centered)
    output.push_str(&box_line_centered(title, width));

    // Separator (ASCII)
    output.push('+');
    output.push_str(&"=".repeat(width - 2));
    output.push_str("+\n");

    // Description
    let wrapped = wrap_text(description, width - 6);
    for line in wrapped.lines() {
        output.push_str(&box_line(&format!("  {}", line), width));
    }

    output.push_str(&box_line(&format!("  URL: {}", base_url), width));

    // Bottom border of header (ASCII)
    output.push('+');
    output.push_str(&"=".repeat(width - 2));
    output.push_str("+\n\n");

    // Content
    output.push_str(&html_to_text(content));
    output.push_str("\n\n");

    // Footer (ASCII)
    output.push_str(&"-".repeat(width));
    output.push('\n');

    cleanup_text(&output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_text_basic() {
        let html = "<p>Hello, world!</p>";
        assert!(html_to_text(html).contains("Hello, world!"));
    }

    #[test]
    fn test_html_to_text_with_entities() {
        let html = "<p>Hello &amp; goodbye &mdash; see you!</p>";
        let text = html_to_text(html);
        assert!(text.contains("Hello & goodbye - see you!")); // mdash -> ASCII dash
    }

    #[test]
    fn test_html_to_text_list() {
        let html = "<ul><li>First</li><li>Second</li></ul>";
        let text = html_to_text(html);
        assert!(text.contains("- First"));
        assert!(text.contains("- Second"));
    }

    #[test]
    fn test_html_to_text_strips_scripts() {
        let html = "<p>Before</p><script>alert('bad');</script><p>After</p>";
        let text = html_to_text(html);
        assert!(!text.contains("script"));
        assert!(!text.contains("alert"));
        assert!(text.contains("Before"));
        assert!(text.contains("After"));
    }

    #[test]
    fn test_html_to_text_headings() {
        let html = "<h1>Main Title</h1><h2>Section</h2><h3>Subsection</h3>";
        let text = html_to_text(html);
        assert!(text.contains("MAIN TITLE"));
        assert!(text.contains("="));
        assert!(text.contains("Section"));
        assert!(text.contains("-"));
        assert!(text.contains("## Subsection"));
    }

    #[test]
    fn test_html_to_text_table() {
        let html =
            "<table><tr><th>Name</th><th>Age</th></tr><tr><td>Alice</td><td>30</td></tr></table>";
        let text = html_to_text(html);
        assert!(text.contains("Name"));
        assert!(text.contains("Age"));
        assert!(text.contains("Alice"));
        assert!(text.contains("30"));
        assert!(text.contains("|")); // Column separator
        assert!(text.contains("-")); // Header separator
    }

    #[test]
    fn test_html_to_text_code_block() {
        let html = "<pre><code>fn main() {\n    println!(\"Hello\");\n}</code></pre>";
        let text = html_to_text(html);
        assert!(text.contains("fn main()"));
        assert!(text.contains("println!"));
        assert!(text.contains("+"));
    }

    #[test]
    fn test_html_to_text_blockquote() {
        let html = "<blockquote>This is a quote</blockquote>";
        let text = html_to_text(html);
        assert!(text.contains("|"));
        assert!(text.contains("This is a quote"));
    }

    #[test]
    fn test_format_post_text() {
        let text = format_post_text(
            "Test Post",
            Some("2024-01-15"),
            Some("A test description"),
            &["rust".to_string(), "web".to_string()],
            5,
            "<p>Post content here.</p>",
            "/blog/test-post/",
            "https://example.com",
        );

        assert!(text.contains("Test Post"));
        assert!(text.contains("2024-01-15"));
        assert!(text.contains("rust, web"));
        assert!(text.contains("5 min"));
        assert!(text.contains("https://example.com/blog/test-post/"));
        assert!(text.contains("Post content here."));
        assert!(text.contains("+"));
        assert!(text.contains("="));
    }

    #[test]
    fn test_process_links() {
        let html = r#"<a href="https://example.com">Example</a>"#;
        let text = process_links(html);
        assert!(text.contains("Example [https://example.com]"));
    }
}
