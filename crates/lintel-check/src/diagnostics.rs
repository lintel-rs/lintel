use miette::{Diagnostic, LabeledSpan, NamedSource, SourceSpan};
use thiserror::Error;

/// A parse error with exact source location.
#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
pub struct ParseDiagnostic {
    #[source_code]
    pub src: NamedSource<String>,

    #[label("here")]
    pub span: SourceSpan,

    pub message: String,
}

/// A schema validation error for a specific file.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct ValidationDiagnostic {
    pub src: NamedSource<String>,

    pub span: SourceSpan,

    pub path: String,

    pub instance_path: String,

    pub message: String,
}

impl Diagnostic for ValidationDiagnostic {
    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        let label = if self.instance_path.is_empty() {
            "here".to_string()
        } else {
            self.instance_path.clone()
        };
        Some(Box::new(core::iter::once(LabeledSpan::new(
            Some(label),
            self.span.offset(),
            self.span.len(),
        ))))
    }
}

/// An I/O or schema-fetch error associated with a file path.
#[derive(Debug, Error, Diagnostic)]
#[error("{path}: {message}")]
pub struct FileDiagnostic {
    pub path: String,
    pub message: String,
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

/// Try to find the byte offset of a JSON pointer path segment in the source text.
///
/// For an `instance_path` like `/properties/name`, searches for the last segment `name`
/// as a JSON key (`"name"`) or YAML key (`name:`). Falls back to 0 if not found.
pub fn find_instance_path_offset(content: &str, instance_path: &str) -> usize {
    if instance_path.is_empty() || instance_path == "/" {
        return 0;
    }

    // Get the last path segment (e.g., "/foo/bar/baz" -> "baz")
    let segment = instance_path.rsplit('/').next().unwrap_or("");
    if segment.is_empty() {
        return 0;
    }

    // Try JSON-style key: "segment"
    let json_key = format!("\"{segment}\"");
    if let Some(pos) = content.find(&json_key) {
        return pos;
    }

    // Try YAML-style key: segment: (at line start or after whitespace)
    let yaml_key = format!("{segment}:");
    let quoted_yaml_key = format!("\"{segment}\":");
    let mut offset = 0;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&yaml_key) || trimmed.starts_with(&quoted_yaml_key) {
            let key_start = line.len() - trimmed.len();
            return offset + key_start;
        }
        offset += line.len() + 1; // +1 for newline
    }

    0
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
}
