use anyhow::{Result, anyhow};
use chrono::NaiveDate;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize, Clone)]
pub struct Frontmatter {
    pub title: String,
    pub description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_date_option")]
    pub date: Option<NaiveDate>,
    pub tags: Option<Vec<String>>,
    pub draft: Option<bool>,
    pub image: Option<String>,
    pub template: Option<String>,
    pub slug: Option<String>,
    pub permalink: Option<String>,
    /// Whether this post's content should be encrypted
    #[serde(default)]
    pub encrypted: bool,
    /// Per-post password (overrides global encryption password)
    pub password: Option<String>,
}

fn deserialize_date_option<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<NaiveDate>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DateOrString {
        Date(toml::value::Datetime),
        String(String),
    }

    match Option::<DateOrString>::deserialize(deserializer)? {
        None => Ok(None),
        Some(DateOrString::String(s)) => NaiveDate::parse_from_str(&s, "%Y-%m-%d")
            .map(Some)
            .map_err(|e| D::Error::custom(format!("invalid date string: {}", e))),
        Some(DateOrString::Date(dt)) => {
            if let Some(date) = dt.date {
                Ok(Some(
                    NaiveDate::from_ymd_opt(date.year as i32, date.month as u32, date.day as u32)
                        .ok_or_else(|| D::Error::custom("invalid date"))?,
                ))
            } else {
                Err(D::Error::custom("datetime missing date component"))
            }
        }
    }
}

/// Parse frontmatter from content string.
/// Supports YAML (---) and TOML (+++) delimiters.
pub fn parse_frontmatter(content: &str) -> Result<(Frontmatter, &str)> {
    let content = content.trim_start();

    // Check for YAML frontmatter (---)
    if content.starts_with("---") {
        let after_start = &content[3..];
        if let Some(end_pos) = after_start.find("\n---") {
            let frontmatter_str = &after_start[..end_pos].trim();
            let remaining = &after_start[end_pos + 4..].trim_start();

            let frontmatter: Frontmatter = serde_yaml::from_str(frontmatter_str)
                .map_err(|e| anyhow!("Failed to parse YAML frontmatter: {}", e))?;

            return Ok((frontmatter, remaining));
        }
    }

    // Check for TOML frontmatter (+++)
    if content.starts_with("+++") {
        let after_start = &content[3..];
        if let Some(end_pos) = after_start.find("\n+++") {
            let frontmatter_str = &after_start[..end_pos].trim();
            let remaining = &after_start[end_pos + 4..].trim_start();

            let frontmatter: Frontmatter = toml::from_str(frontmatter_str)
                .map_err(|e| anyhow!("Failed to parse TOML frontmatter: {}", e))?;

            return Ok((frontmatter, remaining));
        }
    }

    Err(anyhow!(
        "No valid frontmatter found. Use --- for YAML or +++ for TOML."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_frontmatter() {
        let content = r#"---
title: "Test Post"
description: "A test description"
date: 2024-01-15
tags: ["rust", "web"]
---

This is the content.
"#;

        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.title, "Test Post");
        assert_eq!(fm.description, Some("A test description".to_string()));
        assert!(body.starts_with("This is the content."));
    }

    #[test]
    fn test_toml_frontmatter() {
        let content = r#"+++
title = "Test Post"
description = "A test description"
date = 2024-01-15
tags = ["rust", "web"]
+++

This is the content.
"#;

        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.title, "Test Post");
        assert!(body.starts_with("This is the content."));
    }

    #[test]
    fn test_yaml_with_slug() {
        let content = r#"---
title: "Test Post"
slug: "custom-slug"
---

Content.
"#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.slug, Some("custom-slug".to_string()));
    }

    #[test]
    fn test_yaml_with_permalink() {
        let content = r#"---
title: "Test Post"
permalink: "/:year/:month/:slug/"
---

Content.
"#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.permalink, Some("/:year/:month/:slug/".to_string()));
    }

    #[test]
    fn test_yaml_with_template() {
        let content = r#"---
title: "Test Post"
template: "custom.html"
---

Content.
"#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.template, Some("custom.html".to_string()));
    }

    #[test]
    fn test_toml_with_slug_permalink_template() {
        let content = r#"+++
title = "Test Post"
slug = "my-custom-slug"
permalink = "/blog/:year/:slug/"
template = "special.html"
+++

Content.
"#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.slug, Some("my-custom-slug".to_string()));
        assert_eq!(fm.permalink, Some("/blog/:year/:slug/".to_string()));
        assert_eq!(fm.template, Some("special.html".to_string()));
    }

    #[test]
    fn test_toml_native_date() {
        let content = r#"+++
title = "Test Post"
date = 2024-01-15
+++

Content.
"#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
    }

    #[test]
    fn test_toml_string_date() {
        let content = r#"+++
title = "Test Post"
date = "2024-01-15"
+++

Content.
"#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
    }

    #[test]
    fn test_yaml_date() {
        let content = r#"---
title: "Test Post"
date: 2024-01-15
---

Content.
"#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
    }

    #[test]
    fn test_optional_fields_default_to_none() {
        let content = r#"---
title: "Minimal Post"
---

Content.
"#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.title, "Minimal Post");
        assert!(fm.description.is_none());
        assert!(fm.date.is_none());
        assert!(fm.tags.is_none());
        assert!(fm.draft.is_none());
        assert!(fm.image.is_none());
        assert!(fm.template.is_none());
        assert!(fm.slug.is_none());
        assert!(fm.permalink.is_none());
        assert!(!fm.encrypted);
        assert!(fm.password.is_none());
    }

    #[test]
    fn test_yaml_encrypted_post() {
        let content = r#"---
title: "Secret Post"
encrypted: true
---

Secret content.
"#;

        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.title, "Secret Post");
        assert!(fm.encrypted);
        assert!(fm.password.is_none());
        assert!(body.starts_with("Secret content."));
    }

    #[test]
    fn test_yaml_encrypted_with_password() {
        let content = r#"---
title: "Secret Post"
encrypted: true
password: "post-specific-secret"
---

Secret content.
"#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert!(fm.encrypted);
        assert_eq!(fm.password, Some("post-specific-secret".to_string()));
    }

    #[test]
    fn test_toml_encrypted_post() {
        let content = r#"+++
title = "Secret Post"
encrypted = true
password = "my-secret"
+++

Secret content.
"#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert!(fm.encrypted);
        assert_eq!(fm.password, Some("my-secret".to_string()));
    }
}
