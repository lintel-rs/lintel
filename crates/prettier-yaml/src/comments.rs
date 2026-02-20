/// A comment extracted from source text before AST construction.
#[derive(Debug, Clone)]
pub(crate) struct SourceComment {
    pub line: usize,      // 1-indexed line number
    pub col: usize,       // 0-indexed column of the `#`
    pub text: String,     // including the `#`
    pub whole_line: bool, // true if the comment is the only content on this line
}

/// Extract all comments from source text.
///
/// This is a simple heuristic: find `#` characters that are not inside quoted strings.
/// We track whether we're inside a single-quoted or double-quoted string.
pub(crate) fn extract_comments(content: &str) -> Vec<SourceComment> {
    let mut comments = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        if let Some(comment) = find_comment_in_line(line) {
            let whole_line = line[..comment.0].trim().is_empty();
            comments.push(SourceComment {
                line: line_idx + 1,
                col: comment.0,
                text: comment.1.to_string(),
                whole_line,
            });
        }
    }
    comments
}

/// Find a comment in a line, returning (column, text) if found.
/// Handles skipping `#` inside quoted strings.
pub(crate) fn find_comment_in_line(line: &str) -> Option<(usize, &str)> {
    let mut in_single = false;
    let mut in_double = false;
    let mut prev_char = '\0';
    let bytes = line.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        let ch = b as char;
        match ch {
            '\'' if !in_double && prev_char != '\\' => in_single = !in_single,
            '"' if !in_single && prev_char != '\\' => in_double = !in_double,
            '#' if !in_single && !in_double => {
                // A comment `#` must be preceded by a space or be at the start of the line
                if i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t' {
                    return Some((i, &line[i..]));
                }
            }
            _ => {}
        }
        prev_char = ch;
    }
    None
}
