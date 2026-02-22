/// Mutable state carried through the word-wrapping loop.
struct WrapState {
    col: usize,
    word: String,
    word_width: usize,
    pending_space: bool,
}

impl WrapState {
    /// Flush the current word to `out`, emitting a space or line break before it
    /// when `pending_space` is set. Updates `self.col` in place.
    fn flush_word(&mut self, out: &mut String, width: usize, cont_prefix: &str) {
        if self.word.is_empty() {
            return;
        }
        if self.pending_space {
            if self.col + 1 + self.word_width > width {
                out.push('\n');
                out.push_str(cont_prefix);
                self.col = cont_prefix.len();
            } else {
                out.push(' ');
                self.col += 1;
            }
            self.pending_space = false;
        }
        out.push_str(&self.word);
        self.col += self.word_width;
        self.word.clear();
        self.word_width = 0;
    }
}

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
    let mut state = WrapState {
        col: first_line_offset,
        word: String::new(),
        word_width: 0,
        pending_space: false,
    };
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        if ch == '\x1b' && i + 1 < len {
            i = consume_ansi_escape(&chars, i, &mut state.word);
            continue;
        }

        if ch == '\n' {
            state.flush_word(&mut out, width, &cont_prefix);
            out.push('\n');
            if cont_indent > 0 {
                out.push_str(&cont_prefix);
            }
            state.col = cont_indent;
            state.pending_space = false;
            i += 1;
            continue;
        }

        if ch == ' ' {
            state.flush_word(&mut out, width, &cont_prefix);
            if state.col > 0 {
                state.pending_space = true;
            }
            i += 1;
            continue;
        }

        state.word.push(ch);
        state.word_width += 1;
        i += 1;
    }

    state.flush_word(&mut out, width, &cont_prefix);

    out
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
