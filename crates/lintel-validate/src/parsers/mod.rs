mod json;
mod json5;
mod jsonc;
mod markdown;
mod toml_parser;
mod yaml;

use std::path::Path;

use serde_json::Value;

use crate::diagnostics::ParseDiagnostic;

pub use self::json::JsonParser;
pub use self::json5::Json5Parser;
pub use self::jsonc::JsoncParser;
pub use self::markdown::MarkdownParser;
pub use self::toml_parser::TomlParser;
pub use self::yaml::YamlParser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Json,
    Json5,
    Jsonc,
    Toml,
    Yaml,
    Markdown,
}

/// Parse file content into a `serde_json::Value`.
///
/// Implementations must produce a [`ParseDiagnostic`] with an accurate source
/// span when parsing fails.
pub trait Parser {
    /// # Errors
    ///
    /// Returns a [`ParseDiagnostic`] with an accurate source span when parsing fails.
    fn parse(&self, content: &str, file_name: &str) -> Result<Value, ParseDiagnostic>;

    /// Extract the `$schema` URI from file content and/or parsed value.
    ///
    /// The default implementation reads `value["$schema"]`, which works for
    /// JSON, JSON5, and JSONC. YAML and TOML override this to handle their
    /// format-specific conventions (modeline comments, etc.).
    fn extract_schema_uri(&self, _content: &str, value: &Value) -> Option<String> {
        value
            .get("$schema")
            .and_then(Value::as_str)
            .map(String::from)
    }

    /// Insert a schema annotation into the file content.
    ///
    /// Returns `Some(annotated_content)` if the format supports inline schema
    /// annotations, or `None` if it does not (e.g. Markdown).
    fn annotate(&self, _content: &str, _schema_url: &str) -> Option<String> {
        None
    }

    /// Remove an existing schema annotation from the file content.
    ///
    /// Returns the content with the annotation stripped. If no annotation is
    /// found, returns the content unchanged.
    fn strip_annotation(&self, content: &str) -> String {
        content.to_string()
    }
}

/// Detect file format from extension. Returns `None` for unrecognized extensions.
pub fn detect_format(path: &Path) -> Option<FileFormat> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("yaml" | "yml") => Some(FileFormat::Yaml),
        Some("json5") => Some(FileFormat::Json5),
        Some("json" | "jsonc") => Some(FileFormat::Jsonc),
        Some("toml") => Some(FileFormat::Toml),
        Some("md" | "mdx") => Some(FileFormat::Markdown),
        _ => None,
    }
}

/// Return a boxed parser for the given format.
pub fn parser_for(format: FileFormat) -> Box<dyn Parser> {
    match format {
        FileFormat::Json => Box::new(JsonParser),
        FileFormat::Json5 => Box::new(Json5Parser),
        FileFormat::Jsonc => Box::new(JsoncParser),
        FileFormat::Toml => Box::new(TomlParser),
        FileFormat::Yaml => Box::new(YamlParser),
        FileFormat::Markdown => Box::new(MarkdownParser),
    }
}

/// Insert `"$schema": "URL"` as the first property after `{` in a JSON object.
///
/// Uses string manipulation (not parse+reserialize) to preserve formatting.
pub(crate) fn annotate_json_content(content: &str, schema_url: &str) -> String {
    let Some(brace_pos) = content.find('{') else {
        return content.to_string();
    };

    let after_brace = &content[brace_pos + 1..];

    // Detect if the content is compact (no newline before next non-whitespace)
    let next_non_ws = after_brace.find(|c: char| !c.is_ascii_whitespace());
    let has_newline_before_content = after_brace
        .get(..next_non_ws.unwrap_or(0))
        .is_some_and(|s| s.contains('\n'));

    if has_newline_before_content {
        let indent = detect_json_indent(after_brace);
        format!(
            "{}{{\n{indent}\"$schema\": \"{schema_url}\",{}",
            &content[..brace_pos],
            after_brace
        )
    } else {
        format!(
            "{}{{\"$schema\":\"{schema_url}\",{}",
            &content[..brace_pos],
            after_brace.trim_start()
        )
    }
}

/// Detect the indentation used in a JSON string (the whitespace at the start
/// of the first content line after the opening brace).
fn detect_json_indent(after_brace: &str) -> String {
    for line in after_brace.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent_end = line.len() - line.trim_start().len();
        return line[..indent_end].to_string();
    }
    "  ".to_string()
}

/// Remove the top-level `"$schema"` property from a JSON string.
///
/// Uses string manipulation (not parse+reserialize) to preserve formatting.
pub(crate) fn strip_json_schema_property(content: &str) -> String {
    let key = "\"$schema\"";
    let Some(key_start) = content.find(key) else {
        return content.to_string();
    };

    let key_end = key_start + key.len();
    let mut pos = key_end;

    // Skip whitespace (space/tab) between key and colon
    while pos < content.len() && matches!(content.as_bytes()[pos], b' ' | b'\t') {
        pos += 1;
    }
    // Expect colon
    if content.as_bytes().get(pos) != Some(&b':') {
        return content.to_string();
    }
    pos += 1;

    // Skip whitespace (space/tab) between colon and value
    while pos < content.len() && matches!(content.as_bytes()[pos], b' ' | b'\t') {
        pos += 1;
    }
    // Expect opening quote
    if content.as_bytes().get(pos) != Some(&b'"') {
        return content.to_string();
    }
    pos += 1;

    // Read string value until closing quote (handling backslash escapes)
    while pos < content.len() {
        match content.as_bytes()[pos] {
            b'\\' => pos += 2,
            b'"' => {
                pos += 1;
                break;
            }
            _ => pos += 1,
        }
    }
    let value_end = pos;

    // Check for trailing comma (with optional space/tab before it)
    let ws_after = content.as_bytes()[value_end..]
        .iter()
        .take_while(|&&b| b == b' ' || b == b'\t')
        .count();
    let has_trailing_comma = content.as_bytes().get(value_end + ws_after) == Some(&b',');

    if has_trailing_comma {
        let remove_end = value_end + ws_after + 1; // past the comma
        let before = &content[..key_start];
        if let Some(nl_pos) = before.rfind('\n') {
            // Pretty-printed: remove from newline to past the comma
            format!("{}{}", &content[..nl_pos], &content[remove_end..])
        } else {
            // Compact: remove key-value+comma and any space/tab after comma
            let ws_skip = content.as_bytes()[remove_end..]
                .iter()
                .take_while(|&&b| b == b' ' || b == b'\t')
                .count();
            format!(
                "{}{}",
                &content[..key_start],
                &content[remove_end + ws_skip..]
            )
        }
    } else {
        // No trailing comma — $schema is the only or last property
        let before = &content[..key_start];
        let rtrimmed = before.trim_end();
        if rtrimmed.ends_with(',') {
            // Last property: also remove the preceding comma
            let comma_pos = before.rfind(',').expect("comma before $schema");
            format!("{}{}", &content[..comma_pos], &content[value_end..])
        } else if let Some(nl_pos) = before.rfind('\n') {
            // Only property, pretty-printed
            format!("{}{}", &content[..nl_pos], &content[value_end..])
        } else {
            // Only property, compact
            format!("{}{}", &content[..key_start], &content[value_end..])
        }
    }
}

/// Convert 1-based line and column to a byte offset in content.
pub fn line_col_to_offset(content: &str, line: usize, col: usize) -> usize {
    let mut offset = 0;
    for (i, l) in content.lines().enumerate() {
        if i + 1 == line {
            return offset + col.saturating_sub(1);
        }
        offset += l.len() + 1; // +1 for newline
    }
    offset.min(content.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- detect_format ---

    #[test]
    fn detect_format_json() {
        assert_eq!(
            detect_format(Path::new("foo.json")),
            Some(FileFormat::Jsonc)
        );
    }

    #[test]
    fn detect_format_yaml() {
        assert_eq!(detect_format(Path::new("foo.yaml")), Some(FileFormat::Yaml));
        assert_eq!(detect_format(Path::new("foo.yml")), Some(FileFormat::Yaml));
    }

    #[test]
    fn detect_format_json5() {
        assert_eq!(
            detect_format(Path::new("foo.json5")),
            Some(FileFormat::Json5)
        );
    }

    #[test]
    fn detect_format_jsonc() {
        assert_eq!(
            detect_format(Path::new("foo.jsonc")),
            Some(FileFormat::Jsonc)
        );
    }

    #[test]
    fn detect_format_toml() {
        assert_eq!(detect_format(Path::new("foo.toml")), Some(FileFormat::Toml));
    }

    #[test]
    fn detect_format_unknown_returns_none() {
        assert_eq!(detect_format(Path::new("foo.txt")), None);
        assert_eq!(detect_format(Path::new("foo")), None);
        assert_eq!(detect_format(Path::new("devenv.nix")), None);
    }

    // --- extract_schema_uri via trait ---

    #[test]
    fn extract_schema_json_with_schema() {
        let val = serde_json::json!({"$schema": "https://example.com/s.json"});
        let uri = JsonParser.extract_schema_uri("", &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn extract_schema_json_without_schema() {
        let val = serde_json::json!({"key": "value"});
        let uri = JsonParser.extract_schema_uri("", &val);
        assert!(uri.is_none());
    }

    #[test]
    fn extract_schema_json5_with_schema() {
        let val = serde_json::json!({"$schema": "https://example.com/s.json"});
        let uri = Json5Parser.extract_schema_uri("", &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn extract_schema_jsonc_with_schema() {
        let val = serde_json::json!({"$schema": "https://example.com/s.json"});
        let uri = JsoncParser.extract_schema_uri("", &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn extract_schema_yaml_modeline() {
        let content = "# yaml-language-server: $schema=https://example.com/s.json\nkey: value\n";
        let val = serde_json::json!({"key": "value"});
        let uri = YamlParser.extract_schema_uri(content, &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn extract_schema_yaml_modeline_with_leading_blank_lines() {
        let content = "\n# yaml-language-server: $schema=https://example.com/s.json\nkey: value\n";
        let val = serde_json::json!({"key": "value"});
        let uri = YamlParser.extract_schema_uri(content, &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn extract_schema_yaml_modeline_after_other_comment() {
        let content = "# some comment\n# yaml-language-server: $schema=https://example.com/s.json\nkey: value\n";
        let val = serde_json::json!({"key": "value"});
        let uri = YamlParser.extract_schema_uri(content, &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn extract_schema_yaml_modeline_not_in_body() {
        let content = "key: value\n# yaml-language-server: $schema=https://example.com/s.json\n";
        let val = serde_json::json!({"key": "value"});
        let uri = YamlParser.extract_schema_uri(content, &val);
        assert!(uri.is_none());
    }

    #[test]
    fn extract_schema_yaml_top_level_property() {
        let content = "$schema: https://example.com/s.json\nkey: value\n";
        let val = serde_json::json!({"$schema": "https://example.com/s.json", "key": "value"});
        let uri = YamlParser.extract_schema_uri(content, &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn extract_schema_yaml_modeline_takes_priority() {
        let content = "# yaml-language-server: $schema=https://modeline.com/s.json\n$schema: https://property.com/s.json\n";
        let val = serde_json::json!({"$schema": "https://property.com/s.json"});
        let uri = YamlParser.extract_schema_uri(content, &val);
        assert_eq!(uri.as_deref(), Some("https://modeline.com/s.json"));
    }

    #[test]
    fn extract_schema_yaml_none() {
        let content = "key: value\n";
        let val = serde_json::json!({"key": "value"});
        let uri = YamlParser.extract_schema_uri(content, &val);
        assert!(uri.is_none());
    }

    // --- TOML schema extraction ---

    #[test]
    fn extract_schema_toml_comment() {
        let content = "# :schema https://example.com/s.json\nkey = \"value\"\n";
        let val = serde_json::json!({"key": "value"});
        let uri = TomlParser.extract_schema_uri(content, &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn extract_schema_toml_with_leading_blank_lines() {
        let content = "\n# :schema https://example.com/s.json\nkey = \"value\"\n";
        let val = serde_json::json!({"key": "value"});
        let uri = TomlParser.extract_schema_uri(content, &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn extract_schema_toml_not_in_body() {
        let content = "key = \"value\"\n# :schema https://example.com/s.json\n";
        let val = serde_json::json!({"key": "value"});
        let uri = TomlParser.extract_schema_uri(content, &val);
        assert!(uri.is_none());
    }

    #[test]
    fn extract_schema_toml_none() {
        let content = "key = \"value\"\n";
        let val = serde_json::json!({"key": "value"});
        let uri = TomlParser.extract_schema_uri(content, &val);
        assert!(uri.is_none());
    }

    #[test]
    fn extract_schema_toml_legacy_dollar_schema() {
        let content = "# $schema: https://example.com/s.json\nkey = \"value\"\n";
        let val = serde_json::json!({"key": "value"});
        let uri = TomlParser.extract_schema_uri(content, &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    // --- line_col_to_offset ---

    #[test]
    fn line_col_to_offset_first_line() {
        assert_eq!(line_col_to_offset("hello\nworld", 1, 1), 0);
        assert_eq!(line_col_to_offset("hello\nworld", 1, 3), 2);
    }

    #[test]
    fn line_col_to_offset_second_line() {
        assert_eq!(line_col_to_offset("hello\nworld", 2, 1), 6);
        assert_eq!(line_col_to_offset("hello\nworld", 2, 3), 8);
    }

    // --- parser_for round-trip ---

    #[test]
    fn parser_for_json_parses() -> anyhow::Result<()> {
        let p = parser_for(FileFormat::Json);
        let val = p.parse(r#"{"key":"value"}"#, "test.json")?;
        assert_eq!(val, serde_json::json!({"key": "value"}));
        Ok(())
    }

    #[test]
    fn parser_for_yaml_parses() -> anyhow::Result<()> {
        let p = parser_for(FileFormat::Yaml);
        let val = p.parse("key: value\n", "test.yaml")?;
        assert_eq!(val, serde_json::json!({"key": "value"}));
        Ok(())
    }

    #[test]
    fn parser_for_json5_parses() -> anyhow::Result<()> {
        let p = parser_for(FileFormat::Json5);
        let val = p.parse(r#"{key: "value"}"#, "test.json5")?;
        assert_eq!(val, serde_json::json!({"key": "value"}));
        Ok(())
    }

    #[test]
    fn parser_for_jsonc_parses() -> anyhow::Result<()> {
        let p = parser_for(FileFormat::Jsonc);
        let val = p.parse(r#"{"key": "value" /* comment */}"#, "test.jsonc")?;
        assert_eq!(val, serde_json::json!({"key": "value"}));
        Ok(())
    }

    #[test]
    fn parser_for_toml_parses() -> anyhow::Result<()> {
        let p = parser_for(FileFormat::Toml);
        let val = p.parse("key = \"value\"\n", "test.toml")?;
        assert_eq!(val, serde_json::json!({"key": "value"}));
        Ok(())
    }
}
