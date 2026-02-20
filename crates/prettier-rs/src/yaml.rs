use std::fmt::Write;

use anyhow::{Context, Result};

use crate::options::PrettierOptions;

/// Format YAML content with prettier-compatible output.
///
/// # Errors
///
/// Returns an error if the content is not valid YAML.
pub fn format_yaml(content: &str, options: &PrettierOptions) -> Result<String> {
    // Parse YAML value
    let value: serde_yaml::Value = serde_yaml::from_str(content).context("failed to parse YAML")?;

    let mut output = String::new();
    write_value(&value, &mut output, 0, options, true);

    // Ensure trailing newline
    if !output.ends_with('\n') {
        output.push('\n');
    }

    Ok(output)
}

fn write_value(
    value: &serde_yaml::Value,
    output: &mut String,
    indent: usize,
    options: &PrettierOptions,
    is_top_level: bool,
) {
    match value {
        serde_yaml::Value::Null => output.push_str("null"),
        serde_yaml::Value::Bool(b) => output.push_str(if *b { "true" } else { "false" }),
        serde_yaml::Value::Number(n) => output.push_str(&n.to_string()),
        serde_yaml::Value::String(s) => write_string(s, output, options),
        serde_yaml::Value::Sequence(seq) => {
            write_sequence(seq, output, indent, options, is_top_level);
        }
        serde_yaml::Value::Mapping(map) => {
            write_mapping(map, output, indent, options, is_top_level);
        }
        serde_yaml::Value::Tagged(tagged) => {
            let _ = write!(output, "!{} ", tagged.tag);
            write_value(&tagged.value, output, indent, options, false);
        }
    }
}

fn write_string(s: &str, output: &mut String, options: &PrettierOptions) {
    if s.is_empty() {
        let q = if options.single_quote { '\'' } else { '"' };
        output.push(q);
        output.push(q);
        return;
    }

    // Check if quoting is needed
    if needs_quoting(s) {
        let q = if options.single_quote { '\'' } else { '"' };
        output.push(q);
        if options.single_quote {
            // In YAML, single-quoted strings escape single quotes by doubling
            output.push_str(&s.replace('\'', "''"));
        } else {
            output.push_str(&escape_yaml_double_quoted(s));
        }
        output.push(q);
    } else {
        output.push_str(s);
    }
}

fn needs_quoting(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }

    // Values that look like other YAML types need quoting
    let lower = s.to_lowercase();
    if matches!(
        lower.as_str(),
        "true" | "false" | "yes" | "no" | "on" | "off" | "null" | "~"
    ) {
        return true;
    }

    // Strings that start with special characters
    let first = s.chars().next().expect("non-empty string");
    if matches!(
        first,
        '{' | '}'
            | '['
            | ']'
            | ','
            | '#'
            | '&'
            | '*'
            | '!'
            | '|'
            | '>'
            | '\''
            | '"'
            | '%'
            | '@'
            | '`'
    ) {
        return true;
    }

    // Contains : followed by space, or starts with - followed by space
    if s.contains(": ") || s.contains(" #") || s.starts_with("- ") || s.starts_with("? ") {
        return true;
    }

    // Contains newlines
    if s.contains('\n') || s.contains('\r') {
        return true;
    }

    // Looks like a number
    if s.parse::<f64>().is_ok() || s.starts_with("0x") || s.starts_with("0o") || s.starts_with("0b")
    {
        return true;
    }

    false
}

fn escape_yaml_double_quoted(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\u{08}' => result.push_str("\\b"),
            '\u{07}' => result.push_str("\\a"),
            '\u{0B}' => result.push_str("\\v"),
            '\u{0C}' => result.push_str("\\f"),
            '\u{1B}' => result.push_str("\\e"),
            c if c.is_control() => {
                let _ = write!(result, "\\x{:02x}", c as u32);
            }
            c => result.push(c),
        }
    }
    result
}

fn indent_str(indent: usize, options: &PrettierOptions) -> String {
    if options.use_tabs {
        "\t".repeat(indent)
    } else {
        " ".repeat(indent * options.tab_width)
    }
}

fn write_sequence(
    seq: &[serde_yaml::Value],
    output: &mut String,
    indent: usize,
    options: &PrettierOptions,
    is_top_level: bool,
) {
    if seq.is_empty() {
        output.push_str("[]");
        return;
    }

    if !is_top_level {
        output.push('\n');
    }

    let prefix = indent_str(indent, options);
    for (i, item) in seq.iter().enumerate() {
        if i > 0 || !is_top_level {
            output.push_str(&prefix);
        }
        output.push_str("- ");

        match item {
            serde_yaml::Value::Mapping(map) if !map.is_empty() => {
                // Write first key-value inline after the dash
                let mut iter = map.iter();
                if let Some((k, v)) = iter.next() {
                    write_mapping_key(k, output, options);
                    output.push_str(": ");
                    if is_complex_value(v) {
                        write_value(v, output, indent + 2, options, false);
                    } else {
                        write_value(v, output, indent + 1, options, false);
                    }
                    output.push('\n');

                    // Write remaining key-values indented
                    let nested_prefix = indent_str(indent + 1, options);
                    for (k, v) in iter {
                        output.push_str(&nested_prefix);
                        write_mapping_key(k, output, options);
                        output.push_str(": ");
                        if is_complex_value(v) {
                            write_value(v, output, indent + 2, options, false);
                        } else {
                            write_value(v, output, indent + 1, options, false);
                        }
                        output.push('\n');
                    }
                }
            }
            _ => {
                write_value(item, output, indent + 1, options, false);
                output.push('\n');
            }
        }
    }
}

fn write_mapping(
    map: &serde_yaml::Mapping,
    output: &mut String,
    indent: usize,
    options: &PrettierOptions,
    is_top_level: bool,
) {
    if map.is_empty() {
        output.push_str("{}");
        return;
    }

    if !is_top_level {
        output.push('\n');
    }

    let prefix = indent_str(indent, options);
    for (i, (key, value)) in map.iter().enumerate() {
        if i > 0 || !is_top_level {
            output.push_str(&prefix);
        }
        write_mapping_key(key, output, options);
        output.push_str(": ");

        if is_complex_value(value) {
            write_value(value, output, indent + 1, options, false);
        } else {
            write_value(value, output, indent, options, false);
        }
        output.push('\n');
    }
}

fn write_mapping_key(key: &serde_yaml::Value, output: &mut String, options: &PrettierOptions) {
    match key {
        serde_yaml::Value::String(s) => write_string(s, output, options),
        _ => write_value(key, output, 0, options, false),
    }
}

fn is_complex_value(value: &serde_yaml::Value) -> bool {
    matches!(
        value,
        serde_yaml::Value::Mapping(_) | serde_yaml::Value::Sequence(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_simple_mapping() {
        let input = "a: 1\nb: 2\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "a: 1\nb: 2\n");
    }

    #[test]
    fn format_simple_sequence() {
        let input = "- 1\n- 2\n- 3\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "- 1\n- 2\n- 3\n");
    }

    #[test]
    fn format_null_values() {
        let input = "a: null\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        assert_eq!(result, "a: null\n");
    }

    #[test]
    fn format_string_quoting() {
        let input = "a: \"true\"\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        // "true" needs quoting because it looks like a boolean
        assert!(
            result.contains("\"true\"") || result.contains("'true'"),
            "boolean-like string should be quoted: {result}"
        );
    }

    #[test]
    fn format_empty_string() {
        let input = "a: \"\"\n";
        let result = format_yaml(input, &PrettierOptions::default()).expect("format");
        assert!(
            result.contains("\"\"") || result.contains("''"),
            "empty string should be quoted: {result}"
        );
    }
}
