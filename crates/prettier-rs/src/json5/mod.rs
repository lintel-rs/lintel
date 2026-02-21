pub mod parser;

use core::fmt::Write;

use anyhow::Result;

use crate::options::{PrettierOptions, QuoteProps, TrailingComma};
use crate::printer::Doc;
use parser::{Comment, Key, Node, Quote};

/// Format JSON5 content, preserving comments.
///
/// # Errors
///
/// Returns an error if the content is not valid JSON5.
pub fn format_json5(content: &str, options: &PrettierOptions) -> Result<String> {
    let (node, trailing_comments) =
        parser::parse(content).map_err(|e| anyhow::anyhow!("JSON5 parse error: {e}"))?;

    let doc = node_to_doc(&node, options);
    let mut result = crate::printer::print(&doc, options);

    // Append trailing comments
    for comment in &trailing_comments {
        result.push_str(&format_comment(comment));
    }

    result.push('\n');
    Ok(result)
}

#[allow(clippy::too_many_lines)]
fn node_to_doc(node: &Node, options: &PrettierOptions) -> Doc {
    match node {
        Node::Null => Doc::text("null"),
        Node::Bool(b) => Doc::text(if *b { "true" } else { "false" }),
        Node::Number(s) => Doc::text(s.clone()),
        Node::String { value, quote } => {
            let q = if options.single_quote {
                '\''
            } else {
                match quote {
                    Quote::Single => '\'',
                    Quote::Double => '"',
                }
            };
            Doc::text(format!("{q}{}{q}", escape_string(value, q)))
        }
        Node::Array(elements) => {
            if elements.is_empty() {
                return Doc::text("[]");
            }

            let trailing = matches!(
                options.trailing_comma,
                TrailingComma::All | TrailingComma::Es5
            );

            let mut items = Vec::new();
            for (i, elem) in elements.iter().enumerate() {
                // Leading comments
                for comment in &elem.leading_comments {
                    items.push(format_comment_doc(comment));
                    items.push(Doc::Hardline);
                }

                if i > 0 {
                    items.push(Doc::text(","));
                    items.push(Doc::Line);
                }

                items.push(node_to_doc(&elem.value, options));

                // Trailing comment
                if let Some(comment) = &elem.trailing_comment {
                    items.push(Doc::text(" "));
                    items.push(format_comment_doc(comment));
                }
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
        Node::Object(entries) => {
            if entries.is_empty() {
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
            for (i, entry) in entries.iter().enumerate() {
                // Leading comments
                for comment in &entry.leading_comments {
                    items.push(format_comment_doc(comment));
                    items.push(Doc::Hardline);
                }

                if i > 0 {
                    items.push(Doc::text(","));
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

            Doc::group(Doc::concat(vec![
                Doc::text("{"),
                Doc::indent(Doc::concat(core::iter::once(open).chain(items).collect())),
                close,
                Doc::text("}"),
            ]))
        }
    }
}

fn format_key(key: &Key, options: &PrettierOptions) -> Doc {
    match key {
        Key::Identifier(name) => {
            match options.quote_props {
                QuoteProps::AsNeeded | QuoteProps::Preserve => Doc::text(name.clone()),
                QuoteProps::Consistent => {
                    // In consistent mode, we'd need to check all keys first.
                    // For simplicity, preserve identifiers as-is.
                    Doc::text(name.clone())
                }
            }
        }
        Key::String { value, quote } => match options.quote_props {
            QuoteProps::AsNeeded => {
                if is_valid_identifier(value) {
                    Doc::text(value.clone())
                } else {
                    let q = if options.single_quote {
                        '\''
                    } else {
                        match quote {
                            Quote::Single => '\'',
                            Quote::Double => '"',
                        }
                    };
                    Doc::text(format!("{q}{}{q}", escape_string(value, q)))
                }
            }
            QuoteProps::Preserve => {
                let q = match quote {
                    Quote::Single => '\'',
                    Quote::Double => '"',
                };
                Doc::text(format!("{q}{}{q}", escape_string(value, q)))
            }
            QuoteProps::Consistent => {
                let q = if options.single_quote { '\'' } else { '"' };
                Doc::text(format!("{q}{}{q}", escape_string(value, q)))
            }
        },
    }
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
        let result = format_json5(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "{ key: \"value\", num: 42 }\n");
    }

    #[test]
    fn format_json5_single_quotes() {
        let opts = PrettierOptions {
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
        let opts = PrettierOptions {
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
        let result = format_json5("{}", &PrettierOptions::default()).expect("format");
        assert_eq!(result, "{}\n");
    }
}
