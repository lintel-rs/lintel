/// Parsed frontmatter from a markdown document.
pub struct Frontmatter<'a> {
    /// The content between the delimiters (without delimiters).
    pub content: &'a str,
    /// If YAML (plain `---`), this is None. If `---toml` etc, this is `Some("toml")`.
    pub language: Option<&'a str>,
    /// The remaining body after the closing delimiter.
    pub body: &'a str,
}

/// Extract frontmatter from a markdown document.
///
/// Supports YAML (`---` ... `---`) and custom parser frontmatter
/// (`---toml` ... `---`, `---mycustomparser` ... `---`).
///
/// Returns `None` if no frontmatter is present.
pub fn extract_frontmatter(content: &str) -> Option<Frontmatter<'_>> {
    // Frontmatter must start at the very beginning with `---`
    let rest = content.strip_prefix("---")?;

    // Check for custom language tag (e.g., `---toml\n`, `---mycustomparser\n`)
    // vs plain YAML (`---\n`)
    let (language, rest) = if let Some(after_newline) = rest.strip_prefix('\n') {
        (None, after_newline)
    } else if let Some(after_newline) = rest.strip_prefix("\r\n") {
        (None, after_newline)
    } else if rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
        // Custom language: `---toml\n`
        let newline_pos = rest.find('\n')?;
        let lang = rest[..newline_pos].trim_end_matches('\r');
        let after = &rest[newline_pos + 1..];
        (Some(lang), after)
    } else {
        return None;
    };

    // Find the closing `---` on its own line
    let mut search_pos = 0;
    loop {
        let remaining = &rest[search_pos..];
        let idx = remaining.find("---")?;
        let abs_idx = search_pos + idx;

        // Check that `---` is at the start of a line
        let at_line_start = abs_idx == 0 || rest.as_bytes()[abs_idx - 1] == b'\n';
        if !at_line_start {
            search_pos = abs_idx + 3;
            continue;
        }

        // Check that `---` is followed by optional whitespace, then a newline or EOF
        let after = &rest[abs_idx + 3..];
        let trimmed = after
            .strip_prefix(|c: char| c == ' ' || c == '\t')
            .map_or(after, |_| {
                let ws_end = after.len() - after.trim_start_matches([' ', '\t']).len();
                &after[ws_end..]
            });
        let valid_end =
            trimmed.is_empty() || trimmed.starts_with('\n') || trimmed.starts_with("\r\n");
        if !valid_end {
            search_pos = abs_idx + 3;
            continue;
        }

        let fm_content = &rest[..abs_idx];
        let body = if let Some(stripped) = trimmed.strip_prefix('\n') {
            stripped
        } else if let Some(stripped) = trimmed.strip_prefix("\r\n") {
            stripped
        } else {
            trimmed
        };

        return Some(Frontmatter {
            content: fm_content,
            language,
            body,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_frontmatter() {
        let input = "---\ntitle: Hello\n---\n# Heading\n";
        let fm = extract_frontmatter(input).expect("should find frontmatter");
        assert_eq!(fm.content, "title: Hello\n");
        assert!(fm.language.is_none());
        assert_eq!(fm.body, "# Heading\n");
    }

    #[test]
    fn no_frontmatter() {
        let input = "# Heading\nSome text\n";
        assert!(extract_frontmatter(input).is_none());
    }

    #[test]
    fn frontmatter_not_at_start() {
        let input = "\n---\ntitle: Hello\n---\n";
        assert!(extract_frontmatter(input).is_none());
    }

    #[test]
    fn empty_frontmatter() {
        let input = "---\n---\nBody\n";
        let fm = extract_frontmatter(input).expect("should find frontmatter");
        assert_eq!(fm.content, "");
        assert_eq!(fm.body, "Body\n");
    }

    #[test]
    fn frontmatter_with_dashes_in_content() {
        let input = "---\ntitle: foo---bar\n---\nBody\n";
        let fm = extract_frontmatter(input).expect("should find frontmatter");
        assert_eq!(fm.content, "title: foo---bar\n");
        assert_eq!(fm.body, "Body\n");
    }

    #[test]
    fn frontmatter_no_trailing_body() {
        let input = "---\ntitle: Hello\n---";
        let fm = extract_frontmatter(input).expect("should find frontmatter");
        assert_eq!(fm.content, "title: Hello\n");
        assert_eq!(fm.body, "");
    }

    #[test]
    fn custom_language_frontmatter() {
        let input = "---toml\ntitle = \"Hello\"\n---\nBody\n";
        let fm = extract_frontmatter(input).expect("should find frontmatter");
        assert_eq!(fm.content, "title = \"Hello\"\n");
        assert_eq!(fm.language, Some("toml"));
        assert_eq!(fm.body, "Body\n");
    }

    #[test]
    fn custom_parser_frontmatter() {
        let input = "---mycustomparser\n- hello:    world\n-         123\n---\n\n__123__\n";
        let fm = extract_frontmatter(input).expect("should find frontmatter");
        assert_eq!(fm.language, Some("mycustomparser"));
        assert_eq!(fm.content, "- hello:    world\n-         123\n");
    }
}
