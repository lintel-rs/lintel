/// Word-wrap text to fit within `width` visible columns, respecting ANSI escape sequences.
///
/// - `first_line_offset`: columns already occupied on the first line (e.g. 2 for `- ` prefix).
/// - `cont_indent`: number of spaces to prepend to each continuation line (for alignment).
///
/// ANSI CSI (`\x1b[...letter`) and OSC (`\x1b]...ST`) sequences are treated as
/// zero-width and kept with the adjacent word. Existing newlines are preserved.
pub(crate) fn wrap_text(
    text: &str,
    width: usize,
    first_line_offset: usize,
    cont_indent: usize,
) -> String {
    if width == 0 {
        return text.to_string();
    }

    let cont_prefix: String = " ".repeat(cont_indent);
    let mut out = String::new();
    let mut col = first_line_offset;
    let mut word = String::new();
    let mut word_width = 0;
    let mut pending_space = false;
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        if ch == '\x1b' && i + 1 < len {
            i = consume_ansi_escape(&chars, i, &mut word);
            continue;
        }

        if ch == '\n' {
            flush_word(
                &mut out,
                col,
                &mut word,
                &mut word_width,
                &mut pending_space,
                width,
                &cont_prefix,
            );
            out.push('\n');
            if cont_indent > 0 {
                out.push_str(&cont_prefix);
            }
            col = cont_indent;
            pending_space = false;
            i += 1;
            continue;
        }

        if ch == ' ' {
            col = flush_word(
                &mut out,
                col,
                &mut word,
                &mut word_width,
                &mut pending_space,
                width,
                &cont_prefix,
            );
            if col > 0 {
                pending_space = true;
            }
            i += 1;
            continue;
        }

        word.push(ch);
        word_width += 1;
        i += 1;
    }

    flush_word(
        &mut out,
        col,
        &mut word,
        &mut word_width,
        &mut pending_space,
        width,
        &cont_prefix,
    );

    out
}

/// Flush the current word to `out`, emitting a space or line break before it
/// when `pending_space` is set. Returns the updated column position.
fn flush_word(
    out: &mut String,
    col: usize,
    word: &mut String,
    word_width: &mut usize,
    pending_space: &mut bool,
    width: usize,
    cont_prefix: &str,
) -> usize {
    if word.is_empty() {
        return col;
    }
    let mut c = col;
    if *pending_space {
        if c + 1 + *word_width > width {
            out.push('\n');
            out.push_str(cont_prefix);
            c = cont_prefix.len();
        } else {
            out.push(' ');
            c += 1;
        }
        *pending_space = false;
    }
    out.push_str(word);
    c += *word_width;
    word.clear();
    *word_width = 0;
    c
}

/// Consume an ANSI escape sequence starting at `chars[i]` (the `\x1b`),
/// appending it to `word`. Returns the new index past the sequence.
fn consume_ansi_escape(chars: &[char], mut i: usize, word: &mut String) -> usize {
    let len = chars.len();
    word.push(chars[i]); // \x1b
    i += 1;
    let next = chars[i];
    word.push(next);
    i += 1;
    if next == '[' {
        // CSI: consume until ASCII letter
        while i < len {
            word.push(chars[i]);
            let done = chars[i].is_ascii_alphabetic();
            i += 1;
            if done {
                break;
            }
        }
    } else if next == ']' {
        // OSC: consume until ST (\x1b\\) or BEL
        while i < len {
            if chars[i] == '\x07' {
                word.push(chars[i]);
                i += 1;
                break;
            }
            if chars[i] == '\x1b' && i + 1 < len && chars[i + 1] == '\\' {
                word.push(chars[i]);
                word.push(chars[i + 1]);
                i += 2;
                break;
            }
            word.push(chars[i]);
            i += 1;
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    use ansi_term_codes::{BOLD, RESET};

    #[test]
    fn basic() {
        let result = wrap_text("hello world foo bar baz", 10, 0, 0);
        assert_eq!(result, "hello\nworld foo\nbar baz");
    }

    #[test]
    fn with_prefix_offset() {
        // Simulate "- " prefix taking 2 columns. Width=12, so 10 available on first line.
        let result = wrap_text("first second third", 12, 2, 2);
        assert_eq!(result, "first\n  second\n  third");
    }

    #[test]
    fn preserves_ansi() {
        let input = format!("{BOLD}hello{RESET} world");
        let result = wrap_text(&input, 80, 0, 0);
        assert!(result.contains(BOLD));
        assert!(result.contains("hello"));
        assert!(result.contains("world"));
    }

    #[test]
    fn no_wrap_when_fits() {
        let result = wrap_text("short text", 80, 0, 0);
        assert_eq!(result, "short text");
    }
}
