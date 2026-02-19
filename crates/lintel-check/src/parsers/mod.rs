mod json;
mod json5;
mod jsonc;
mod toml_parser;
mod yaml;

use std::path::Path;

use serde_json::Value;

use crate::diagnostics::ParseDiagnostic;

pub use self::json::JsonParser;
pub use self::json5::Json5Parser;
pub use self::jsonc::JsoncParser;
pub use self::toml_parser::TomlParser;
pub use self::yaml::YamlParser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Json,
    Json5,
    Jsonc,
    Toml,
    Yaml,
}

/// Parse file content into a `serde_json::Value`.
///
/// Implementations must produce a [`ParseDiagnostic`] with an accurate source
/// span when parsing fails.
pub trait Parser {
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
}

/// Detect file format from extension. Defaults to JSON for unknown extensions.
pub fn detect_format(path: &Path) -> FileFormat {
    match path.extension().and_then(|e| e.to_str()) {
        Some("yaml" | "yml") => FileFormat::Yaml,
        Some("json5") => FileFormat::Json5,
        Some("jsonc") => FileFormat::Jsonc,
        Some("toml") => FileFormat::Toml,
        _ => FileFormat::Json,
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
        assert_eq!(detect_format(Path::new("foo.json")), FileFormat::Json);
    }

    #[test]
    fn detect_format_yaml() {
        assert_eq!(detect_format(Path::new("foo.yaml")), FileFormat::Yaml);
        assert_eq!(detect_format(Path::new("foo.yml")), FileFormat::Yaml);
    }

    #[test]
    fn detect_format_json5() {
        assert_eq!(detect_format(Path::new("foo.json5")), FileFormat::Json5);
    }

    #[test]
    fn detect_format_jsonc() {
        assert_eq!(detect_format(Path::new("foo.jsonc")), FileFormat::Jsonc);
    }

    #[test]
    fn detect_format_toml() {
        assert_eq!(detect_format(Path::new("foo.toml")), FileFormat::Toml);
    }

    #[test]
    fn detect_format_unknown_defaults_to_json() {
        assert_eq!(detect_format(Path::new("foo.txt")), FileFormat::Json);
        assert_eq!(detect_format(Path::new("foo")), FileFormat::Json);
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
        let content = "# $schema: https://example.com/s.json\nkey = \"value\"\n";
        let val = serde_json::json!({"key": "value"});
        let uri = TomlParser.extract_schema_uri(content, &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn extract_schema_toml_with_leading_blank_lines() {
        let content = "\n# $schema: https://example.com/s.json\nkey = \"value\"\n";
        let val = serde_json::json!({"key": "value"});
        let uri = TomlParser.extract_schema_uri(content, &val);
        assert_eq!(uri.as_deref(), Some("https://example.com/s.json"));
    }

    #[test]
    fn extract_schema_toml_not_in_body() {
        let content = "key = \"value\"\n# $schema: https://example.com/s.json\n";
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
    fn parser_for_json_parses() {
        let p = parser_for(FileFormat::Json);
        let val = p.parse(r#"{"key":"value"}"#, "test.json").unwrap();
        assert_eq!(val, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn parser_for_yaml_parses() {
        let p = parser_for(FileFormat::Yaml);
        let val = p.parse("key: value\n", "test.yaml").unwrap();
        assert_eq!(val, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn parser_for_json5_parses() {
        let p = parser_for(FileFormat::Json5);
        let val = p.parse(r#"{key: "value"}"#, "test.json5").unwrap();
        assert_eq!(val, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn parser_for_jsonc_parses() {
        let p = parser_for(FileFormat::Jsonc);
        let val = p
            .parse(r#"{"key": "value" /* comment */}"#, "test.jsonc")
            .unwrap();
        assert_eq!(val, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn parser_for_toml_parses() {
        let p = parser_for(FileFormat::Toml);
        let val = p.parse("key = \"value\"\n", "test.toml").unwrap();
        assert_eq!(val, serde_json::json!({"key": "value"}));
    }
}
