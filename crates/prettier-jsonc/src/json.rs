use core::fmt::Write;

use serde_json::Value;

use crate::PrettierConfig;
use crate::printer::Doc;

/// Convert a JSON value to a document IR.
pub fn json_to_doc(value: &Value, options: &PrettierConfig) -> Doc {
    value_to_doc(value, options)
}

fn value_to_doc(value: &Value, options: &PrettierConfig) -> Doc {
    match value {
        Value::Null => Doc::text("null"),
        Value::Bool(b) => Doc::text(if *b { "true" } else { "false" }),
        Value::Number(n) => Doc::text(n.to_string()),
        Value::String(s) => Doc::text(format!("\"{}\"", escape_json_string(s))),
        Value::Array(arr) => array_to_doc(arr, options),
        Value::Object(obj) => object_to_doc(obj, options),
    }
}

fn array_to_doc(arr: &[Value], options: &PrettierConfig) -> Doc {
    if arr.is_empty() {
        return Doc::text("[]");
    }

    let mut items = Vec::new();
    for (i, val) in arr.iter().enumerate() {
        if i > 0 {
            items.push(Doc::text(","));
            items.push(Doc::Line);
        }
        items.push(value_to_doc(val, options));
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

fn object_to_doc(obj: &serde_json::Map<String, Value>, options: &PrettierConfig) -> Doc {
    if obj.is_empty() {
        return Doc::text("{}");
    }

    let (open, close) = if options.bracket_spacing {
        (Doc::Line, Doc::Line)
    } else {
        (Doc::Softline, Doc::Softline)
    };

    let mut items = Vec::new();
    for (i, (key, val)) in obj.iter().enumerate() {
        if i > 0 {
            items.push(Doc::text(","));
            items.push(Doc::Line);
        }
        items.push(Doc::concat(vec![
            Doc::text(format!("\"{}\"", escape_json_string(key))),
            Doc::text(": "),
            value_to_doc(val, options),
        ]));
    }

    let inner = Doc::concat(core::iter::once(open).chain(items).collect());

    Doc::group(Doc::concat(vec![
        Doc::text("{"),
        Doc::indent(inner),
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
    use crate::printer;

    fn format_json(input: &str) -> String {
        let value: Value = serde_json::from_str(input).expect("valid JSON");
        let doc = json_to_doc(&value, &PrettierConfig::default());
        let mut result = printer::print(&doc, &PrettierConfig::default());
        result.push('\n');
        result
    }

    #[test]
    fn empty_object() {
        assert_eq!(format_json("{}"), "{}\n");
    }

    #[test]
    fn empty_array() {
        assert_eq!(format_json("[]"), "[]\n");
    }

    #[test]
    fn simple_object() {
        let result = format_json(r#"{"a": 1, "b": 2}"#);
        assert_eq!(result, "{ \"a\": 1, \"b\": 2 }\n");
    }

    #[test]
    fn simple_array() {
        let result = format_json("[1, 2, 3]");
        assert_eq!(result, "[1, 2, 3]\n");
    }

    #[test]
    fn nested_object() {
        let result = format_json(r#"{"a": {"b": 1}}"#);
        assert_eq!(result, "{ \"a\": { \"b\": 1 } }\n");
    }

    #[test]
    fn long_array_breaks() {
        let input = r"[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]";
        let result = format_json(input);
        assert!(result.contains('\n'), "long array should break: {result}");
    }

    #[test]
    fn null_bool_number() {
        assert_eq!(format_json("null"), "null\n");
        assert_eq!(format_json("true"), "true\n");
        assert_eq!(format_json("false"), "false\n");
        assert_eq!(format_json("42"), "42\n");
    }

    #[test]
    fn string_escaping() {
        let result = format_json(r#""hello\nworld""#);
        assert_eq!(result, "\"hello\\nworld\"\n");
    }

    #[test]
    fn no_bracket_spacing() {
        let opts = PrettierConfig {
            bracket_spacing: false,
            ..Default::default()
        };
        let value: Value = serde_json::from_str(r#"{"a": 1}"#).expect("valid JSON");
        let doc = json_to_doc(&value, &opts);
        let mut result = printer::print(&doc, &opts);
        result.push('\n');
        assert_eq!(result, "{\"a\": 1}\n");
    }
}
