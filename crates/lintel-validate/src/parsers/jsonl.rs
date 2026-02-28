use miette::NamedSource;
use serde_json::Value;

use crate::diagnostics::ParseDiagnostic;

use super::Parser;

/// A single parsed line from a JSONL file.
#[derive(Debug)]
pub struct JsonlLine {
    /// 1-based line number in the original file.
    pub line_number: usize,
    /// Byte offset of the start of this line in the original file.
    pub byte_offset: usize,
    /// Parsed JSON value.
    pub value: Value,
    /// Raw line text (without trailing newline).
    pub raw: String,
}

/// A UTF-8 BOM (byte order mark).
const BOM: &str = "\u{feff}";

/// Parser for JSONL/NDJSON files.
///
/// `parse()` returns the first line's value (for schema identification).
/// Per-line validation is handled separately by `parse_jsonl()`.
pub struct JsonlParser;

impl Parser for JsonlParser {
    fn parse(&self, content: &str, file_name: &str) -> Result<Value, ParseDiagnostic> {
        let lines = parse_jsonl(content, file_name)?;
        lines
            .into_iter()
            .next()
            .map(|l| l.value)
            .ok_or_else(|| ParseDiagnostic {
                src: NamedSource::new(file_name, content.to_string()),
                span: 0.into(),
                message: "empty JSONL file".to_string(),
            })
    }

    fn extract_schema_uri(&self, _content: &str, value: &Value) -> Option<String> {
        extract_schema_uri(value)
    }

    fn annotate(&self, _content: &str, _schema_url: &str) -> Option<String> {
        None
    }
}

/// Parse JSONL content into a list of [`JsonlLine`]s.
///
/// Strips a leading UTF-8 BOM if present. Empty lines are skipped.
///
/// # Errors
///
/// Returns a [`ParseDiagnostic`] on the first line that fails to parse as JSON.
pub fn parse_jsonl(content: &str, file_name: &str) -> Result<Vec<JsonlLine>, ParseDiagnostic> {
    // Strip BOM; all offsets are relative to the post-BOM content which is
    // what we report as the source for diagnostics.
    let content = content.strip_prefix(BOM).unwrap_or(content);

    let mut lines = Vec::new();
    let mut byte_offset: usize = 0;

    // Use split('\n') instead of lines() so that '\r' is preserved in each
    // yielded slice. This makes `line.len() + 1` correct for both LF and CRLF:
    // on CRLF the '\r' is included in the length, and we add 1 for the '\n'.
    for (line_idx, line) in content.split('\n').enumerate() {
        let line_number = line_idx + 1;
        let line_trimmed = line.trim_end_matches('\r');

        if line_trimmed.is_empty() {
            byte_offset += line.len() + 1; // +1 for \n
            continue;
        }

        match serde_json::from_str::<Value>(line_trimmed) {
            Ok(value) => {
                lines.push(JsonlLine {
                    line_number,
                    byte_offset,
                    value,
                    raw: line_trimmed.to_string(),
                });
            }
            Err(e) => {
                let error_offset =
                    byte_offset + super::line_col_to_offset(line_trimmed, e.line(), e.column());
                return Err(ParseDiagnostic {
                    src: NamedSource::new(file_name, content.to_string()),
                    span: error_offset.into(),
                    message: format!("line {line_number}: {e}"),
                });
            }
        }

        byte_offset += line.len() + 1; // +1 for \n
    }

    Ok(lines)
}

/// Extract the `$schema` URI from a parsed JSON value.
pub fn extract_schema_uri(value: &Value) -> Option<String> {
    value
        .get("$schema")
        .and_then(Value::as_str)
        .map(String::from)
}

/// A line whose `$schema` differs from the majority/first schema.
#[derive(Debug)]
pub struct SchemaMismatch {
    pub line_number: usize,
    pub byte_offset: usize,
    pub schema_uri: String,
}

/// Check whether all lines in a JSONL file use the same `$schema`.
///
/// Returns `None` if all lines are consistent (or no lines have `$schema`).
/// Returns `Some(mismatches)` listing lines whose `$schema` differs from the
/// first line that declares one.
pub fn check_schema_consistency(lines: &[JsonlLine]) -> Option<Vec<SchemaMismatch>> {
    // Find the first schema URI.
    let first_schema = lines
        .iter()
        .find_map(|line| extract_schema_uri(&line.value))?;

    let mut mismatches = Vec::new();
    for line in lines {
        if let Some(uri) = extract_schema_uri(&line.value).filter(|uri| uri != &first_schema) {
            mismatches.push(SchemaMismatch {
                line_number: line.line_number,
                byte_offset: line.byte_offset,
                schema_uri: uri,
            });
        }
    }

    if mismatches.is_empty() {
        None
    } else {
        Some(mismatches)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn basic_parsing() {
        let content = r#"{"name":"alice"}
{"name":"bob"}
"#;
        let lines = parse_jsonl(content, "test.jsonl").unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_number, 1);
        assert_eq!(lines[0].byte_offset, 0);
        assert_eq!(lines[0].value["name"], "alice");
        assert_eq!(lines[1].line_number, 2);
        // {"name":"alice"}\n = 17 bytes
        assert_eq!(lines[1].byte_offset, 17);
        assert_eq!(lines[1].value["name"], "bob");
    }

    #[test]
    fn empty_lines_skipped() {
        let content = "{\"a\":1}\n\n{\"b\":2}\n";
        let lines = parse_jsonl(content, "test.jsonl").unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_number, 1);
        assert_eq!(lines[1].line_number, 3);
    }

    #[test]
    fn bom_stripped() {
        let content = "\u{feff}{\"name\":\"alice\"}\n";
        let lines = parse_jsonl(content, "test.jsonl").unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].value["name"], "alice");
    }

    #[test]
    fn crlf_handling() {
        let content = "{\"a\":1}\r\n{\"b\":2}\r\n";
        let lines = parse_jsonl(content, "test.jsonl").unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].value["a"], 1);
        assert_eq!(lines[0].byte_offset, 0);
        assert_eq!(lines[1].value["b"], 2);
        // {"a":1}\r\n = 7 + 1 + 1 = 9 bytes
        assert_eq!(lines[1].byte_offset, 9);
    }

    #[test]
    fn no_trailing_newline() {
        let content = "{\"a\":1}\n{\"b\":2}";
        let lines = parse_jsonl(content, "test.jsonl").unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn parse_error_with_correct_line_number() {
        let content = "{\"a\":1}\n{bad json}\n{\"b\":2}\n";
        let err = parse_jsonl(content, "test.jsonl").unwrap_err();
        assert!(err.message.starts_with("line 2:"));
    }

    #[test]
    fn schema_extraction() {
        let val = serde_json::json!({"$schema": "https://example.com/s.json", "name": "test"});
        assert_eq!(
            extract_schema_uri(&val).as_deref(),
            Some("https://example.com/s.json")
        );
    }

    #[test]
    fn schema_extraction_missing() {
        let val = serde_json::json!({"name": "test"});
        assert!(extract_schema_uri(&val).is_none());
    }

    #[test]
    fn consistency_all_same() {
        let content = r#"{"$schema":"https://example.com/s.json","a":1}
{"$schema":"https://example.com/s.json","b":2}
"#;
        let lines = parse_jsonl(content, "test.jsonl").unwrap();
        assert!(check_schema_consistency(&lines).is_none());
    }

    #[test]
    fn consistency_mismatch() {
        let content = r#"{"$schema":"https://example.com/s1.json","a":1}
{"$schema":"https://example.com/s2.json","b":2}
"#;
        let lines = parse_jsonl(content, "test.jsonl").unwrap();
        let mismatches = check_schema_consistency(&lines).unwrap();
        assert_eq!(mismatches.len(), 1);
        assert_eq!(mismatches[0].line_number, 2);
        assert_eq!(mismatches[0].schema_uri, "https://example.com/s2.json");
    }

    #[test]
    fn consistency_no_schemas() {
        let content = "{\"a\":1}\n{\"b\":2}\n";
        let lines = parse_jsonl(content, "test.jsonl").unwrap();
        assert!(check_schema_consistency(&lines).is_none());
    }

    #[test]
    fn parse_array_line() {
        let content = "[1,2,3]\n[4,5,6]\n";
        let lines = parse_jsonl(content, "test.jsonl").unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn parse_scalar_line() {
        let content = "42\n\"hello\"\ntrue\nnull\n";
        let lines = parse_jsonl(content, "test.jsonl").unwrap();
        assert_eq!(lines.len(), 4);
    }
}
