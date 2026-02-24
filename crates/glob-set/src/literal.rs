/// Extract the longest contiguous literal substring from a glob pattern.
///
/// A literal substring is one that appears in the "fixed" (non-variable) part
/// of the pattern. Character classes (`[...]`), alternation groups (`{...}`),
/// escaped characters, wildcards, and `**/` tokens are all treated as
/// non-literal boundaries.
///
/// Returns `None` if no literal substring of length >= 1 exists.
pub(crate) fn extract_literal(pattern: &str) -> Option<&str> {
    let bytes = pattern.as_bytes();
    let mut best = 0..0_usize;
    let mut cur_start = 0;
    let mut cur_len = 0;
    let mut i = 0;

    macro_rules! flush {
        () => {
            if cur_len > best.len() {
                best = cur_start..cur_start + cur_len;
            }
        };
    }

    while i < bytes.len() {
        match bytes[i] {
            b'*' => {
                flush!();
                let start = i;
                while i < bytes.len() && bytes[i] == b'*' {
                    i += 1;
                }
                // `**/` is a single optional token -- skip the `/` too.
                if i - start >= 2 && i < bytes.len() && bytes[i] == b'/' {
                    i += 1;
                }
                cur_start = i;
                cur_len = 0;
            }
            b'?' => {
                flush!();
                i += 1;
                cur_start = i;
                cur_len = 0;
            }
            b'[' => {
                flush!();
                i = skip_char_class(bytes, i);
                cur_start = i;
                cur_len = 0;
            }
            b'{' => {
                flush!();
                i = skip_braces(bytes, i);
                cur_start = i;
                cur_len = 0;
            }
            b'\\' => {
                flush!();
                i += 2; // backslash + escaped char
                cur_start = i;
                cur_len = 0;
            }
            _ => {
                cur_len += 1;
                i += 1;
            }
        }
    }

    flush!();
    if best.is_empty() {
        None
    } else {
        Some(&pattern[best])
    }
}

/// Advance past a `[...]` character class starting at `bytes[i] == b'['`.
fn skip_char_class(bytes: &[u8], mut i: usize) -> usize {
    i += 1; // skip `[`
    if i < bytes.len() && matches!(bytes[i], b'^' | b'!') {
        i += 1;
    }
    // `]` as first char in class is literal, not a close.
    if i < bytes.len() && bytes[i] == b']' {
        i += 1;
    }
    while i < bytes.len() && bytes[i] != b']' {
        if bytes[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    if i < bytes.len() {
        i += 1; // skip `]`
    }
    i
}

/// Advance past a `{...}` alternation group (potentially nested).
fn skip_braces(bytes: &[u8], mut i: usize) -> usize {
    let mut depth = 1u32;
    i += 1; // skip `{`
    while i < bytes.len() && depth > 0 {
        match bytes[i] {
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn simple_literal() {
        assert_eq!(extract_literal("foo.txt"), Some("foo.txt"));
    }

    #[test]
    fn star_pattern() {
        assert_eq!(extract_literal("*.txt"), Some(".txt"));
    }

    #[test]
    fn globstar_pattern() {
        assert_eq!(extract_literal("**/*.rs"), Some(".rs"));
    }

    #[test]
    fn braces_pattern() {
        assert_eq!(extract_literal("test.{js,ts}"), Some("test."));
    }

    #[test]
    fn char_class_pattern() {
        assert_eq!(extract_literal("[abc].txt"), Some(".txt"));
    }

    #[test]
    fn longest_literal() {
        assert_eq!(extract_literal("*.longer_suffix"), Some(".longer_suffix"));
    }

    #[test]
    fn all_wildcards() {
        assert_eq!(extract_literal("*"), None);
        assert_eq!(extract_literal("**"), None);
        assert_eq!(extract_literal("?"), None);
        assert_eq!(extract_literal("**/*"), None);
    }

    #[test]
    fn escaped_chars() {
        assert_eq!(extract_literal("a\\*b"), Some("a"));
    }

    #[test]
    fn empty_pattern() {
        assert_eq!(extract_literal(""), None);
    }

    #[test]
    fn path_with_globstar() {
        assert_eq!(extract_literal("src/**/*.test.js"), Some(".test.js"));
    }

    #[test]
    fn globstar_preserves_prefix() {
        assert_eq!(extract_literal("src/**"), Some("src/"));
    }
}
