pub mod parser;

use core::fmt::Write;

use anyhow::Result;

use parser::{Comment, Key, Node, Quote};
pub use prettier_config::PrettierConfig;
use prettier_config::{QuoteProps, TrailingComma};
use wadler_lindig::{Doc, force_group_break, trim_trailing_whitespace};

/// Format JSON5 content, preserving comments.
///
/// # Errors
///
/// Returns an error if the content is not valid JSON5.
pub fn format_json5(content: &str, options: &PrettierConfig) -> Result<String> {
    // Strip UTF-8 BOM if present
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    let (node, leading_comments, trailing_comments) =
        parser::parse(content).map_err(|e| anyhow::anyhow!("JSON5 parse error: {e}"))?;

    let mut doc = node_to_doc(&node, options);

    // When there are leading comments before the root value, prettier forces
    // the root value to break (objects/arrays expand to multi-line).
    if !leading_comments.is_empty() {
        doc = force_group_break(doc);
    }

    // Build full doc with leading and trailing comments
    let mut parts = Vec::new();
    for comment in &leading_comments {
        parts.push(format_comment_doc(comment));
        match comment {
            Comment::Line(_) => parts.push(Doc::Hardline),
            Comment::Block(_) => parts.push(Doc::text(" ")),
        }
    }
    parts.push(doc);
    for comment in &trailing_comments {
        parts.push(Doc::text(" "));
        parts.push(format_comment_doc(comment));
    }

    let full_doc = Doc::concat(parts);
    let mut result = wadler_lindig::print(&full_doc, options);

    // Trim trailing whitespace on each line (prettier does this)
    result = trim_trailing_whitespace(&result);

    result.push('\n');
    Ok(result)
}

#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
fn node_to_doc(node: &Node, options: &PrettierConfig) -> Doc {
    match node {
        Node::Null => Doc::text("null"),
        Node::Undefined => Doc::text("undefined"),
        Node::Hole => Doc::text(""),
        Node::Bool(b) => Doc::text(if *b { "true" } else { "false" }),
        Node::Number(s) => Doc::text(normalize_json5_number(s)),
        Node::String {
            value: _,
            quote,
            raw,
            ..
        } => {
            // Backtick strings are always preserved as-is
            if *quote == Quote::Backtick {
                return Doc::text(raw.clone());
            }

            // In AsNeeded mode, pick the quote that minimizes escapes.
            // In Consistent/Preserve mode (or JSON format), always use preferred quote.
            let q = if options.quote_props == QuoteProps::AsNeeded {
                choose_best_quote(raw, options.single_quote)
            } else if options.single_quote {
                '\''
            } else {
                '"'
            };
            let source_q = match quote {
                Quote::Single => '\'',
                Quote::Double => '"',
                Quote::Backtick => unreachable!(),
            };
            if q == source_q {
                // Same quote — use raw source to preserve escape sequences
                Doc::text(raw.clone())
            } else {
                // Different quote — re-quote the raw inner text
                Doc::text(re_quote_raw_string(raw, q))
            }
        }
        Node::Array(elements) => {
            if elements.is_empty() {
                return Doc::text("[]");
            }

            let trailing = matches!(
                options.trailing_comma,
                TrailingComma::All | TrailingComma::Es5
            );

            // Use fill for arrays of simple unsigned numeric literals (no comments).
            // Matches prettier's isConciselyPrintedArray: only NumericLiteral elements.
            // Signed numbers (-1, +1), NaN, Infinity, undefined are excluded since
            // they map to UnaryExpression/Identifier in babel's AST.
            let is_concise = elements.len() > 1
                && elements.iter().all(|e| {
                    if let Node::Number(s) = &e.value {
                        !s.starts_with('+')
                            && !s.starts_with('-')
                            && s != "NaN"
                            && s != "Infinity"
                            && e.leading_comments.is_empty()
                            && e.trailing_comment.is_none()
                    } else {
                        false
                    }
                });

            if is_concise {
                return json5_array_concise(elements, trailing, options);
            }

            let mut items = Vec::new();
            for (i, elem) in elements.iter().enumerate() {
                // Leading comments
                for comment in &elem.leading_comments {
                    items.push(format_comment_doc(comment));
                    items.push(Doc::Hardline);
                }

                if i > 0 {
                    items.push(Doc::text(","));
                    if elem.preceded_by_blank_line {
                        items.push(Doc::Line);
                        items.push(Doc::Softline);
                    } else {
                        items.push(Doc::Line);
                    }
                }

                items.push(node_to_doc(&elem.value, options));

                // Trailing comment
                if let Some(comment) = &elem.trailing_comment {
                    items.push(Doc::text(" "));
                    items.push(format_comment_doc(comment));
                }
            }

            // Force trailing comma when last element is a hole (JS semantics:
            // [,].length===1 requires the comma). Otherwise use ifBreak for
            // normal trailing comma behavior.
            let last_is_hole = elements
                .last()
                .is_some_and(|e| matches!(e.value, Node::Hole));
            if last_is_hole {
                items.push(Doc::text(","));
            } else if trailing {
                items.push(Doc::if_break(Doc::text(""), Doc::text(",")));
            }

            Doc::group(Doc::concat(vec![
                Doc::text("["),
                Doc::indent(Doc::concat(
                    core::iter::once(Doc::Softline).chain(items).collect(),
                )),
                Doc::Softline,
                Doc::text("]"),
            ]))
        }
        Node::Object {
            entries,
            force_break,
        } => {
            if entries.is_empty() {
                return Doc::text("{}");
            }

            let trailing = matches!(
                options.trailing_comma,
                TrailingComma::All | TrailingComma::Es5
            );

            let open_sep = if options.bracket_spacing {
                Doc::Line
            } else {
                Doc::Softline
            };
            let close_sep = open_sep.clone();

            let mut items = Vec::new();
            for (i, entry) in entries.iter().enumerate() {
                let has_line_comment = entry
                    .leading_comments
                    .iter()
                    .any(|c| matches!(c, Comment::Line(_)));

                if i > 0 {
                    items.push(Doc::text(","));
                }

                if has_line_comment {
                    for comment in &entry.leading_comments {
                        items.push(Doc::Line);
                        items.push(format_comment_doc(comment));
                        if matches!(comment, Comment::Line(_)) {
                            items.push(Doc::BreakParent);
                        }
                    }
                    items.push(Doc::Line);
                } else if !entry.leading_comments.is_empty() {
                    // Block comments only — can stay on same line
                    items.push(Doc::Line);
                    for comment in &entry.leading_comments {
                        items.push(format_comment_doc(comment));
                        items.push(Doc::text(" "));
                    }
                } else if i == 0 {
                    items.push(open_sep.clone());
                } else if entry.preceded_by_blank_line {
                    items.push(Doc::Line);
                    items.push(Doc::Softline);
                } else {
                    items.push(Doc::Line);
                }

                let key_doc = format_key(&entry.key, options);
                items.push(Doc::concat(vec![
                    key_doc,
                    Doc::text(": "),
                    node_to_doc(&entry.value, options),
                ]));

                // Trailing comment
                if let Some(comment) = &entry.trailing_comment {
                    items.push(Doc::text(" "));
                    items.push(format_comment_doc(comment));
                }
            }

            if trailing {
                items.push(Doc::if_break(Doc::text(""), Doc::text(",")));
            }

            // If source had newline between { and first property, force break
            if *force_break {
                items.insert(0, Doc::BreakParent);
            }

            Doc::group(Doc::concat(vec![
                Doc::text("{"),
                Doc::indent(Doc::concat(items)),
                close_sep,
                Doc::text("}"),
            ]))
        }
    }
}

/// Build a fill-based doc for all-numeric JSON5 arrays.
fn json5_array_concise(
    elements: &[parser::ArrayElement],
    trailing: bool,
    options: &PrettierConfig,
) -> Doc {
    let mut fill_parts: Vec<Doc> = Vec::new();

    for (i, elem) in elements.iter().enumerate() {
        let is_last = i == elements.len() - 1;

        // Separator (before content, except first)
        if i > 0 {
            if elem.preceded_by_blank_line {
                fill_parts.push(Doc::concat(vec![Doc::Hardline, Doc::Hardline]));
            } else {
                fill_parts.push(Doc::Line);
            }
        }

        // Content: value + comma (except last)
        let value_doc = node_to_doc(&elem.value, options);
        if is_last {
            fill_parts.push(value_doc);
        } else {
            fill_parts.push(Doc::concat(vec![value_doc, Doc::text(",")]));
        }
    }

    let mut inner = vec![Doc::Softline, Doc::fill(fill_parts)];

    if trailing {
        inner.push(Doc::if_break(Doc::text(""), Doc::text(",")));
    }

    Doc::group(Doc::concat(vec![
        Doc::text("["),
        Doc::indent(Doc::concat(inner)),
        Doc::Softline,
        Doc::text("]"),
    ]))
}

fn format_key(key: &Key, options: &PrettierConfig) -> Doc {
    match key {
        Key::Identifier(name) => match options.quote_props {
            QuoteProps::AsNeeded | QuoteProps::Preserve => Doc::text(name.clone()),
            QuoteProps::Consistent => {
                let q = if options.single_quote { '\'' } else { '"' };
                Doc::text(format!("{q}{}{q}", escape_string(name, q)))
            }
        },
        Key::Number(raw) => {
            let normalized = normalize_json5_number(raw);
            match options.quote_props {
                QuoteProps::AsNeeded | QuoteProps::Preserve => {
                    // Numeric keys stay unquoted with normalized form
                    Doc::text(normalized)
                }
                QuoteProps::Consistent => {
                    // In Consistent mode (used by json format), numeric keys are quoted
                    // only if their JS-evaluated form matches the normalized form.
                    // Otherwise they stay as unquoted numeric literals.
                    if should_quote_numeric_key(raw, &normalized) {
                        let q = if options.single_quote { '\'' } else { '"' };
                        Doc::text(format!("{q}{normalized}{q}"))
                    } else {
                        Doc::text(normalized)
                    }
                }
            }
        }
        Key::String {
            value, quote, raw, ..
        } => {
            // Backtick keys — preserve raw
            if *quote == Quote::Backtick {
                return Doc::text(raw.clone());
            }
            let target_q = if options.single_quote { '\'' } else { '"' };
            let source_q = match quote {
                Quote::Single => '\'',
                Quote::Double | Quote::Backtick => '"',
            };
            match options.quote_props {
                QuoteProps::AsNeeded => {
                    if is_valid_identifier(value) {
                        Doc::text(value.clone())
                    } else if target_q == source_q {
                        Doc::text(raw.clone())
                    } else {
                        Doc::text(re_quote_raw_string(raw, target_q))
                    }
                }
                QuoteProps::Preserve => {
                    // Keep quoted/unquoted status but use preferred quote character
                    if target_q == source_q {
                        Doc::text(raw.clone())
                    } else {
                        Doc::text(re_quote_raw_string(raw, target_q))
                    }
                }
                QuoteProps::Consistent => {
                    if target_q == source_q {
                        Doc::text(raw.clone())
                    } else {
                        Doc::text(re_quote_raw_string(raw, target_q))
                    }
                }
            }
        }
    }
}

/// Determine if a numeric key should be quoted in Consistent mode.
///
/// In prettier's json format (estree-json printer), numeric literal keys are
/// quoted when `String(Number(raw)) === printNumber(raw)` — i.e., when the
/// JS-evaluated canonical form matches the normalized source form. If they
/// differ, the key stays as an unquoted numeric literal.
fn should_quote_numeric_key(raw: &str, normalized: &str) -> bool {
    // Try to parse as f64 (stripping underscores for numeric separators)
    let clean = raw.replace('_', "");
    let parsed: Option<f64> = clean.parse().ok();

    match parsed {
        Some(v) if v.is_nan() || v.is_infinite() => {
            // NaN or Infinity from parsing means the normalized form is unique
            false
        }
        Some(v) => {
            let js_str = js_number_to_string(v);
            js_str == normalized
        }
        None => {
            // Can't parse (e.g., has underscores like 1_2_3) → unique form
            false
        }
    }
}

/// Emulate JavaScript's `Number.prototype.toString()` for an f64 value.
///
/// JS uses exponential notation when exponent >= 21 or <= -7.
fn js_number_to_string(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        };
    }
    if v == 0.0 {
        return "0".to_string();
    }

    let negative = v < 0.0;
    let abs = v.abs();
    let sign = if negative { "-" } else { "" };

    // Check if it's an integer that fits in decimal form (< 1e21)
    // Exact comparison is intentional — mirrors JS Number.toString() semantics
    #[allow(clippy::float_cmp)]
    if abs == abs.trunc() && abs < 1e21 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        if abs <= 9_007_199_254_740_992.0 {
            return format!("{sign}{}", abs as u64);
        }
    }

    // For very large integers (>= 1e21), use exponential
    #[allow(clippy::float_cmp)]
    if abs >= 1e21 && abs == abs.trunc() {
        // Use Rust's exponential formatting
        let s = format!("{abs:e}");
        // Convert to JS style: 1e30 → "1e+30"
        return format!("{sign}{}", js_exp_format(&s));
    }

    // For very small decimals (< 1e-6), use exponential
    if abs < 1e-6 && abs > 0.0 {
        // Use Rust's exponential formatting
        let s = format!("{abs:e}");
        return format!("{sign}{}", js_exp_format(&s));
    }

    // Regular decimal
    // Use ryu-like shortest representation
    let s = format!("{v}");
    s
}

/// Convert Rust exponential format to JS style.
/// Rust: "1e-30" → JS: "1e-30" (same)
/// Rust: "1e30" → JS: "1e+30" (add +)
fn js_exp_format(s: &str) -> String {
    if let Some(e_pos) = s.find('e') {
        let mantissa = &s[..e_pos];
        let exp = &s[e_pos + 1..];
        if exp.starts_with('-') {
            format!("{mantissa}e{exp}")
        } else {
            format!("{mantissa}e+{exp}")
        }
    } else {
        s.to_string()
    }
}

/// Normalize a JSON5 number literal.
///
/// Applies these transformations:
/// 1. Lowercase `E` to `e`
/// 2. Add leading zero: `.123` → `0.123`
/// 3. Strip trailing zeros from decimal (keep at least one): `1.00000` → `1.0`
/// 4. Remove trailing dot: `123.` → `123`
/// 5. Remove `+` from exponent: `e+2` → `e2`
fn normalize_json5_number(s: &str) -> String {
    // Handle special values
    if s == "Infinity" || s == "-Infinity" || s == "+Infinity" || s == "NaN" {
        return s.to_string();
    }

    // Handle hex — lowercase hex digits and prefix
    {
        let rest = s
            .strip_prefix('+')
            .or_else(|| s.strip_prefix('-'))
            .unwrap_or(s);
        if rest.starts_with("0x") || rest.starts_with("0X") {
            return s[..s.len() - rest.len()].to_string() + &rest.to_lowercase();
        }

        // Handle octal and binary — pass through unchanged
        if rest.starts_with("0o")
            || rest.starts_with("0O")
            || rest.starts_with("0b")
            || rest.starts_with("0B")
        {
            return s.to_string();
        }
    }

    // 1. Lowercase E to e
    let mut result = s.replace('E', "e");

    // 2. Add leading zero: .123 → 0.123, -.5 → -0.5
    result = add_leading_zero(&result);

    // 3. Strip trailing zeros from decimal part (keep at least one digit after dot)
    // Regex equivalent: /\.([0-9]+?)0+($|e)/ → ".$1$2"
    // The +? (non-greedy) ensures at least one digit is kept, so .00000 → .0
    if let Some(dot_pos) = result.find('.') {
        let decimal_end = result[dot_pos + 1..]
            .find('e')
            .map_or(result.len(), |p| dot_pos + 1 + p);
        let decimal_part = &result[dot_pos + 1..decimal_end];

        if decimal_part.len() > 1 && decimal_part.ends_with('0') {
            let trimmed = decimal_part.trim_end_matches('0');
            // Keep at least one digit (matching the +? in the regex)
            let keep = if trimmed.is_empty() {
                &decimal_part[..1]
            } else {
                trimmed
            };
            let before = &result[..=dot_pos];
            let after = &result[decimal_end..];
            result = format!("{before}{keep}{after}");
        }
    }

    // 4. Remove zero exponent: 1e0 → 1, 2e00 → 2, 2e-00 → 2
    if let Some(e_pos) = result.rfind('e') {
        let exp_part = &result[e_pos + 1..];
        let exp_digits = exp_part
            .strip_prefix('+')
            .or_else(|| exp_part.strip_prefix('-'))
            .unwrap_or(exp_part);
        if !exp_digits.is_empty() && exp_digits.chars().all(|c| c == '0') {
            result = result[..e_pos].to_string();
        }
    }

    // 5. Remove trailing dot (only if at end or before e)
    if let Some(dot_pos) = result.find('.') {
        let after_dot = &result[dot_pos + 1..];
        if after_dot.is_empty() || after_dot.starts_with('e') {
            let before = &result[..dot_pos];
            let after = &result[dot_pos + 1..];
            result = format!("{before}{after}");
        }
    }

    // 6. Remove + from exponent
    result = result.replace("e+", "e");

    result
}

/// Add leading zero for bare decimal: `.123` → `0.123`
fn add_leading_zero(s: &str) -> String {
    let (sign, rest) = if let Some(r) = s.strip_prefix('+') {
        ("+", r)
    } else if let Some(r) = s.strip_prefix('-') {
        ("-", r)
    } else {
        ("", s)
    };

    if rest.starts_with('.') {
        format!("{sign}0{rest}")
    } else {
        s.to_string()
    }
}

/// Choose the best quote character for a string based on raw source text.
/// Matches prettier's algorithm: counts total and escaped quote occurrences
/// in the raw inner text and picks whichever quote requires fewer escapes.
fn choose_best_quote(raw: &str, prefer_single: bool) -> char {
    let inner = &raw[1..raw.len() - 1];
    let preferred = if prefer_single { '\'' } else { '"' };
    let alternate = if prefer_single { '"' } else { '\'' };

    // Count total occurrences and escaped occurrences of each quote
    let mut pref_total = 0usize;
    let mut pref_escaped = 0usize;
    let mut alt_total = 0usize;
    let mut alt_escaped = 0usize;
    let mut prev_backslash = false;
    for ch in inner.chars() {
        if ch == preferred {
            pref_total += 1;
            if prev_backslash {
                pref_escaped += 1;
            }
        } else if ch == alternate {
            alt_total += 1;
            if prev_backslash {
                alt_escaped += 1;
            }
        }
        prev_backslash = ch == '\\' && !prev_backslash;
    }

    if alt_total == 0 {
        // No alternate quotes in raw text → switching to alternate saves all preferred escapes
        if pref_total > 0 { alternate } else { preferred }
    } else {
        // Both quotes present → pick the one with fewer net unescaped occurrences
        let pref_net = pref_total - pref_escaped;
        let alt_net = alt_total - alt_escaped;
        if pref_net > alt_net {
            alternate
        } else {
            preferred
        }
    }
}

/// Re-quote a raw string literal to use a different quote character.
/// Transforms the raw inner text, preserving escape sequences like `\/` and `\` continuation.
fn re_quote_raw_string(raw: &str, new_quote: char) -> String {
    let old_quote = raw.chars().next().unwrap_or('"');
    let inner = &raw[1..raw.len() - 1];

    let mut result = String::with_capacity(raw.len() + 2);
    result.push(new_quote);

    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                if next == old_quote {
                    // Was escaped old quote → plain character if not new quote
                    chars.next();
                    if next == new_quote {
                        result.push('\\');
                    }
                    result.push(next);
                } else if next == new_quote {
                    // Escaped new quote — keep the escape
                    result.push('\\');
                    if let Some(ch) = chars.next() {
                        result.push(ch);
                    }
                } else {
                    // Other escape — preserve as-is
                    result.push('\\');
                    if let Some(ch) = chars.next() {
                        result.push(ch);
                    }
                }
            } else {
                result.push('\\');
            }
        } else if c == new_quote {
            // Unescaped new quote character → needs escaping
            result.push('\\');
            result.push(c);
        } else {
            result.push(c);
        }
    }

    result.push(new_quote);
    result
}

fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().expect("non-empty string");
    if !first.is_alphabetic() && first != '_' && first != '$' {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

fn escape_string(s: &str, quote: char) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch == quote {
            result.push('\\');
            result.push(ch);
        } else {
            match ch {
                '\\' => result.push_str("\\\\"),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                '\u{08}' => result.push_str("\\b"),
                '\u{0C}' => result.push_str("\\f"),
                c if c.is_control() => {
                    let _ = write!(result, "\\u{:04x}", c as u32);
                }
                c => result.push(c),
            }
        }
    }
    result
}

fn format_comment(comment: &Comment) -> String {
    match comment {
        Comment::Line(text) => format!("//{text}"),
        Comment::Block(text) => format!("/*{text}*/"),
    }
}

fn format_comment_doc(comment: &Comment) -> Doc {
    Doc::text(format_comment(comment))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_simple_json5() {
        let input = r#"{key: "value", num: 42}"#;
        let result = format_json5(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "{ key: \"value\", num: 42 }\n");
    }

    #[test]
    fn format_json5_single_quotes() {
        let opts = PrettierConfig {
            single_quote: true,
            ..Default::default()
        };
        let input = r#"{key: "value"}"#;
        let result = format_json5(input, &opts).expect("format");
        assert!(
            result.contains("'value'"),
            "expected single quotes: {result}"
        );
    }

    #[test]
    fn format_json5_trailing_commas() {
        let opts = PrettierConfig {
            print_width: 10, // force break
            trailing_comma: TrailingComma::All,
            ..Default::default()
        };
        let input = r#"{longkey: "longvalue"}"#;
        let result = format_json5(input, &opts).expect("format");
        assert!(result.contains(','), "expected trailing comma: {result}");
    }

    #[test]
    fn format_json5_empty() {
        let result = format_json5("{}", &PrettierConfig::default()).expect("format");
        assert_eq!(result, "{}\n");
    }
}
