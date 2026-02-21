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
/// We track whether we're inside a single-quoted or double-quoted string across lines,
/// since YAML quoted strings can span multiple lines.
pub(crate) fn extract_comments(content: &str) -> Vec<SourceComment> {
    let mut comments = Vec::new();
    let mut in_single = false;
    let mut in_double = false;
    for (line_idx, line) in content.lines().enumerate() {
        if let Some((comment, new_in_single, new_in_double)) =
            find_comment_in_line_with_state(line, in_single, in_double)
        {
            in_single = new_in_single;
            in_double = new_in_double;
            let whole_line = line[..comment.0].trim().is_empty();
            comments.push(SourceComment {
                line: line_idx + 1,
                col: comment.0,
                text: comment.1.to_string(),
                whole_line,
            });
        } else {
            // No comment found — update quote state for next line
            update_quote_state(line, &mut in_single, &mut in_double);
        }
    }
    comments
}

/// Find a comment in a line with existing quote state.
/// Returns ((column, text), `new_in_single`, `new_in_double`) if a comment is found.
fn find_comment_in_line_with_state(
    line: &str,
    mut in_single: bool,
    mut in_double: bool,
) -> Option<((usize, &str), bool, bool)> {
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
                    return Some(((i, &line[i..]), in_single, in_double));
                }
            }
            _ => {}
        }
        prev_char = ch;
    }
    None
}

/// Update quote state after processing a full line (no comment found).
fn update_quote_state(line: &str, in_single: &mut bool, in_double: &mut bool) {
    let mut prev_char = '\0';
    for &b in line.as_bytes() {
        let ch = b as char;
        match ch {
            '\'' if !*in_double && prev_char != '\\' => *in_single = !*in_single,
            '"' if !*in_single && prev_char != '\\' => *in_double = !*in_double,
            _ => {}
        }
        prev_char = ch;
    }
}
