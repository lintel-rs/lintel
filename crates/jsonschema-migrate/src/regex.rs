/// Normalize an ECMA 262 regex pattern for compatibility with Rust's `regex_syntax`.
///
/// Two incompatibilities are fixed:
///
/// 1. **Bare braces**: Unescaped `{` and `}` that do not form valid quantifiers
///    (`{n}`, `{n,}`, `{n,m}`) are escaped. ECMA 262 treats unmatched braces as
///    literals, but `regex_syntax` rejects them.
///
/// 2. **`\d` in character classes**: `\d` inside `[…]` is expanded to `0-9`.
///    This prevents `regex_syntax` from rejecting patterns where `\d` appears as
///    a range endpoint (e.g. `[\d-\.]`), and ensures ASCII-only digit matching
///    consistent with ECMA 262 semantics.
#[allow(clippy::missing_panics_doc)] // from_utf8 cannot panic on our output
pub fn normalize_ecma_regex(pattern: &str) -> String {
    let b = pattern.as_bytes();
    let valid_braces = find_valid_quantifier_braces(b);
    let mut out = Vec::with_capacity(b.len() + 16);
    let mut i = 0;
    let mut in_class = false;

    while i < b.len() {
        // Handle escape sequences
        if b[i] == b'\\' && i + 1 < b.len() {
            let next = b[i + 1];

            // Expand \d → 0-9 inside character classes
            if in_class && next == b'd' {
                out.extend_from_slice(b"0-9");
                i += 2;
                continue;
            }

            // Pass through Unicode escapes: \p{...}, \P{...}, \u{...}
            if matches!(next, b'p' | b'P' | b'u') && i + 2 < b.len() && b[i + 2] == b'{' {
                out.push(b'\\');
                out.push(next);
                i += 2;
                if let Some(close) = b[i..].iter().position(|&c| c == b'}') {
                    out.extend_from_slice(&b[i..=i + close]);
                    i += close + 1;
                }
                continue;
            }

            out.push(b[i]);
            out.push(next);
            i += 2;
            continue;
        }

        // Character class start
        if b[i] == b'[' && !in_class {
            in_class = true;
            out.push(b'[');
            i += 1;
            // Skip negation and literal ] at class start
            if i < b.len() && b[i] == b'^' {
                out.push(b'^');
                i += 1;
            }
            if i < b.len() && b[i] == b']' {
                out.push(b']');
                i += 1;
            }
            continue;
        }
        if b[i] == b']' && in_class {
            in_class = false;
            out.push(b']');
            i += 1;
            continue;
        }

        // Inside character class, everything is literal (no brace escaping needed)
        if in_class {
            out.push(b[i]);
            i += 1;
            continue;
        }

        // Escape bare braces outside character class
        if b[i] == b'{' && !valid_braces[i] {
            out.extend_from_slice(b"\\{");
            i += 1;
            continue;
        }
        if b[i] == b'}' && !valid_braces[i] {
            out.extend_from_slice(b"\\}");
            i += 1;
            continue;
        }

        out.push(b[i]);
        i += 1;
    }

    // Safety: input is valid UTF-8 and we only replace/insert ASCII bytes.
    // Non-ASCII bytes (≥ 128) never match our ASCII comparisons, so multi-byte
    // UTF-8 sequences pass through unchanged.
    String::from_utf8(out).expect("normalization preserves UTF-8")
}

/// Identify positions of `{` and `}` that form valid quantifiers.
fn find_valid_quantifier_braces(b: &[u8]) -> Vec<bool> {
    let mut valid = vec![false; b.len()];
    let mut i = 0;
    let mut in_class = false;

    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            // Skip Unicode escapes: \p{...}, \P{...}, \u{...}
            if matches!(b[i + 1], b'p' | b'P' | b'u') && i + 2 < b.len() && b[i + 2] == b'{' {
                i += 2;
                if let Some(close) = b[i..].iter().position(|&c| c == b'}') {
                    i += close + 1;
                }
                continue;
            }
            i += 2;
            continue;
        }
        if b[i] == b'[' && !in_class {
            in_class = true;
            i += 1;
            continue;
        }
        if b[i] == b']' && in_class {
            in_class = false;
            i += 1;
            continue;
        }
        if b[i] == b'{'
            && !in_class
            && let Some(end) = parse_quantifier(b, i)
        {
            valid[i] = true;
            valid[end] = true;
            i = end + 1;
            continue;
        }
        i += 1;
    }

    valid
}

/// Check if `b[start]` (which must be `{`) begins a valid quantifier.
///
/// Returns the index of the closing `}` if valid, or `None` otherwise.
/// Valid forms: `{n}`, `{n,}`, `{n,m}` where n and m are non-negative integers.
fn parse_quantifier(b: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;
    // First number (required)
    let n_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == n_start || i >= b.len() {
        return None;
    }
    if b[i] == b'}' {
        return Some(i); // {n}
    }
    if b[i] != b',' {
        return None;
    }
    i += 1; // skip comma
    if i >= b.len() {
        return None;
    }
    if b[i] == b'}' {
        return Some(i); // {n,}
    }
    // Second number
    let n_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == n_start || i >= b.len() {
        return None;
    }
    if b[i] == b'}' {
        return Some(i); // {n,m}
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_braces_escaped() {
        assert_eq!(
            normalize_ecma_regex(r"^{?[a-zA-Z0-9-_.@#\s]+}?$"),
            r"^\{?[a-zA-Z0-9-_.@#\s]+\}?$"
        );
    }

    #[test]
    fn valid_quantifier_preserved() {
        assert_eq!(normalize_ecma_regex(r"^[0-9a-f]{40}$"), r"^[0-9a-f]{40}$");
        assert_eq!(normalize_ecma_regex(r"\d{1,3}"), r"\d{1,3}");
        assert_eq!(normalize_ecma_regex(r"x{2,}y"), r"x{2,}y");
    }

    #[test]
    fn escaped_braces_preserved() {
        assert_eq!(normalize_ecma_regex(r"\{\{.*\}\}"), r"\{\{.*\}\}");
    }

    #[test]
    fn backslash_d_expanded_in_class() {
        assert_eq!(
            normalize_ecma_regex(r"^[a-z][a-z\d-\.]*[a-z\d]$"),
            r"^[a-z][a-z0-9-\.]*[a-z0-9]$"
        );
    }

    #[test]
    fn backslash_d_preserved_outside_class() {
        assert_eq!(normalize_ecma_regex(r"^\d+$"), r"^\d+$");
    }

    #[test]
    fn idempotent() {
        let patterns = [
            r"^[a-z][a-z0-9-\.]*[a-z0-9]$",
            r"^\{?[a-zA-Z0-9]+\}?$",
            r"^[0-9a-f]{40}$",
            r"^\d{1,3}\.\d{1,3}$",
        ];
        for pat in patterns {
            assert_eq!(normalize_ecma_regex(pat), pat, "not idempotent: {pat}");
        }
    }

    #[test]
    fn combined_braces_and_class() {
        assert_eq!(normalize_ecma_regex(r"^{[\d-\.]+}$"), r"^\{[0-9-\.]+\}$");
    }

    #[test]
    fn unicode_property_escapes_preserved() {
        assert_eq!(
            normalize_ecma_regex(r"^(\p{L}|_)(\p{L}|\p{N}|[.\-_])*$"),
            r"^(\p{L}|_)(\p{L}|\p{N}|[.\-_])*$"
        );
        assert_eq!(normalize_ecma_regex(r"\P{Lu}"), r"\P{Lu}");
        assert_eq!(normalize_ecma_regex(r"\u{1f600}"), r"\u{1f600}");
    }

    #[test]
    fn normalized_patterns_parse_with_regex_syntax() {
        use regex_syntax::ast::parse::Parser;

        let patterns = [
            r"^{?[a-zA-Z0-9-_.@#\s]+}?$",
            r"^[a-z][a-z\d-\.]*[a-z\d]$",
            r"\$\{\{\s*(.*?)\s*\}\}",
            r"\$\{\{\s*(.*?)\s*\}\}|(?:\d{1,3}\.){3}\d{1,3}(?:\/\d\d?)?,?",
            r#"\"?\{\{(\$)?([a-z0-9\-]+)\}\}\"?"#,
            r"^(\p{L}|_)(\p{L}|\p{N}|[.\-_])*$",
        ];
        for pat in patterns {
            let norm = normalize_ecma_regex(pat);
            let result = Parser::new().parse(&norm);
            assert!(
                result.is_ok(),
                "pattern {norm:?} failed to parse: {}",
                result.expect_err("unreachable")
            );
        }
    }
}
