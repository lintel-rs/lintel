use core::fmt::Write;

use anyhow::{Context, Result};

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
        &jsonc_parser::CollectOptions::default(),
        &jsonc_parser::ParseOptions::default(),
    )
    .map_err(|e| anyhow::anyhow!("JSONC parse error: {e}"))?;

    let value = parsed.value.context("empty JSONC document")?;

    let doc = jsonc_value_to_doc(&value, options);
    let mut result = crate::printer::print(&doc, options);
    result.push('\n');
    Ok(result)
}

fn jsonc_value_to_doc(value: &jsonc_parser::ast::Value, options: &PrettierOptions) -> Doc {
    use jsonc_parser::ast::Value;

    match value {
        Value::NullKeyword(_) => Doc::text("null"),
        Value::BooleanLit(b) => Doc::text(if b.value { "true" } else { "false" }),
        Value::NumberLit(n) => Doc::text(n.value.to_string()),
        Value::StringLit(s) => Doc::text(format!("\"{}\"", escape_json_string(&s.value))),
        Value::Array(arr) => jsonc_array_to_doc(arr, options),
        Value::Object(obj) => jsonc_object_to_doc(obj, options),
    }
}

fn jsonc_array_to_doc(arr: &jsonc_parser::ast::Array, options: &PrettierOptions) -> Doc {
    if arr.elements.is_empty() {
        return Doc::text("[]");
    }

    let trailing = matches!(
        options.trailing_comma,
        TrailingComma::All | TrailingComma::Es5
    );

    let mut items = Vec::new();
    for (i, elem) in arr.elements.iter().enumerate() {
        if i > 0 {
            items.push(Doc::text(","));
            items.push(Doc::Line);
        }
        items.push(jsonc_value_to_doc(elem, options));
    }

    if trailing {
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

fn jsonc_object_to_doc(obj: &jsonc_parser::ast::Object, options: &PrettierOptions) -> Doc {
    if obj.properties.is_empty() {
        return Doc::text("{}");
    }

    let trailing = matches!(
        options.trailing_comma,
        TrailingComma::All | TrailingComma::Es5
    );

    let (open, close) = if options.bracket_spacing {
        (Doc::Line, Doc::Line)
    } else {
        (Doc::Softline, Doc::Softline)
    };

    let mut items = Vec::new();
    for (i, prop) in obj.properties.iter().enumerate() {
        if i > 0 {
            items.push(Doc::text(","));
            items.push(Doc::Line);
        }
        items.push(Doc::concat(vec![
            Doc::text(format!("\"{}\"", escape_json_string(prop.name.as_str()))),
            Doc::text(": "),
            jsonc_value_to_doc(&prop.value, options),
        ]));
    }

    if trailing {
        items.push(Doc::if_break(Doc::text(""), Doc::text(",")));
    }

    Doc::group(Doc::concat(vec![
        Doc::text("{"),
        Doc::indent(Doc::concat(core::iter::once(open).chain(items).collect())),
        close,
        Doc::text("}"),
    ]))
}

fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => result.push_str("\\\""),
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
}
