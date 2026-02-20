mod ast;
mod comments;
mod parser;
mod print;
mod printer;
mod utilities;

use anyhow::Result;

/// Prose wrapping mode for YAML formatting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProseWrap {
    Always,
    Never,
    #[default]
    Preserve,
}

/// YAML-specific formatting options (self-contained, no dependency on prettier-rs).
#[derive(Debug, Clone)]
pub struct YamlFormatOptions {
    /// Specify the line length that the printer will wrap on.
    pub print_width: usize,
    /// Specify the number of spaces per indentation-level.
    pub tab_width: usize,
    /// Indent lines with tabs instead of spaces.
    pub use_tabs: bool,
    /// Use single quotes instead of double quotes.
    pub single_quote: bool,
    /// Print spaces between brackets in object literals.
    pub bracket_spacing: bool,
    /// How to wrap prose (long text).
    pub prose_wrap: ProseWrap,
}

impl Default for YamlFormatOptions {
    fn default() -> Self {
        Self {
            print_width: 80,
            tab_width: 2,
            use_tabs: false,
            single_quote: false,
            bracket_spacing: true,
            prose_wrap: ProseWrap::Preserve,
        }
    }
}

/// Format YAML content with prettier-compatible output.
///
/// # Errors
///
/// Returns an error if the content is not valid YAML.
pub fn format_yaml(content: &str, options: &YamlFormatOptions) -> Result<String> {
    let events = parser::collect_events(content)?;
    let comments = comments::extract_comments(content);
    let mut builder = parser::AstBuilder::new(content, &events, &comments);
    let stream = builder.build_stream()?;
    let output = printer::format_stream(&stream, options);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_simple_mapping() {
        let input = "a: 1\nb: 2\n";
        let result = format_yaml(input, &YamlFormatOptions::default()).expect("format");
        assert_eq!(result, "a: 1\nb: 2\n");
    }

    #[test]
    fn format_simple_sequence() {
        let input = "- 1\n- 2\n- 3\n";
        let result = format_yaml(input, &YamlFormatOptions::default()).expect("format");
        assert_eq!(result, "- 1\n- 2\n- 3\n");
    }

    #[test]
    fn format_null_values() {
        let input = "a:\n";
        let result = format_yaml(input, &YamlFormatOptions::default()).expect("format");
        assert_eq!(result, "a:\n");
    }

    #[test]
    fn format_nested_mapping() {
        let input = "key:\n  nested: value\n";
        let result = format_yaml(input, &YamlFormatOptions::default()).expect("format");
        assert_eq!(result, "key:\n  nested: value\n");
    }

    #[test]
    fn format_sequence_of_mappings() {
        let input = "- a: b\n  c: d\n";
        let result = format_yaml(input, &YamlFormatOptions::default()).expect("format");
        assert_eq!(result, "- a: b\n  c: d\n");
    }

    #[test]
    fn format_block_literal_clip() {
        let input = "|\n    123\n    456\n    789\n";
        let result = format_yaml(input, &YamlFormatOptions::default()).expect("format");
        assert_eq!(result, "|\n  123\n  456\n  789\n");
    }

    #[test]
    fn format_block_literal_keep() {
        let input = "|+\n    123\n    456\n    789\n\n\n";
        let result = format_yaml(input, &YamlFormatOptions::default()).expect("format");
        assert_eq!(result, "|+\n  123\n  456\n  789\n\n\n");
    }

    #[test]
    fn format_block_literal_in_mapping() {
        let input = "a: |\n  123\n  456\n  789\n";
        let result = format_yaml(input, &YamlFormatOptions::default()).expect("format");
        eprintln!("result: {:?}", result);
        assert_eq!(result, "a: |\n  123\n  456\n  789\n");
    }

    #[test]
    fn format_block_literal_multi_entry_map() {
        let input = "a: |\n  123\n  456\n  789\nb: |1\n    123\n   456\n  789\nd: |\n  123\n  456\n  789\n\nc: 0\n";
        let result = format_yaml(input, &YamlFormatOptions::default()).expect("format");
        assert_eq!(result, input);
    }

    #[test]
    fn format_flow_seq_alias_key_flat() {
        let input = "[&123 foo, *123 : 456]\n";
        let result = format_yaml(input, &YamlFormatOptions::default()).expect("format");
        assert_eq!(result, "[&123 foo, *123 : 456]\n");
    }
}
