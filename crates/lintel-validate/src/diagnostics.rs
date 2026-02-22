use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

/// Default label text used for span annotations when no specific instance path
/// is available. Checked by reporters to decide whether to show the path suffix.
pub const DEFAULT_LABEL: &str = "here";

/// A parse error with exact source location.
///
/// Used as the error type for the [`Parser`](crate::parsers::Parser) trait.
/// Converted into [`LintError::Parse`] via the `From` impl.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct ParseDiagnostic {
    pub src: NamedSource<String>,
    pub span: SourceSpan,
    pub message: String,
}

/// A single lint error produced during validation.
#[derive(Debug, Error, Diagnostic)]
pub enum LintError {
    #[error("{message}")]
    #[diagnostic(code(lintel::parse))]
    Parse {
        #[source_code]
        src: NamedSource<String>,
        #[label("here")]
        span: SourceSpan,
        message: String,
    },

    #[error("{message}")]
    #[diagnostic(
        code(lintel::validation),
        url("{schema_url}"),
        help("run `lintel explain --file {path}` to see the full schema definition")
    )]
    Validation {
        #[source_code]
        src: NamedSource<String>,
        #[label("{label}")]
        span: SourceSpan,
        #[label("from {schema_url}")]
        schema_span: SourceSpan,
        path: String,
        instance_path: String,
        label: String,
        message: String,
        /// Schema URI this file was validated against (shown as a clickable link
        /// in terminals for remote schemas).
        schema_url: String,
        /// JSON Schema path that triggered the error (e.g. `/properties/jobs/oneOf`).
        schema_path: String,
    },

    /// Validation error for `lintel.toml` against its built-in schema.
    ///
    /// Uses `#[error("{path}: {message}")]` so the path appears even when
    /// rendered as a plain string (e.g. via `to_string()`). The `path` field
    /// mirrors `Validation` for consistency; `src` carries the same value via
    /// `NamedSource` for miette's source-code rendering.
    #[error("{path}: {message}")]
    #[diagnostic(code(lintel::config))]
    Config {
        #[source_code]
        src: NamedSource<String>,
        #[label("{instance_path}")]
        span: SourceSpan,
        path: String,
        instance_path: String,
        message: String,
    },

    #[error("{path}: {message}")]
    #[diagnostic(code(lintel::io))]
    Io { path: String, message: String },

    #[error("{path}: {message}")]
    #[diagnostic(code(lintel::schema::fetch))]
    SchemaFetch { path: String, message: String },

    #[error("{path}: {message}")]
    #[diagnostic(code(lintel::schema::compile))]
    SchemaCompile { path: String, message: String },
}

impl From<ParseDiagnostic> for LintError {
    fn from(d: ParseDiagnostic) -> Self {
        LintError::Parse {
            src: d.src,
            span: d.span,
            message: d.message,
        }
    }
}

impl LintError {
    /// File path associated with this error.
    pub fn path(&self) -> &str {
        match self {
            LintError::Parse { src, .. } => src.name(),
            LintError::Validation { path, .. }
            | LintError::Config { path, .. }
            | LintError::Io { path, .. }
            | LintError::SchemaFetch { path, .. }
            | LintError::SchemaCompile { path, .. } => path,
        }
    }

    /// Human-readable error message.
    pub fn message(&self) -> &str {
        match self {
            LintError::Parse { message, .. }
            | LintError::Validation { message, .. }
            | LintError::Config { message, .. }
            | LintError::Io { message, .. }
            | LintError::SchemaFetch { message, .. }
            | LintError::SchemaCompile { message, .. } => message,
        }
    }

    /// Byte offset in the source file (for sorting).
    pub fn offset(&self) -> usize {
        match self {
            LintError::Parse { span, .. }
            | LintError::Validation { span, .. }
            | LintError::Config { span, .. } => span.offset(),
            LintError::Io { .. }
            | LintError::SchemaFetch { .. }
            | LintError::SchemaCompile { .. } => 0,
        }
    }
}

/// Convert a byte offset into 1-based (line, column).
///
/// Returns `(1, 1)` if the offset is 0 or the content is empty.
pub fn offset_to_line_col(content: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(content.len());
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in content.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Find the byte offset of the first non-comment, non-blank line in the content.
///
/// Skips lines that start with `#` (YAML/TOML comments, modelines) or `//` (JSONC),
/// as well as blank lines. Returns 0 if all lines are comments or the content is empty.
fn first_content_offset(content: &str) -> usize {
    let mut offset = 0;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("//") {
            let key_start = line.len() - trimmed.len();
            return offset + key_start;
        }
        offset += line.len() + 1; // +1 for newline
    }
    0
}

/// Find the byte span `(offset, length)` of a JSON pointer path segment in the
/// source text, suitable for converting directly into a [`SourceSpan`].
///
/// For an `instance_path` like `/properties/name`, searches for the last segment
/// `name` as a JSON key (`"name"`) or YAML key (`name:`), and returns a span
/// covering the matched token.
///
/// For root-level errors (empty or "/" path), skips past leading comment and blank
/// lines so the error arrow points at actual content rather than modeline comments.
/// The returned span has zero length in this case since there is no specific token.
///
/// Falls back to `(0, 0)` if nothing is found.
pub fn find_instance_path_span(content: &str, instance_path: &str) -> (usize, usize) {
    if instance_path.is_empty() || instance_path == "/" {
        return (first_content_offset(content), 0);
    }

    // Get the last path segment (e.g., "/foo/bar/baz" -> "baz")
    let segment = instance_path.rsplit('/').next().unwrap_or("");
    if segment.is_empty() {
        return (0, 0);
    }

    // Try JSON-style key: "segment" — highlight including quotes
    let json_key = format!("\"{segment}\"");
    if let Some(pos) = content.find(&json_key) {
        return (pos, json_key.len());
    }

    // Try YAML-style key: segment: (at line start or after whitespace)
    let yaml_key = format!("{segment}:");
    let quoted_yaml_key = format!("\"{segment}\":");
    let mut offset = 0;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&quoted_yaml_key) {
            let key_start = line.len() - trimmed.len();
            // Highlight the quoted key without the trailing colon
            return (offset + key_start, quoted_yaml_key.len() - 1);
        }
        if trimmed.starts_with(&yaml_key) {
            let key_start = line.len() - trimmed.len();
            // Highlight just the key without the trailing colon
            return (offset + key_start, segment.len());
        }
        offset += line.len() + 1; // +1 for newline
    }

    (0, 0)
}

/// Build a label string combining the instance path and the schema path.
///
/// Returns just the `instance_path` when `schema_path` is empty.
pub fn format_label(instance_path: &str, schema_path: &str) -> String {
    if schema_path.is_empty() {
        instance_path.to_string()
    } else {
        format!("{instance_path} in {schema_path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_zero_returns_line_one_col_one() {
        assert_eq!(offset_to_line_col("hello", 0), (1, 1));
    }

    #[test]
    fn offset_within_first_line() {
        assert_eq!(offset_to_line_col("hello world", 5), (1, 6));
    }

    #[test]
    fn offset_at_second_line() {
        assert_eq!(offset_to_line_col("ab\ncd\nef", 3), (2, 1));
    }

    #[test]
    fn offset_middle_of_second_line() {
        assert_eq!(offset_to_line_col("ab\ncd\nef", 4), (2, 2));
    }

    #[test]
    fn offset_at_third_line() {
        assert_eq!(offset_to_line_col("ab\ncd\nef", 6), (3, 1));
    }

    #[test]
    fn offset_past_end_clamps() {
        assert_eq!(offset_to_line_col("ab\ncd", 100), (2, 3));
    }

    #[test]
    fn empty_content() {
        assert_eq!(offset_to_line_col("", 0), (1, 1));
    }

    #[test]
    fn root_path_skips_yaml_modeline() {
        let content = "# yaml-language-server: $schema=https://example.com/s.json\nname: hello\n";
        let (offset, len) = find_instance_path_span(content, "");
        assert_eq!(offset, 59); // "name: hello" starts at byte 59
        assert_eq!(len, 0); // root-level: no specific token
        assert_eq!(offset_to_line_col(content, offset), (2, 1));
    }

    #[test]
    fn root_path_skips_multiple_comments() {
        let content = "# modeline\n# another comment\n\nname: hello\n";
        let (offset, _) = find_instance_path_span(content, "");
        assert_eq!(offset_to_line_col(content, offset), (4, 1));
    }

    #[test]
    fn root_path_no_comments_returns_zero() {
        let content = "{\"name\": \"hello\"}";
        assert_eq!(find_instance_path_span(content, ""), (0, 0));
    }

    #[test]
    fn root_path_skips_toml_modeline() {
        let content = "# :schema https://example.com/s.json\nname = \"hello\"\n";
        let (offset, _) = find_instance_path_span(content, "");
        assert_eq!(offset_to_line_col(content, offset), (2, 1));
    }

    #[test]
    fn root_path_slash_skips_comments() {
        let content = "# yaml-language-server: $schema=url\ndata: value\n";
        let (offset, _) = find_instance_path_span(content, "/");
        assert_eq!(offset_to_line_col(content, offset), (2, 1));
    }

    #[test]
    fn span_highlights_json_key() {
        let content = r#"{"name": "hello", "age": 30}"#;
        assert_eq!(find_instance_path_span(content, "/name"), (1, 6)); // "name"
        assert_eq!(find_instance_path_span(content, "/age"), (18, 5)); // "age"
    }

    #[test]
    fn span_highlights_yaml_key() {
        let content = "name: hello\nage: 30\n";
        assert_eq!(find_instance_path_span(content, "/name"), (0, 4)); // name
        assert_eq!(find_instance_path_span(content, "/age"), (12, 3)); // age
    }

    #[test]
    fn span_highlights_quoted_yaml_key() {
        let content = "\"on\": push\n";
        assert_eq!(find_instance_path_span(content, "/on"), (0, 4)); // "on"
    }

    // --- Error code tests ---

    #[test]
    fn error_codes() {
        use miette::Diagnostic;

        let cases: Vec<(LintError, &str)> = vec![
            (
                LintError::Parse {
                    src: NamedSource::new("f", String::new()),
                    span: 0.into(),
                    message: String::new(),
                },
                "lintel::parse",
            ),
            (
                LintError::Validation {
                    src: NamedSource::new("f", String::new()),
                    span: 0.into(),
                    schema_span: 0.into(),
                    path: String::new(),
                    instance_path: String::new(),
                    label: String::new(),
                    message: String::new(),
                    schema_url: String::new(),
                    schema_path: String::new(),
                },
                "lintel::validation",
            ),
            (
                LintError::Config {
                    src: NamedSource::new("f", String::new()),
                    span: 0.into(),
                    path: String::new(),
                    instance_path: String::new(),
                    message: String::new(),
                },
                "lintel::config",
            ),
            (
                LintError::Io {
                    path: String::new(),
                    message: String::new(),
                },
                "lintel::io",
            ),
            (
                LintError::SchemaFetch {
                    path: String::new(),
                    message: String::new(),
                },
                "lintel::schema::fetch",
            ),
            (
                LintError::SchemaCompile {
                    path: String::new(),
                    message: String::new(),
                },
                "lintel::schema::compile",
            ),
        ];

        for (error, expected_code) in cases {
            assert_eq!(
                error.code().expect("missing diagnostic code").to_string(),
                expected_code,
                "wrong code for {error:?}"
            );
        }
    }

    // --- format_label tests ---

    #[test]
    fn format_label_with_schema_path() {
        assert_eq!(
            format_label(
                "/jobs/build",
                "/properties/jobs/patternProperties/^[_a-zA-Z][a-zA-Z0-9_-]*$/oneOf"
            ),
            "/jobs/build in /properties/jobs/patternProperties/^[_a-zA-Z][a-zA-Z0-9_-]*$/oneOf"
        );
    }

    #[test]
    fn format_label_empty_schema_path() {
        assert_eq!(format_label("/name", ""), "/name");
    }
}
