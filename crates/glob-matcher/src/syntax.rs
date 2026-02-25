/// Advance past a `[...]` character class in a glob pattern.
///
/// `i` must point at the opening `[`. Returns the index immediately after
/// the closing `]`. If no closing `]` is found, returns `pattern.len()`.
///
/// Handles negation (`[^...]`, `[!...]`), `]` as the first character in
/// the class (which is literal, not a close), and backslash escapes.
///
/// # Example
///
/// ```
/// assert_eq!(glob_matcher::skip_char_class(b"[abc]xyz", 0), 5);
/// assert_eq!(glob_matcher::skip_char_class(b"[!a-z]_", 0), 6);
/// assert_eq!(glob_matcher::skip_char_class(b"[]]end", 0), 3);
/// ```
pub fn skip_char_class(pattern: &[u8], mut i: usize) -> usize {
    debug_assert!(i < pattern.len() && pattern[i] == b'[');
    i += 1; // skip `[`
    // Skip negation prefix.
    if i < pattern.len() && matches!(pattern[i], b'^' | b'!') {
        i += 1;
    }
    // `]` as first character in class is literal, not a close.
    if i < pattern.len() && pattern[i] == b']' {
        i += 1;
    }
    while i < pattern.len() && pattern[i] != b']' {
        if pattern[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    if i < pattern.len() {
        i += 1; // skip `]`
    }
    i
}

/// Advance past a `{...}` alternation group in a glob pattern.
///
/// `i` must point at the opening `{`. Returns the index immediately after
/// the closing `}`. Handles nested braces, `[...]` character classes
/// (so `}` inside `[}]` is not treated as a brace terminator), and
/// backslash escapes.
///
/// If no matching `}` is found, returns `pattern.len()`.
///
/// # Example
///
/// ```
/// assert_eq!(glob_matcher::skip_braces(b"{a,b}xyz", 0), 5);
/// assert_eq!(glob_matcher::skip_braces(b"{a,{b,c}}", 0), 9);
/// assert_eq!(glob_matcher::skip_braces(b"{[}],foo}", 0), 9);
/// ```
pub fn skip_braces(pattern: &[u8], mut i: usize) -> usize {
    debug_assert!(i < pattern.len() && pattern[i] == b'{');
    let mut depth = 1u32;
    i += 1; // skip `{`
    while i < pattern.len() && depth > 0 {
        match pattern[i] {
            b'[' => {
                i = skip_char_class(pattern, i);
                continue;
            }
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b'\\' => {
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- skip_char_class tests --

    #[test]
    fn skip_char_class_simple() {
        assert_eq!(skip_char_class(b"[abc]", 0), 5);
    }

    #[test]
    fn skip_char_class_range() {
        assert_eq!(skip_char_class(b"[a-z]", 0), 5);
    }

    #[test]
    fn skip_char_class_negated_caret() {
        assert_eq!(skip_char_class(b"[^abc]", 0), 6);
    }

    #[test]
    fn skip_char_class_negated_bang() {
        assert_eq!(skip_char_class(b"[!abc]", 0), 6);
    }

    #[test]
    fn skip_char_class_close_first() {
        // `]` as first char is literal, not a close.
        assert_eq!(skip_char_class(b"[]]", 0), 3);
    }

    #[test]
    fn skip_char_class_close_first_negated() {
        assert_eq!(skip_char_class(b"[^]]", 0), 4);
    }

    #[test]
    fn skip_char_class_escaped() {
        assert_eq!(skip_char_class(b"[a\\]b]", 0), 6);
    }

    #[test]
    fn skip_char_class_with_trailing() {
        assert_eq!(skip_char_class(b"[abc]xyz", 0), 5);
    }

    #[test]
    fn skip_char_class_offset() {
        assert_eq!(skip_char_class(b"xx[abc]yy", 2), 7);
    }

    #[test]
    fn skip_char_class_unclosed() {
        assert_eq!(skip_char_class(b"[abc", 0), 4);
    }

    #[test]
    fn skip_char_class_empty_negated() {
        // `[^]` — `^` is negation, `]` is first so literal, then unclosed
        assert_eq!(skip_char_class(b"[^]", 0), 3);
    }

    #[test]
    fn skip_char_class_brace_inside() {
        // `}` inside char class is just a regular char
        assert_eq!(skip_char_class(b"[}]", 0), 3);
    }

    #[test]
    fn skip_char_class_special_chars() {
        assert_eq!(skip_char_class(b"[*?{]", 0), 5);
    }

    // -- skip_braces tests --

    #[test]
    fn skip_braces_simple() {
        assert_eq!(skip_braces(b"{a,b}", 0), 5);
    }

    #[test]
    fn skip_braces_nested() {
        assert_eq!(skip_braces(b"{a,{b,c}}", 0), 9);
    }

    #[test]
    fn skip_braces_with_brackets() {
        // `[}]` inside braces — the `}` is inside a char class, not a brace close
        assert_eq!(skip_braces(b"{[}],foo}", 0), 9);
    }

    #[test]
    fn skip_braces_with_trailing() {
        assert_eq!(skip_braces(b"{a,b}xyz", 0), 5);
    }

    #[test]
    fn skip_braces_offset() {
        assert_eq!(skip_braces(b"xx{a,b}yy", 2), 7);
    }

    #[test]
    fn skip_braces_escaped() {
        assert_eq!(skip_braces(b"{a,\\},c}", 0), 8);
    }

    #[test]
    fn skip_braces_unclosed() {
        assert_eq!(skip_braces(b"{a,b", 0), 4);
    }

    #[test]
    fn skip_braces_empty() {
        assert_eq!(skip_braces(b"{}", 0), 2);
    }

    #[test]
    fn skip_braces_complex_bracket() {
        // Nested braces with char class containing `}` and `,`
        assert_eq!(skip_braces(b"{a,[},]x,b}", 0), 11);
    }

    #[test]
    fn skip_braces_bracket_negated_close() {
        assert_eq!(skip_braces(b"{[^}],x}", 0), 8);
    }

    #[test]
    fn skip_braces_bracket_first_close() {
        // `[]]` inside braces — `]` as first char is literal
        assert_eq!(skip_braces(b"{[]],x}", 0), 7);
    }
}
