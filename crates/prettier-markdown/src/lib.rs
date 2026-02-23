#![doc = include_str!("../README.md")]

mod ast_to_doc;
mod frontmatter;

use anyhow::Result;
use core::fmt::Write as _;
use prettier_config::PrettierConfig;

/// Format markdown content with prettier-compatible output.
///
/// Handles YAML frontmatter extraction and formatting, then formats the
/// markdown body using the Wadler-Lindig pretty-printing algorithm.
///
/// # Errors
///
/// Returns an error if the YAML frontmatter cannot be parsed or formatted.
pub fn format_markdown(content: &str, options: &PrettierConfig) -> Result<String> {
    let mut output = String::new();

    let body = if let Some(fm) = frontmatter::extract_frontmatter(content) {
        if let Some(lang) = fm.language {
            // Non-YAML frontmatter (e.g., ---toml): pass through as-is
            let _ = writeln!(output, "---{lang}");
            output.push_str(fm.content);
            output.push_str("---\n");
        } else {
            // YAML frontmatter: format with prettier-yaml
            output.push_str("---\n");
            if !fm.content.is_empty() {
                let formatted_yaml = prettier_yaml::format_yaml(fm.content, options)?;
                output.push_str(&formatted_yaml);
                if !formatted_yaml.ends_with('\n') {
                    output.push('\n');
                }
            }
            output.push_str("---\n");
        }
        fm.body
    } else {
        content
    };

    // Format the markdown body
    let formatted_body = format_body(body, options);

    // If we emitted frontmatter and the body is non-empty, add a blank line
    if !output.is_empty() && !formatted_body.is_empty() {
        output.push('\n');
    }

    output.push_str(&formatted_body);

    // Ensure trailing newline
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }

    Ok(output)
}

/// Format the markdown body (everything after frontmatter).
fn format_body(body: &str, options: &PrettierConfig) -> String {
    let doc = ast_to_doc::markdown_to_doc(body, options);
    wadler_lindig::print(&doc, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prettier_config::ProseWrap;

    #[test]
    fn format_simple_paragraph() {
        let input = "Hello world.\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "Hello world.\n");
    }

    #[test]
    fn format_heading() {
        let input = "# Hello\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "# Hello\n");
    }

    #[test]
    fn format_heading_levels() {
        for level in 1..=6 {
            let hashes = "#".repeat(level);
            let input = format!("{hashes} Title\n");
            let result = format_markdown(&input, &PrettierConfig::default()).expect("format");
            assert_eq!(result, format!("{hashes} Title\n"));
        }
    }

    #[test]
    fn format_yaml_frontmatter() {
        let input = "---\ntitle: Hello\nauthor: World\n---\n\n# Heading\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        assert!(result.starts_with("---\n"));
        assert!(result.contains("title: Hello"));
        assert!(result.contains("---\n"));
        assert!(result.contains("# Heading"));
    }

    #[test]
    fn format_code_block() {
        let input = "```js\nconst x = 1;\n```\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        assert!(result.contains("```js\n"));
        assert!(result.contains("const x = 1;\n"));
        assert!(result.contains("```\n"));
    }

    #[test]
    fn format_prose_wrap_always() {
        let opts = PrettierConfig {
            print_width: 30,
            prose_wrap: ProseWrap::Always,
            ..Default::default()
        };
        let input = "This is a long sentence that should wrap at the specified width.\n";
        let result = format_markdown(input, &opts).expect("format");
        // Lines should not exceed print_width significantly
        for line in result.lines() {
            // Allow some overflow for single words longer than print_width
            assert!(
                line.len() <= 40,
                "Line too long ({} chars): {line:?}",
                line.len()
            );
        }
    }

    #[test]
    fn format_prose_wrap_never() {
        let opts = PrettierConfig {
            print_width: 30,
            prose_wrap: ProseWrap::Never,
            ..Default::default()
        };
        let input = "This is a\nsentence that\nwas broken\ninto many lines.\n";
        let result = format_markdown(input, &opts).expect("format");
        // All lines should be joined into one paragraph
        assert!(
            !result.trim().contains('\n') || result.lines().count() <= 2,
            "Should join lines: {result:?}"
        );
    }

    #[test]
    fn format_prose_wrap_preserve() {
        let opts = PrettierConfig {
            prose_wrap: ProseWrap::Preserve,
            ..Default::default()
        };
        let input = "Line one.\nLine two.\n";
        let result = format_markdown(input, &opts).expect("format");
        // Preserve should maintain original line structure
        assert!(
            result.contains("Line one.\n"),
            "should preserve lines: {result:?}"
        );
        assert!(
            result.contains("Line two.\n"),
            "should preserve lines: {result:?}"
        );
    }

    #[test]
    fn format_ordered_list() {
        let input = "1. First\n2. Second\n3. Third\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        // Prettier uses marker + 1 space (no tabWidth alignment)
        assert!(result.contains("1. First"), "got: {result:?}");
        assert!(result.contains("2. Second"), "got: {result:?}");
        assert!(result.contains("3. Third"), "got: {result:?}");
    }

    #[test]
    fn format_unordered_list() {
        let input = "- First\n- Second\n- Third\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        assert!(result.contains("- First"));
        assert!(result.contains("- Second"));
        assert!(result.contains("- Third"));
    }

    #[test]
    fn format_thematic_break() {
        let input = "Above\n\n---\n\nBelow\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        assert!(
            result.contains("---"),
            "should contain thematic break: {result:?}"
        );
    }

    #[test]
    fn format_emphasis_and_strong() {
        let input = "This is _emphasis_ and **strong**.\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        assert!(
            result.contains("_emphasis_"),
            "should contain emphasis: {result:?}"
        );
        assert!(
            result.contains("**strong**"),
            "should contain strong: {result:?}"
        );
    }

    #[test]
    fn format_inline_code() {
        let input = "Use `code` here.\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        assert!(
            result.contains("`code`"),
            "should contain inline code: {result:?}"
        );
    }

    #[test]
    fn format_link() {
        let input = "Click [here](https://example.com) now.\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        assert!(
            result.contains("[here](https://example.com)"),
            "should contain link: {result:?}"
        );
    }

    #[test]
    fn format_image() {
        let input = "![alt text](image.png)\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        assert!(
            result.contains("![alt text](image.png)"),
            "should contain image: {result:?}"
        );
    }

    #[test]
    fn format_blockquote() {
        let input = "> Quoted text\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        assert!(
            result.contains("> Quoted text"),
            "should contain blockquote: {result:?}"
        );
    }

    #[test]
    fn format_empty_input() {
        let input = "";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        // Empty input should produce empty output (prettier behavior)
        assert_eq!(result, "");
    }

    #[test]
    fn format_blank_line_normalization() {
        let input = "Paragraph one.\n\n\n\n\nParagraph two.\n";
        let result = format_markdown(input, &PrettierConfig::default()).expect("format");
        // Should not have more than one blank line between paragraphs
        assert!(
            !result.contains("\n\n\n"),
            "should collapse blank lines: {result:?}"
        );
    }

    #[test]
    fn debug_codeblock_list() {
        let input = "1. ol01\n\n    ```js\n    const a = 1;\n\n\n    const b = 2;\n    ```\n\n2. ol02\n\n    ```js\n    const a = 1;\n\n\n    const b = 2;\n    ```\n";
        let opts = PrettierConfig {
            tab_width: 0,
            prose_wrap: ProseWrap::Always,
            ..Default::default()
        };
        let result = format_markdown(input, &opts).expect("format");
        eprintln!("RESULT:\n{result}");
        let expected = "1. ol01\n\n   ```js\n   const a = 1;\n\n   const b = 2;\n   ```\n\n2. ol02\n\n   ```js\n   const a = 1;\n\n   const b = 2;\n   ```\n";
        assert_eq!(result, expected);
    }

    #[test]
    fn debug_tab_codeblock() {
        let input = "* Text\n\n \t```\n \tfoo\n \t```\n";
        let opts = PrettierConfig {
            tab_width: 0,
            prose_wrap: ProseWrap::Always,
            ..Default::default()
        };
        let result = format_markdown(input, &opts).expect("format");
        eprintln!("RESULT:\n{result}");
        let expected = "- Text\n\n  ```\n  foo\n  ```\n";
        assert_eq!(result, expected);
    }

    #[test]
    fn format_list_followed_by_code() {
        let opts = PrettierConfig {
            prose_wrap: ProseWrap::Always,
            ..Default::default()
        };

        let cases: &[(&str, &str)] = &[
            (
                "-    foo\n\n    top level indented code block\n",
                "-    foo\n\n\n    top level indented code block\n",
            ),
            (
                "   -    foo\n\n       top level indented code block\n",
                "   -    foo\n\n\n       top level indented code block\n",
            ),
            (
                "1.    foo\n\n     top level indented code block\n",
                "1.    foo\n\n\n     top level indented code block\n",
            ),
            (
                "1. item 1\n    1. item 1-1\n    2. item 1-2\n100. item 1\n\n    top level indented code block\n",
                "1.   item 1\n     1. item 1-1\n     2. item 1-2\n2.   item 1\n\n\n    top level indented code block\n",
            ),
            (
                "   -    item 1\n        -    item 1-1\n        -    item 1-2\n\n            indented code block\n\n    top level indented code block\n",
                "-    item 1\n     -    item 1-1\n     -    item 1-2\n\n\n         indented code block\n\n\n    top level indented code block\n",
            ),
        ];

        for (input, expected) in cases {
            let result = format_markdown(input, &opts).expect("format");
            assert_eq!(&result, expected, "input: {input:?}");
        }
    }

    #[test]
    fn format_escape_table() {
        let input = "| a | b | c |\n|:--|:-:|--:|\n| \\| | \\| | \\| |\n";
        let opts = PrettierConfig {
            prose_wrap: ProseWrap::Always,
            ..Default::default()
        };
        let result = format_markdown(input, &opts).expect("format");
        let expected = "| a   |  b  |   c |\n| :-- | :-: | --: |\n| \\|  | \\|  |  \\| |\n";
        assert_eq!(result, expected);
    }
}
