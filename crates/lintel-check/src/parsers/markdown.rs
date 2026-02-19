use serde_json::Value;

use crate::diagnostics::ParseDiagnostic;

use super::Parser;

pub struct MarkdownParser;

/// Skip leading whitespace and HTML comments (`<!-- ... -->`).
/// Returns the remaining content and the byte offset into the original string.
fn skip_html_comments(content: &str) -> (&str, usize) {
    let mut s = content.trim_start();
    let mut offset = content.len() - s.len();

    while s.starts_with("<!--") {
        if let Some(end) = s.find("-->") {
            let after = &s[end + 3..];
            let trimmed = after.trim_start();
            offset += s.len() - trimmed.len();
            s = trimmed;
        } else {
            // Unclosed comment — stop skipping
            break;
        }
    }

    (s, offset)
}

/// Extract YAML frontmatter delimited by `---`.
fn extract_yaml_frontmatter(content: &str) -> Option<(&str, usize)> {
    let (trimmed, offset) = skip_html_comments(content);

    if !trimmed.starts_with("---") {
        return None;
    }

    let after_open = &trimmed[3..];
    // The opening --- must be followed by a newline
    let after_newline = after_open
        .strip_prefix('\n')
        .or_else(|| after_open.strip_prefix("\r\n"))?;

    let front_start = offset + 3 + (after_open.len() - after_newline.len());

    // Find closing ---
    let closing = after_newline.find("\n---")?;
    let frontmatter = &after_newline[..closing];

    Some((frontmatter, front_start))
}

/// Extract TOML frontmatter delimited by `+++`.
fn extract_toml_frontmatter(content: &str) -> Option<(&str, usize)> {
    let (trimmed, offset) = skip_html_comments(content);

    if !trimmed.starts_with("+++") {
        return None;
    }

    let after_open = &trimmed[3..];
    let after_newline = after_open
        .strip_prefix('\n')
        .or_else(|| after_open.strip_prefix("\r\n"))?;

    let front_start = offset + 3 + (after_open.len() - after_newline.len());

    let closing = after_newline.find("\n+++")?;
    let frontmatter = &after_newline[..closing];

    Some((frontmatter, front_start))
}

impl Parser for MarkdownParser {
    fn parse(&self, content: &str, file_name: &str) -> Result<Value, ParseDiagnostic> {
        // Try YAML frontmatter first (---)
        if let Some((frontmatter, offset)) = extract_yaml_frontmatter(content) {
            return serde_yaml::from_str(frontmatter).map_err(|e| {
                let span = e
                    .location()
                    .map(|loc| offset + loc.index())
                    .unwrap_or(offset);
                ParseDiagnostic {
                    src: miette::NamedSource::new(file_name, content.to_string()),
                    span: span.into(),
                    message: format!("YAML frontmatter: {e}"),
                }
            });
        }

        // Try TOML frontmatter (+++)
        if let Some((frontmatter, offset)) = extract_toml_frontmatter(content) {
            let toml_value: toml::Value = toml::from_str(frontmatter).map_err(|e| {
                let span = e.span().map(|s| offset + s.start).unwrap_or(offset);
                ParseDiagnostic {
                    src: miette::NamedSource::new(file_name, content.to_string()),
                    span: span.into(),
                    message: format!("TOML frontmatter: {e}"),
                }
            })?;
            return serde_json::to_value(toml_value).map_err(|e| ParseDiagnostic {
                src: miette::NamedSource::new(file_name, content.to_string()),
                span: offset.into(),
                message: format!("TOML frontmatter conversion: {e}"),
            });
        }

        // No frontmatter found — return null so it gets skipped
        Ok(Value::Null)
    }

    fn extract_schema_uri(&self, content: &str, value: &Value) -> Option<String> {
        // Check for $schema in frontmatter value
        if let Some(uri) = value.get("$schema").and_then(Value::as_str) {
            return Some(uri.to_string());
        }

        // Check for schema comment before frontmatter
        // e.g. <!-- $schema: https://... -->
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("<!--") {
                let rest = rest.trim();
                if let Some(rest) = rest.strip_prefix("$schema:") {
                    let rest = rest.trim().trim_end_matches("-->").trim();
                    if !rest.is_empty() {
                        return Some(rest.to_string());
                    }
                }
            }
            // Stop at frontmatter delimiter
            if trimmed == "---" || trimmed == "+++" {
                break;
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_yaml_frontmatter() {
        let content = "---\nname: test\ndescription: hello\n---\n# Body\n";
        let val = MarkdownParser.parse(content, "test.md").unwrap();
        assert_eq!(val["name"], "test");
        assert_eq!(val["description"], "hello");
    }

    #[test]
    fn parse_toml_frontmatter() {
        let content = "+++\nname = \"test\"\n+++\n# Body\n";
        let val = MarkdownParser.parse(content, "test.md").unwrap();
        assert_eq!(val["name"], "test");
    }

    #[test]
    fn no_frontmatter_returns_null() {
        let content = "# Just a heading\nSome text\n";
        let val = MarkdownParser.parse(content, "test.md").unwrap();
        assert!(val.is_null());
    }

    #[test]
    fn extract_schema_from_frontmatter_value() {
        let val = serde_json::json!({"$schema": "https://example.com/s.json", "name": "test"});
        let uri = MarkdownParser.extract_schema_uri("---\n$schema: ...\n---\n", &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn extract_schema_from_html_comment() {
        let content = "<!-- $schema: https://example.com/s.json -->\n---\nname: test\n---\n";
        let val = serde_json::json!({"name": "test"});
        let uri = MarkdownParser.extract_schema_uri(content, &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn yaml_frontmatter_with_leading_html_comment() {
        let content =
            "<!-- $schema: https://example.com/s.json -->\n---\nname: test\n---\n# Body\n";
        let val = MarkdownParser.parse(content, "test.md").unwrap();
        assert_eq!(val["name"], "test");
    }

    #[test]
    fn toml_frontmatter_with_leading_html_comment() {
        let content =
            "<!-- $schema: https://example.com/s.json -->\n+++\nname = \"test\"\n+++\n# Body\n";
        let val = MarkdownParser.parse(content, "test.md").unwrap();
        assert_eq!(val["name"], "test");
    }

    #[test]
    fn html_comment_schema_plus_yaml_frontmatter() {
        let content =
            "<!-- $schema: https://example.com/s.json -->\n---\nname: researcher\n---\n# Body\n";
        let val = MarkdownParser.parse(content, "test.md").unwrap();
        assert_eq!(val["name"], "researcher");
        let uri = MarkdownParser.extract_schema_uri(content, &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn multiple_html_comments_before_frontmatter() {
        let content = "<!-- comment 1 -->\n<!-- comment 2 -->\n---\nname: test\n---\n";
        let val = MarkdownParser.parse(content, "test.md").unwrap();
        assert_eq!(val["name"], "test");
    }

    #[test]
    fn yaml_frontmatter_with_complex_values() {
        let content = "---\nname: my-skill\nallowed-tools:\n  - Bash\n  - Read\n---\n# Body\n";
        let val = MarkdownParser.parse(content, "test.md").unwrap();
        assert_eq!(val["name"], "my-skill");
        assert!(val["allowed-tools"].is_array());
    }
}
