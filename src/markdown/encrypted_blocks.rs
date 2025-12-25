use once_cell::sync::Lazy;
use regex::Regex;

/// Represents an extracted encrypted block
#[derive(Debug, Clone)]
pub struct EncryptedBlock {
    /// The markdown content inside the block
    pub content: String,
    /// Unique identifier for this block
    pub id: usize,
    /// Optional per-block password (overrides global)
    pub password: Option<String>,
}

/// Result of pre-processing markdown for encrypted blocks
#[derive(Debug)]
pub struct PreprocessResult {
    /// The markdown with encrypted blocks replaced by placeholders
    pub markdown: String,
    /// The extracted encrypted blocks
    pub blocks: Vec<EncryptedBlock>,
}

/// Regex for matching :::encrypted blocks with optional password attribute (Markdown)
static ENCRYPTED_BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?ms)^:::encrypted(?:\s+password="([^"]*)")?\s*\n(.*?)\n:::(?:\s*$|\n)"#)
        .expect("Invalid encrypted block regex pattern")
});

/// Regex for matching <encrypted>...</encrypted> blocks (HTML)
static HTML_ENCRYPTED_BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?ms)<encrypted(?:\s+password="([^"]*)")?>(.*?)</encrypted>"#)
        .expect("Invalid HTML encrypted block regex pattern")
});

/// Extract `:::encrypted ... :::` blocks from markdown
/// Supports optional password attribute: `:::encrypted password="secret"`
/// Returns the modified markdown with placeholders and the extracted blocks
pub fn extract_encrypted_blocks(content: &str) -> PreprocessResult {
    let mut blocks = Vec::new();
    let mut block_id = 0;

    let markdown = ENCRYPTED_BLOCK_RE
        .replace_all(content, |caps: &regex::Captures| {
            let password = caps.get(1).map(|m| m.as_str().to_string());
            let block_content = caps.get(2).map_or("", |m| m.as_str()).to_string();
            let placeholder = format!("<!-- ENCRYPTED_BLOCK_{} -->", block_id);

            blocks.push(EncryptedBlock {
                content: block_content,
                id: block_id,
                password,
            });

            block_id += 1;
            placeholder
        })
        .to_string();

    PreprocessResult { markdown, blocks }
}

/// Extract `<encrypted>...</encrypted>` blocks from HTML content
/// Supports optional password attribute: `<encrypted password="secret">`
/// Returns the modified HTML with placeholders and the extracted blocks
/// Note: This should be called AFTER Tera rendering so the content inside can use Tera
pub fn extract_html_encrypted_blocks(content: &str) -> PreprocessResult {
    let mut blocks = Vec::new();
    let mut block_id = 0;

    let html = HTML_ENCRYPTED_BLOCK_RE
        .replace_all(content, |caps: &regex::Captures| {
            let password = caps.get(1).map(|m| m.as_str().to_string());
            let block_content = caps.get(2).map_or("", |m| m.as_str()).to_string();
            let placeholder = format!("<!-- ENCRYPTED_BLOCK_{} -->", block_id);

            blocks.push(EncryptedBlock {
                content: block_content,
                id: block_id,
                password,
            });

            block_id += 1;
            placeholder
        })
        .to_string();

    PreprocessResult {
        markdown: html,
        blocks,
    }
}

/// Replace placeholders in HTML with encrypted content divs
/// encrypted_htmls: Vec of (id, ciphertext, salt, nonce, has_own_password)
pub fn replace_placeholders(
    html: &str,
    encrypted_htmls: &[(usize, String, String, String, bool)],
    slug: &str,
) -> String {
    let mut result = html.to_string();

    for (id, ciphertext, salt, nonce, has_own_password) in encrypted_htmls {
        let placeholder = format!("<!-- ENCRYPTED_BLOCK_{} -->", id);
        let own_password_attr = if *has_own_password {
            "\n     data-own-password=\"true\""
        } else {
            ""
        };
        let replacement = format!(
            r#"<div class="encrypted-content encrypted-block"
     data-encrypted="{}"
     data-salt="{}"
     data-nonce="{}"
     data-slug="{}"
     data-block-id="{}"{}>
    <div class="decrypt-prompt">
        <p class="encrypted-message">This section is encrypted.</p>
        <input type="password" placeholder="Enter password..." aria-label="Password">
        <label class="remember-label">
            <input type="checkbox" class="remember">
            Remember for this post
        </label>
        <button type="button">Decrypt</button>
    </div>
</div>"#,
            ciphertext, salt, nonce, slug, id, own_password_attr
        );
        result = result.replace(&placeholder, &replacement);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_block() {
        let content = r#"# Hello

Some public content.

:::encrypted
This is secret content.
It has multiple lines.
:::

More public content.
"#;

        let result = extract_encrypted_blocks(content);

        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.blocks[0].id, 0);
        assert!(result.blocks[0].content.contains("This is secret content."));
        assert!(result.blocks[0].password.is_none());
        assert!(result.markdown.contains("<!-- ENCRYPTED_BLOCK_0 -->"));
        assert!(!result.markdown.contains(":::encrypted"));
    }

    #[test]
    fn test_extract_multiple_blocks() {
        let content = r#"# Hello

:::encrypted
First secret.
:::

Public part.

:::encrypted
Second secret.
:::

End.
"#;

        let result = extract_encrypted_blocks(content);

        assert_eq!(result.blocks.len(), 2);
        assert_eq!(result.blocks[0].id, 0);
        assert_eq!(result.blocks[1].id, 1);
        assert!(result.blocks[0].content.contains("First secret."));
        assert!(result.blocks[1].content.contains("Second secret."));
        assert!(result.markdown.contains("<!-- ENCRYPTED_BLOCK_0 -->"));
        assert!(result.markdown.contains("<!-- ENCRYPTED_BLOCK_1 -->"));
    }

    #[test]
    fn test_extract_block_with_password() {
        let content = r#"# Hello

:::encrypted password="secret123"
This is secret content.
:::

More content.
"#;

        let result = extract_encrypted_blocks(content);

        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.blocks[0].password, Some("secret123".to_string()));
        assert!(result.blocks[0].content.contains("This is secret content."));
    }

    #[test]
    fn test_extract_blocks_with_different_passwords() {
        let content = r#"# Hello

:::encrypted password="pass1"
First secret.
:::

:::encrypted password="pass2"
Second secret.
:::

:::encrypted
Third secret (no password).
:::
"#;

        let result = extract_encrypted_blocks(content);

        assert_eq!(result.blocks.len(), 3);
        assert_eq!(result.blocks[0].password, Some("pass1".to_string()));
        assert_eq!(result.blocks[1].password, Some("pass2".to_string()));
        assert_eq!(result.blocks[2].password, None);
    }

    #[test]
    fn test_no_encrypted_blocks() {
        let content = "# Hello\n\nJust regular content.\n";

        let result = extract_encrypted_blocks(content);

        assert!(result.blocks.is_empty());
        assert_eq!(result.markdown, content);
    }

    #[test]
    fn test_replace_placeholders() {
        let html = "<p>Hello</p>\n<!-- ENCRYPTED_BLOCK_0 -->\n<p>World</p>";
        let encrypted = vec![(
            0,
            "cipher".to_string(),
            "salt".to_string(),
            "nonce".to_string(),
            false,
        )];

        let result = replace_placeholders(html, &encrypted, "test-post");

        assert!(result.contains("data-encrypted=\"cipher\""));
        assert!(result.contains("data-salt=\"salt\""));
        assert!(result.contains("data-nonce=\"nonce\""));
        assert!(result.contains("data-slug=\"test-post\""));
        assert!(result.contains("data-block-id=\"0\""));
        assert!(!result.contains("data-own-password"));
        assert!(!result.contains("<!-- ENCRYPTED_BLOCK_0 -->"));
    }

    #[test]
    fn test_replace_placeholders_with_own_password() {
        let html = "<!-- ENCRYPTED_BLOCK_0 -->";
        let encrypted = vec![(
            0,
            "cipher".to_string(),
            "salt".to_string(),
            "nonce".to_string(),
            true,
        )];

        let result = replace_placeholders(html, &encrypted, "test-post");

        assert!(result.contains("data-own-password=\"true\""));
    }

    #[test]
    fn test_html_extract_single_block() {
        let content = r#"<h1>Hello</h1>
<p>Some public content.</p>
<encrypted>
This is secret content.
It has multiple lines.
</encrypted>
<p>More public content.</p>"#;

        let result = extract_html_encrypted_blocks(content);

        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.blocks[0].id, 0);
        assert!(result.blocks[0].content.contains("This is secret content."));
        assert!(result.blocks[0].password.is_none());
        assert!(result.markdown.contains("<!-- ENCRYPTED_BLOCK_0 -->"));
        assert!(!result.markdown.contains("<encrypted>"));
    }

    #[test]
    fn test_html_extract_multiple_blocks() {
        let content = r#"<h1>Hello</h1>
<encrypted>First secret.</encrypted>
<p>Public part.</p>
<encrypted>Second secret.</encrypted>
<p>End.</p>"#;

        let result = extract_html_encrypted_blocks(content);

        assert_eq!(result.blocks.len(), 2);
        assert_eq!(result.blocks[0].id, 0);
        assert_eq!(result.blocks[1].id, 1);
        assert!(result.blocks[0].content.contains("First secret."));
        assert!(result.blocks[1].content.contains("Second secret."));
        assert!(result.markdown.contains("<!-- ENCRYPTED_BLOCK_0 -->"));
        assert!(result.markdown.contains("<!-- ENCRYPTED_BLOCK_1 -->"));
    }

    #[test]
    fn test_html_extract_block_with_password() {
        let content = r#"<h1>Hello</h1>
<encrypted password="secret123">This is secret content.</encrypted>
<p>More content.</p>"#;

        let result = extract_html_encrypted_blocks(content);

        assert_eq!(result.blocks.len(), 1);
        assert_eq!(result.blocks[0].password, Some("secret123".to_string()));
        assert!(result.blocks[0].content.contains("This is secret content."));
    }

    #[test]
    fn test_html_extract_blocks_with_different_passwords() {
        let content = r#"<encrypted password="pass1">First secret.</encrypted>
<encrypted password="pass2">Second secret.</encrypted>
<encrypted>Third secret (no password).</encrypted>"#;

        let result = extract_html_encrypted_blocks(content);

        assert_eq!(result.blocks.len(), 3);
        assert_eq!(result.blocks[0].password, Some("pass1".to_string()));
        assert_eq!(result.blocks[1].password, Some("pass2".to_string()));
        assert_eq!(result.blocks[2].password, None);
    }

    #[test]
    fn test_html_no_encrypted_blocks() {
        let content = "<h1>Hello</h1>\n<p>Just regular content.</p>\n";

        let result = extract_html_encrypted_blocks(content);

        assert!(result.blocks.is_empty());
        assert_eq!(result.markdown, content);
    }
}
