#![doc = include_str!("../README.md")]

mod highlight;
mod render;
mod wrap;

use pulldown_cmark::{Options as CmarkOptions, Parser};

pub use highlight::has_syntax;

pub(crate) use ansi_term_codes::{BLUE, BOLD, ITALIC, RESET, UNDERLINE};

/// Options controlling rendering behavior.
pub struct Options {
    /// Whether to syntax-highlight fenced code blocks.
    pub syntax_highlight: bool,
    /// Available column width for text wrapping and code block background padding.
    /// `None` means no wrapping; code blocks default to 80 columns.
    pub width: Option<usize>,
}

/// Render markdown to ANSI text (block-level: paragraphs, headers, code blocks, lists).
pub fn render(text: &str, opts: &Options) -> String {
    let parser = Parser::new_ext(text, CmarkOptions::empty());
    render::render_events(parser, opts, false)
}

/// Render inline markdown only (code spans, bold, links, raw URLs).
///
/// The input is parsed as a full `CommonMark` document but outer paragraph
/// wrapper events are stripped so only inline content is emitted.
pub fn render_inline(text: &str, opts: &Options) -> String {
    let parser = Parser::new_ext(text, CmarkOptions::empty());
    render::render_events(parser, opts, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            syntax_highlight: true,
            width: None,
        }
    }

    #[test]
    fn paragraph_reflow() {
        let input = "first line\nsecond line";
        let result = render(input, &opts());
        assert!(
            result.contains("first line second line"),
            "Single newlines should become spaces, got: {result:?}"
        );
    }

    #[test]
    fn paragraph_break_preserved() {
        let input = "first paragraph\n\nsecond paragraph";
        let result = render(input, &opts());
        assert!(
            result.contains("first paragraph\n\nsecond paragraph"),
            "Double newlines should preserve paragraph breaks, got: {result:?}"
        );
    }

    #[test]
    fn heading_bolded() {
        let result = render("# My Header\nSome text", &opts());
        assert!(result.contains(BOLD));
        assert!(result.contains("My Header"));
        assert!(!result.contains("# "));
    }

    #[test]
    fn inline_code_blue() {
        let result = render_inline("Use `foo` and `bar`", &opts());
        assert!(result.contains(BLUE));
        assert!(result.contains("foo"));
        assert!(result.contains("bar"));
        assert!(!result.contains('`'));
    }

    #[test]
    fn inline_bold() {
        let result = render_inline("This is **important** text", &opts());
        assert!(result.contains(BOLD));
        assert!(result.contains("important"));
        assert!(!result.contains("**"));
    }

    #[test]
    fn inline_link_osc8() {
        let result = render_inline("See [docs](https://example.com) here", &opts());
        assert!(
            result.contains("\x1b]8;;https://example.com\x1b\\"),
            "should contain OSC 8 hyperlink, got: {result:?}"
        );
        assert!(result.contains(UNDERLINE));
        assert!(result.contains("docs"));
        assert!(result.contains("\x1b]8;;\x1b\\"));
    }

    #[test]
    fn code_block_highlighted() {
        let input = "Some text\n\n```toml\n[package]\nname = \"test\"\n```\n\nMore text";
        let result = render(input, &opts());
        assert!(result.contains("\x1b["));
        assert!(!result.contains("```"));
        assert!(result.contains("package"));
        assert!(result.contains("test"));
        assert!(result.contains("Some text"));
        assert!(result.contains("More text"));
    }

    #[test]
    fn code_block_no_syntax_highlight_preserves_fences() {
        let no_highlight = Options {
            syntax_highlight: false,
            width: None,
        };
        let input = "Before\n\n```toml\n[package]\nname = \"test\"\n```\n\nAfter";
        let result = render(input, &no_highlight);
        assert!(result.contains("```toml"));
        assert!(result.contains("[package]"));
        assert!(result.contains("name = \"test\""));
        assert!(!result.contains("\x1b[38;2;"));
        assert!(result.contains("Before"));
        assert!(result.contains("After"));
    }

    #[test]
    fn unordered_list() {
        let input = "- first\n- second\n- third";
        let result = render(input, &opts());
        assert!(result.contains("- first"));
        assert!(result.contains("- second"));
        assert!(result.contains("- third"));
    }

    #[test]
    fn ordered_list() {
        let input = "1. first\n2. second\n3. third";
        let result = render(input, &opts());
        assert!(result.contains("1. first"));
        assert!(result.contains("2. second"));
        assert!(result.contains("3. third"));
    }

    #[test]
    fn syntect_knows_toml() {
        assert!(
            has_syntax("toml"),
            "syntect should have a TOML syntax definition"
        );
    }

    #[test]
    fn toml_code_block_highlights_keys_and_strings() {
        let input = "```toml\n[package]\nname = \"test\"\n```";
        let result = render(input, &opts());
        let ansi_count = result.matches("\x1b[38;2;").count();
        assert!(
            ansi_count >= 2,
            "TOML highlighting should produce multiple color codes, got {ansi_count}"
        );
    }

    #[test]
    fn render_inline_strips_paragraph() {
        let result = render_inline("hello world", &opts());
        assert_eq!(result, "hello world");
    }

    #[test]
    fn list_items_on_separate_lines() {
        let input = "- first\n- second\n- third";
        let result = render(input, &opts());
        assert_eq!(result, "- first\n- second\n- third\n");
    }

    #[test]
    fn loose_list_items_on_separate_lines() {
        let input = "- first\n\n- second\n\n- third";
        let result = render(input, &opts());
        assert_eq!(result, "- first\n- second\n- third\n");
    }

    #[test]
    fn list_separated_from_paragraphs() {
        let input = "Some text.\n\n- item one\n- item two\n\nMore text.";
        let result = render(input, &opts());
        assert_eq!(
            result,
            "Some text.\n\n- item one\n- item two\n\nMore text.\n"
        );
    }

    #[test]
    fn code_block_followed_by_text() {
        let input = "```json\n{}\n```\n\nAfter code.";
        let result = render(input, &opts());
        assert!(
            result.contains("\n\nAfter code."),
            "Should have blank line after code block, got: {result:?}"
        );
    }

    #[test]
    fn paragraph_wrapping() {
        let input =
            "This is a long paragraph that should be wrapped at a specific width for display.";
        let result = render(
            input,
            &Options {
                syntax_highlight: true,
                width: Some(30),
            },
        );
        for line in result.trim_end().split('\n') {
            assert!(
                line.len() <= 30,
                "Line should be at most 30 chars, got {} chars: {line:?}",
                line.len()
            );
        }
        assert!(result.contains("This"));
        assert!(result.contains("display."));
    }

    #[test]
    fn list_item_wrapping_accounts_for_prefix() {
        let input = "- aaa bbb ccc ddd eee";
        let result = render(
            input,
            &Options {
                syntax_highlight: true,
                width: Some(20),
            },
        );
        for line in result.trim_end().split('\n') {
            assert!(
                line.len() <= 20,
                "Line should be at most 20 chars, got {} chars: {line:?}",
                line.len()
            );
        }
        let lines: Vec<&str> = result.trim_end().split('\n').collect();
        assert!(
            lines[0].starts_with("- "),
            "First line should start with '- ', got: {:?}",
            lines[0]
        );
        if lines.len() > 1 {
            assert!(
                lines[1].starts_with("  "),
                "Continuation line should start with 2-space indent, got: {:?}",
                lines[1]
            );
        }
    }
}
