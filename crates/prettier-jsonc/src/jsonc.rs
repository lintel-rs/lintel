use anyhow::{Context, Result};
use jsonc_parser::ast::Comment;
use jsonc_parser::common::Ranged;

use crate::options::{PrettierOptions, TrailingComma};
use crate::printer::Doc;

/// Format JSONC content, preserving comments.
///
/// # Errors
///
/// Returns an error if the content is not valid JSONC.
pub fn format_jsonc(content: &str, options: &PrettierOptions) -> Result<String> {
    let parsed = jsonc_parser::parse_to_ast(
        content,
        &jsonc_parser::CollectOptions {
            comments: jsonc_parser::CommentCollectionStrategy::Separate,
            tokens: false,
        },
        &jsonc_parser::ParseOptions::default(),
    )
    .map_err(|e| anyhow::anyhow!("JSONC parse error: {e}"))?;

    let value = parsed.value.context("empty JSONC document")?;

    // Collect and deduplicate comments sorted by position
    let mut comments = collect_comments(parsed.comments.as_ref());
    let mut ctx = CommentCtx::new(&mut comments);
    let source = content;

    let value_start = value.range().start;

    // Leading comments (before root value)
    let leading = ctx.take_range(0, value_start);

    // Generate doc for value (internal comments consumed here)
    let mut doc = jsonc_value_to_doc(&value, options, &mut ctx, source);

    // When there are leading comments before the root value, prettier forces
    // the root value to break (objects/arrays expand to multi-line).
    if !leading.is_empty() {
        doc = force_group_break(doc);
    }

    // Trailing comments (after root value)
    let trailing = ctx.take_remaining();

    let mut parts = Vec::new();
    for c in &leading {
        parts.push(comment_doc(c));
        match c {
            Comment::Line(_) => parts.push(Doc::Hardline),
            Comment::Block(_) => parts.push(Doc::text(" ")),
        }
    }
    parts.push(doc);
    for c in &trailing {
        parts.push(Doc::text(" "));
        parts.push(comment_doc(c));
    }

    let full_doc = Doc::concat(parts);
    let mut result = crate::printer::print(&full_doc, options);
    // Trim trailing whitespace on each line (prettier does this)
    result = trim_trailing_whitespace(&result);
    result.push('\n');
    Ok(result)
}

/// Collect all comments from the `CommentMap`, deduplicated and sorted by position.
fn collect_comments<'a>(map: Option<&jsonc_parser::CommentMap<'a>>) -> Vec<Comment<'a>> {
    let Some(map) = map else {
        return Vec::new();
    };
    let mut all: Vec<Comment<'a>> = Vec::new();
    for comment_list in map.values() {
        for c in comment_list.iter() {
            all.push(c.clone());
        }
    }
    all.sort_by_key(|c| c.range().start);
    all.dedup_by(|a, b| a.range().start == b.range().start);
    all
}

/// Tracks which comments have been consumed during doc generation.
struct CommentCtx<'a, 'b> {
    comments: &'b mut Vec<Comment<'a>>,
    consumed: Vec<bool>,
}

impl<'a, 'b> CommentCtx<'a, 'b> {
    fn new(comments: &'b mut Vec<Comment<'a>>) -> Self {
        let len = comments.len();
        CommentCtx {
            comments,
            consumed: vec![false; len],
        }
    }

    /// Take all comments whose start position is in [from, to).
    fn take_range(&mut self, from: usize, to: usize) -> Vec<Comment<'a>> {
        let mut result = Vec::new();
        for (i, consumed) in self.consumed.iter_mut().enumerate() {
            if *consumed {
                continue;
            }
            let start = self.comments[i].range().start;
            if start >= to {
                break; // comments are sorted, no more can match
            }
            if start >= from {
                *consumed = true;
                result.push(self.comments[i].clone());
            }
        }
        result
    }

    /// Check if there are any unconsumed comments in the given range.
    fn has_comments_in_range(&self, from: usize, to: usize) -> bool {
        self.comments
            .iter()
            .enumerate()
            .any(|(i, c)| !self.consumed[i] && c.range().start >= from && c.range().start < to)
    }

    /// Take all remaining unconsumed comments.
    fn take_remaining(&mut self) -> Vec<Comment<'a>> {
        let mut result = Vec::new();
        for (i, consumed) in self.consumed.iter_mut().enumerate() {
            if !*consumed {
                *consumed = true;
                result.push(self.comments[i].clone());
            }
        }
        result
    }
}

/// Force the outermost Group in a Doc to break by inserting `BreakParent`.
fn force_group_break(doc: Doc) -> Doc {
    match doc {
        Doc::Group(inner) => Doc::Group(Box::new(Doc::concat(vec![Doc::BreakParent, *inner]))),
        other => other,
    }
}

/// Create a Doc for a single comment.
fn comment_doc(c: &Comment) -> Doc {
    match c {
        Comment::Line(lc) => Doc::text(format!("//{}", lc.text)),
        Comment::Block(bc) => Doc::text(format!("/*{}*/", bc.text)),
    }
}

fn jsonc_value_to_doc<'a>(
    value: &jsonc_parser::ast::Value<'a>,
    options: &PrettierOptions,
    ctx: &mut CommentCtx<'a, '_>,
    source: &str,
) -> Doc {
    use jsonc_parser::ast::Value;

    match value {
        Value::NullKeyword(_) => Doc::text("null"),
        Value::BooleanLit(b) => Doc::text(if b.value { "true" } else { "false" }),
        Value::NumberLit(n) => Doc::text(normalize_number(n.value)),
        Value::StringLit(s) => {
            // Use raw source text to preserve escape sequences like \/
            let raw = &source[s.range.start..s.range.end];
            Doc::text(re_quote_to_double(raw))
        }
        Value::Array(arr) => jsonc_array_to_doc(arr, options, ctx, source),
        Value::Object(obj) => jsonc_object_to_doc(obj, options, ctx, source),
    }
}

fn jsonc_array_to_doc<'a>(
    arr: &jsonc_parser::ast::Array<'a>,
    options: &PrettierOptions,
    ctx: &mut CommentCtx<'a, '_>,
    source: &str,
) -> Doc {
    let arr_start = arr.range().start;
    let arr_end = arr.range().end;

    if arr.elements.is_empty() {
        // Check for dangling comments inside empty []
        let dangling = ctx.take_range(arr_start + 1, arr_end);
        if dangling.is_empty() {
            return Doc::text("[]");
        }
        let mut parts = Vec::new();
        for c in &dangling {
            parts.push(Doc::Hardline);
            parts.push(comment_doc(c));
        }
        return Doc::group(Doc::concat(vec![
            Doc::text("["),
            Doc::indent(Doc::concat(parts)),
            Doc::Hardline,
            Doc::text("]"),
        ]));
    }

    // Use fill (concise) format for all-numeric arrays without comments.
    // This packs numbers onto lines like prettier does.
    if is_concise_array(arr) && !ctx.has_comments_in_range(arr_start + 1, arr_end) {
        return jsonc_array_to_doc_concise(arr, options, ctx, source);
    }

    let trailing = matches!(
        options.trailing_comma,
        TrailingComma::All | TrailingComma::Es5
    );

    let mut items = Vec::new();
    let mut prev_end = arr_start + 1; // after '['

    for (i, elem) in arr.elements.iter().enumerate() {
        let elem_start = elem.range().start;

        // Comments between previous position and this element
        let comments_before = ctx.take_range(prev_end, elem_start);

        if i > 0 {
            items.push(Doc::text(","));
        }

        if !comments_before.is_empty() {
            emit_leading_comments(&comments_before, i == 0, &mut items);
        } else if i == 0 {
            items.push(Doc::Softline);
        } else if has_blank_line_between(source, prev_end, elem_start) {
            // Blank line: Line + Softline produces blank line only when group breaks.
            // Line → newline+indent, Softline → newline+indent → two newlines.
            // After trim_trailing_whitespace, the blank line has no trailing spaces.
            items.push(Doc::Line);
            items.push(Doc::Softline);
        } else {
            items.push(Doc::Line);
        }

        items.push(jsonc_value_to_doc(elem, options, ctx, source));
        prev_end = elem.range().end;

        // Trailing comments
        let trailing_comments = ctx.take_range(prev_end, next_significant_pos(prev_end, arr_end));
        for c in &trailing_comments {
            items.push(Doc::text(" "));
            items.push(comment_doc(c));
            if matches!(c, Comment::Line(_)) {
                items.push(Doc::BreakParent);
            }
            prev_end = c.range().end;
        }
    }

    if trailing {
        items.push(Doc::if_break(Doc::text(""), Doc::text(",")));
    }

    // Dangling comments after last element
    let end_comments = ctx.take_range(prev_end, arr_end);
    for c in &end_comments {
        items.push(Doc::Line);
        items.push(comment_doc(c));
        if matches!(c, Comment::Line(_)) {
            items.push(Doc::BreakParent);
        }
    }

    Doc::group(Doc::concat(vec![
        Doc::text("["),
        Doc::indent(Doc::concat(items)),
        Doc::Softline,
        Doc::text("]"),
    ]))
}

/// Check if an array contains only numeric literals (concise/fill formatting).
fn is_concise_array(arr: &jsonc_parser::ast::Array<'_>) -> bool {
    use jsonc_parser::ast::Value;
    !arr.elements.is_empty()
        && arr
            .elements
            .iter()
            .all(|elem| matches!(elem, Value::NumberLit(_)))
}

/// Build a fill-based doc for all-numeric arrays.
/// Packs numbers onto lines, breaking only when needed or at blank lines.
fn jsonc_array_to_doc_concise<'a>(
    arr: &jsonc_parser::ast::Array<'a>,
    options: &PrettierOptions,
    ctx: &mut CommentCtx<'a, '_>,
    source: &str,
) -> Doc {
    let arr_start = arr.range().start;
    let arr_end = arr.range().end;

    let trailing = matches!(
        options.trailing_comma,
        TrailingComma::All | TrailingComma::Es5
    );

    let mut fill_parts: Vec<Doc> = Vec::new();
    let mut prev_end = arr_start + 1; // after '['

    for (i, elem) in arr.elements.iter().enumerate() {
        let elem_start = elem.range().start;
        let is_last = i == arr.elements.len() - 1;

        // Separator (before content, except for first element)
        if i > 0 {
            if has_blank_line_between(source, prev_end, elem_start) {
                fill_parts.push(Doc::concat(vec![Doc::Hardline, Doc::Hardline]));
            } else {
                fill_parts.push(Doc::Line);
            }
        }

        // Content: value + comma (except last element)
        let value_doc = jsonc_value_to_doc(elem, options, ctx, source);
        if is_last {
            fill_parts.push(value_doc);
        } else {
            fill_parts.push(Doc::concat(vec![value_doc, Doc::text(",")]));
        }

        prev_end = elem.range().end;
    }

    // Consume any remaining comments in the array range
    let _ = ctx.take_range(prev_end, arr_end);

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

fn jsonc_object_to_doc<'a>(
    obj: &jsonc_parser::ast::Object<'a>,
    options: &PrettierOptions,
    ctx: &mut CommentCtx<'a, '_>,
    source: &str,
) -> Doc {
    let obj_start = obj.range().start;
    let obj_end = obj.range().end;

    if obj.properties.is_empty() {
        // Check for dangling comments inside empty {}
        let dangling = ctx.take_range(obj_start + 1, obj_end);
        if dangling.is_empty() {
            return Doc::text("{}");
        }
        let mut parts = Vec::new();
        for c in &dangling {
            parts.push(Doc::Hardline);
            parts.push(comment_doc(c));
        }
        return Doc::group(Doc::concat(vec![
            Doc::text("{"),
            Doc::indent(Doc::concat(parts)),
            Doc::Hardline,
            Doc::text("}"),
        ]));
    }

    let trailing_comma = matches!(
        options.trailing_comma,
        TrailingComma::All | TrailingComma::Es5
    );

    let open_sep = if options.bracket_spacing {
        Doc::Line
    } else {
        Doc::Softline
    };

    let mut items = Vec::new();
    let mut prev_end = obj_start + 1; // after '{'

    for (i, prop) in obj.properties.iter().enumerate() {
        let prop_start = prop.range().start;

        // Comments between previous position and this property
        let comments_before = ctx.take_range(prev_end, prop_start);

        if i > 0 {
            items.push(Doc::text(","));
        }

        if !comments_before.is_empty() {
            emit_leading_comments(
                &comments_before,
                i == 0 && options.bracket_spacing,
                &mut items,
            );
        } else if i == 0 {
            items.push(open_sep.clone());
        } else if has_blank_line_between(source, prev_end, prop_start) {
            items.push(Doc::Line);
            items.push(Doc::Softline);
        } else {
            items.push(Doc::Line);
        }

        // Property key: re-quote strings to double, quote bare words
        let raw_key = match &prop.name {
            jsonc_parser::ast::ObjectPropName::String(s) => {
                re_quote_to_double(&source[s.range.start..s.range.end])
            }
            jsonc_parser::ast::ObjectPropName::Word(w) => {
                let name = &source[w.range.start..w.range.end];
                format!("\"{name}\"")
            }
        };

        items.push(Doc::concat(vec![
            Doc::text(raw_key),
            Doc::text(": "),
            jsonc_value_to_doc(&prop.value, options, ctx, source),
        ]));

        prev_end = prop.range().end;

        // Check for trailing comment on same line as property value
        let trailing_comments = ctx.take_range(prev_end, next_significant_pos(prev_end, obj_end));
        for c in &trailing_comments {
            items.push(Doc::text(" "));
            items.push(comment_doc(c));
            if matches!(c, Comment::Line(_)) {
                items.push(Doc::BreakParent);
            }
            prev_end = c.range().end;
        }
    }

    if trailing_comma {
        items.push(Doc::if_break(Doc::text(""), Doc::text(",")));
    }

    // Dangling comments after last property
    let end_comments = ctx.take_range(prev_end, obj_end);
    for c in &end_comments {
        items.push(Doc::Line);
        items.push(comment_doc(c));
        if matches!(c, Comment::Line(_)) {
            items.push(Doc::BreakParent);
        }
    }

    let close_sep = if options.bracket_spacing {
        Doc::Line
    } else {
        Doc::Softline
    };

    Doc::group(Doc::concat(vec![
        Doc::text("{"),
        Doc::indent(Doc::concat(items)),
        close_sep,
        Doc::text("}"),
    ]))
}

/// Emit leading comments before a node.
///
/// For block comments: space + comment + space (can stay on same line)
/// For line comments: hardline + comment + break parent (forces group to break)
fn emit_leading_comments(comments: &[Comment], _is_first_in_container: bool, items: &mut Vec<Doc>) {
    let has_line_comment = comments.iter().any(|c| matches!(c, Comment::Line(_)));

    if has_line_comment {
        // Line comments force the group to break
        for c in comments {
            items.push(Doc::Line);
            items.push(comment_doc(c));
            if matches!(c, Comment::Line(_)) {
                items.push(Doc::BreakParent);
            }
        }
        // After all comments, add line break before the node
        items.push(Doc::Line);
    } else {
        // Block comments only — can stay on same line
        items.push(Doc::Line);
        for c in comments {
            items.push(comment_doc(c));
            items.push(Doc::text(" "));
        }
    }
}

/// Returns `max` — trailing comments are found by scanning the full remaining range.
fn next_significant_pos(_pos: usize, max: usize) -> usize {
    max
}

/// Check if there are any blank lines (two consecutive newlines) between
/// two byte positions in the source text.
fn has_blank_line_between(source: &str, from: usize, to: usize) -> bool {
    let from = from.min(source.len());
    let to = to.min(source.len());
    if from >= to {
        return false;
    }
    let slice = &source[from..to];
    // Look for \n followed by optional whitespace then another \n
    let mut saw_newline = false;
    for ch in slice.chars() {
        if ch == '\n' {
            if saw_newline {
                return true;
            }
            saw_newline = true;
        } else if ch == '\r' || ch == ' ' || ch == '\t' {
            // whitespace between newlines is fine
        } else {
            saw_newline = false;
        }
    }
    false
}

/// Normalize a JSON number literal to match prettier's output:
/// - Lowercase 'E' to 'e' in exponents
/// - Remove '+' from exponent
/// - Strip trailing zeros in decimal part
fn normalize_number(s: &str) -> String {
    // Split into mantissa and exponent parts
    let (mantissa, exponent) = if let Some(e_pos) = s.find(['e', 'E']) {
        (&s[..e_pos], Some(&s[e_pos + 1..]))
    } else {
        (s, None)
    };

    // Normalize mantissa: strip trailing zeros after decimal point
    let mantissa = if let Some(dot_pos) = mantissa.find('.') {
        let before = &mantissa[..dot_pos];
        let after = mantissa[dot_pos + 1..].trim_end_matches('0');
        if after.is_empty() {
            before.to_string()
        } else {
            format!("{before}.{after}")
        }
    } else {
        mantissa.to_string()
    };

    // Normalize exponent
    if let Some(exp) = exponent {
        // Remove leading '+' from exponent
        let exp = exp.strip_prefix('+').unwrap_or(exp);
        // Remove exponent if it's zero (e.g., 1e00 → 1, 2e-00 → 2)
        let exp_value = exp.strip_prefix('-').unwrap_or(exp);
        if exp_value.chars().all(|c| c == '0') {
            mantissa
        } else {
            format!("{mantissa}e{exp}")
        }
    } else {
        mantissa
    }
}

/// Trim trailing whitespace from each line.
fn trim_trailing_whitespace(s: &str) -> String {
    s.lines().map(str::trim_end).collect::<Vec<_>>().join("\n")
}

/// Re-quote a string literal to use double quotes.
/// Input is the raw source text including surrounding quotes.
fn re_quote_to_double(raw: &str) -> String {
    let first = raw.chars().next().unwrap_or('"');
    if first == '"' {
        // Already double-quoted
        return raw.to_string();
    }

    // Single-quoted — convert to double quotes
    let inner = &raw[1..raw.len() - 1];
    let mut result = String::with_capacity(raw.len() + 2);
    result.push('"');

    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    match next {
                        '\'' => {
                            // Escaped single quote → plain single quote in double-quoted
                            result.push('\'');
                        }
                        '"' => {
                            // Was unescaped double quote in single-quoted → escape it
                            result.push_str("\\\"");
                        }
                        other => {
                            result.push('\\');
                            result.push(other);
                        }
                    }
                } else {
                    result.push('\\');
                }
            }
            '"' => {
                // Unescaped double quote in single-quoted → escape it
                result.push_str("\\\"");
            }
            other => result.push(other),
        }
    }

    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_simple_jsonc() {
        let input = r#"{"a":1,"b":2}"#;
        let result = format_jsonc(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "{ \"a\": 1, \"b\": 2 }\n");
    }

    #[test]
    fn format_jsonc_with_trailing_comma() {
        let input = r#"{"a": 1,}"#;
        let result = format_jsonc(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "{ \"a\": 1 }\n");
    }

    #[test]
    fn format_empty_jsonc_object() {
        let input = "{}";
        let result = format_jsonc(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "{}\n");
    }

    #[test]
    fn format_block_comment_before_property() {
        let input = r#"{/*comment*/"K":"V"}"#;
        let result = format_jsonc(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "{ /*comment*/ \"K\": \"V\" }\n");
    }

    #[test]
    fn format_line_comment_before_property() {
        let input = "{\n  //comment\n  \"K\":\"V\"\n}";
        let opts = PrettierOptions {
            trailing_comma: TrailingComma::None,
            ..PrettierOptions::default()
        };
        let result = format_jsonc(input, &opts).expect("format");
        assert_eq!(result, "{\n  //comment\n  \"K\": \"V\"\n}\n");
    }

    #[test]
    fn format_line_comment_before_property_trailing_comma() {
        let input = "{\n  //comment\n  \"K\":\"V\"\n}";
        let result = format_jsonc(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "{\n  //comment\n  \"K\": \"V\",\n}\n");
    }

    #[test]
    fn format_trailing_block_comment() {
        let input = "1 /* block-comment */";
        let result = format_jsonc(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "1 /* block-comment */\n");
    }

    #[test]
    fn format_trailing_line_comment() {
        let input = "1 // line-comment";
        let result = format_jsonc(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "1 // line-comment\n");
    }

    #[test]
    fn format_leading_block_comment() {
        // Leading comments on root value force the container to break
        let input = "/* comment */{\n  \"foo\": \"bar\"\n}";
        let opts = PrettierOptions {
            trailing_comma: TrailingComma::None,
            ..PrettierOptions::default()
        };
        let result = format_jsonc(input, &opts).expect("format");
        assert_eq!(result, "/* comment */ {\n  \"foo\": \"bar\"\n}\n");
    }

    #[test]
    fn format_leading_line_comments() {
        // Leading comments on root value force the container to break
        let input = "// comment 1\n// comment 2\n{\n  \"foo\": \"bar\"\n}";
        let opts = PrettierOptions {
            trailing_comma: TrailingComma::None,
            ..PrettierOptions::default()
        };
        let result = format_jsonc(input, &opts).expect("format");
        assert_eq!(
            result,
            "// comment 1\n// comment 2\n{\n  \"foo\": \"bar\"\n}\n"
        );
    }
}
