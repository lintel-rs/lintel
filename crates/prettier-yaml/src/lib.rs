extern crate alloc;

mod ast;
mod comments;
mod parser;
mod print;
mod printer;
mod utilities;

use anyhow::Result;

pub use prettier_config::{self, PrettierConfig, ProseWrap};

/// Backwards-compatible type alias.
pub type YamlFormatOptions = PrettierConfig;

/// Format YAML content with prettier-compatible output.
///
/// # Errors
///
/// Returns an error if the content is not valid YAML.
pub fn format_yaml(content: &str, options: &PrettierConfig) -> Result<String> {
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
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "a: 1\nb: 2\n");
    }

    #[test]
    fn format_simple_sequence() {
        let input = "- 1\n- 2\n- 3\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "- 1\n- 2\n- 3\n");
    }

    #[test]
    fn format_null_values() {
        let input = "a:\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "a:\n");
    }

    #[test]
    fn format_nested_mapping() {
        let input = "key:\n  nested: value\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "key:\n  nested: value\n");
    }

    #[test]
    fn format_sequence_of_mappings() {
        let input = "- a: b\n  c: d\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "- a: b\n  c: d\n");
    }

    #[test]
    fn format_block_literal_clip() {
        let input = "|\n    123\n    456\n    789\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "|\n  123\n  456\n  789\n");
    }

    #[test]
    fn format_block_literal_keep() {
        let input = "|+\n    123\n    456\n    789\n\n\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "|+\n  123\n  456\n  789\n\n\n");
    }

    #[test]
    fn format_block_literal_in_mapping() {
        let input = "a: |\n  123\n  456\n  789\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(result, "a: |\n  123\n  456\n  789\n");
    }

    #[test]
    fn format_block_literal_multi_entry_map() {
        let input = "a: |\n  123\n  456\n  789\nb: |1\n    123\n   456\n  789\nd: |\n  123\n  456\n  789\n\nc: 0\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, input);
    }

    #[test]
    fn format_block_scalar_header_comments_not_duplicated() {
        // Verify header comments aren't duplicated as leading comments of next item
        let input = "- | # Empty header\n  literal\n- >1 # Indentation indicator\n   folded\n- |+ # Chomping indicator\n  keep\n\n- >1- # Both indicators\n   strip\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        // Comments should only appear on the block scalar header line, not duplicated
        assert!(!result.contains("\n# Indentation indicator\n"));
        assert!(!result.contains("\n# Chomping indicator\n"));
        assert!(!result.contains("\n# Both indicators\n"));
        // All four block scalar headers should be present
        assert!(result.contains("| # Empty header"));
        assert!(result.contains(">1 # Indentation indicator"));
        assert!(result.contains("|+ # Chomping indicator"));
        assert!(result.contains(">1- # Both indicators"));
    }

    #[test]
    fn format_flow_seq_alias_key_flat() {
        let input = "[&123 foo, *123 : 456]\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "[&123 foo, *123 : 456]\n");
    }

    #[test]
    fn format_key_trailing_comment_with_value() {
        // spec-example-6-9: trailing comment on key line, value on next line
        let input = "key:    # Comment\n  value\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "key: # Comment\n  value\n");
    }

    #[test]
    fn format_multiline_comments_on_key() {
        // spec-example-6-11: multiple trailing comments
        let input = "key:    # Comment\n        # lines\n  value\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, "key: # Comment\n  # lines\n  value\n");
    }

    #[test]
    fn format_explicit_key_with_leading_comment() {
        let input = "? # comment\n  key\n: value\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(result, "? # comment\n  key\n: value\n");
    }

    #[test]
    fn format_explicit_key_with_between_comment() {
        // Prettier keeps explicit key format when between_comments exist
        let input = "? key\n# comment\n: longlonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglong\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(
            result,
            "? key\n# comment\n: longlonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglong\n"
        );
    }

    #[test]
    fn format_spec_2_9_usetabs() {
        let input = "---\nhr: # 1998 hr ranking\n  - Mark McGwire\n  - Sammy Sosa\nrbi:\n  # 1998 rbi ranking\n  - Sammy Sosa\n  - Ken Griffey\n";
        let opts = PrettierConfig {
            use_tabs: true,
            ..Default::default()
        };
        let result = format_yaml(input, &opts).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(result, input);
    }

    #[test]
    fn format_spec_2_9_default() {
        let input = "---\nhr: # 1998 hr ranking\n  - Mark McGwire\n  - Sammy Sosa\nrbi:\n  # 1998 rbi ranking\n  - Sammy Sosa\n  - Ken Griffey\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(result, input);
    }

    #[test]
    fn format_doc_end_comment() {
        let input = "%YAML 1.2\n---\nDocument\n... # Suffix\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(result, input);
    }

    #[test]
    fn format_empty_scalar_chomping() {
        let input = "strip: >-\n\nclip: >\n\nkeep: |+\n\n\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(result, input);
    }

    #[test]
    fn format_anchor_with_trailing_comment() {
        let input = "key1: &default # This key ...\n  subkey1: value1\n\nkey2:\n  <<: *default\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(
            result,
            "key1: &default # This key ...\n  subkey1: value1\n\nkey2:\n  <<: *default\n"
        );
    }

    #[test]
    fn format_spec_88_literal_content() {
        // spec-example-8-8: literal block scalar with trailing comment
        let input = "|\n \n  \n  literal\n   \n  \n  text\n\n # Comment\n";
        let opts = PrettierConfig {
            prose_wrap: ProseWrap::Always,
            ..Default::default()
        };
        let result = format_yaml(input, &opts).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(result, "|\n\n\n  literal\n   \n\n  text\n\n# Comment\n");
    }

    #[test]
    fn format_spec_85_chomping_trailing() {
        // spec-example-8-5: block scalars with trailing comments.
        // After block scalar values, off-grid comments (col 1) normalize to
        // the structural indent (col 0) via group-min floor snapping.
        let input = "# Strip\n# Comments:\nstrip: |-\n  # text\n \n # Clip\n # comments:\n \nclip: |\n  # text\n\n # Keep\n # comments:\n\nkeep: |+\n  # text\n\n # Trail\n # comments.\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        let expected = "# Strip\n# Comments:\nstrip: |-\n  # text\n\n# Clip\n# comments:\n\nclip: |\n  # text\n\n# Keep\n# comments:\n\nkeep: |+\n  # text\n\n# Trail\n# comments.\n";
        assert_eq!(result, expected);
    }

    #[test]
    fn format_explicit_key_with_comment_between() {
        // explicit-key.yml: comment between long key and its value
        let input = "solongitshouldbreakbutitcannot_longlonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglonglong:\n  # Comment\n  foo: bar\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(result, input);
    }

    #[test]
    fn format_collection_comments() {
        // collection.yml: document trailing comment at col 0
        let input = "f:\n  - a\n  # b.leadingComments\n  - b\n    # b.endComments\n  - c\n    # c.endComments\n  # sequence.endComments\n# documentBody.children\n\nempty_content:\n  # hello world\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(result, input);
    }

    #[test]
    fn format_object_comments_blank_lines() {
        // object.yml: blank lines between comments and entries
        let input =
            "#6445\n\nobj:\n  # before\n\n  # before\n\n  key: value\n\n  # after\n\n  # after\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, input);
    }

    #[test]
    fn debug_spec_88_ast() {
        let input = "|\n \n  \n  literal\n   \n  \n  text\n\n # Comment\n";
        let events = crate::parser::collect_events(input).expect("parse");
        let comments = crate::comments::extract_comments(input);
        eprintln!("COMMENTS:");
        for (i, c) in comments.iter().enumerate() {
            eprintln!(
                "  {} line={} col={} whole={} text={:?}",
                i, c.line, c.col, c.whole_line, c.text
            );
        }
        eprintln!("EVENTS:");
        for (i, (ev, sp)) in events.iter().enumerate() {
            eprintln!(
                "  {} {:?} {}:{}-{}:{}",
                i,
                ev,
                sp.start.line(),
                sp.start.col(),
                sp.end.line(),
                sp.end.col()
            );
        }
        let mut builder = crate::parser::AstBuilder::new(input, &events, &comments);
        let stream = builder.build_stream().expect("build");
        eprintln!(
            "STREAM trailing_comments: {:?}",
            stream.trailing_comments.len()
        );
        for c in &stream.trailing_comments {
            eprintln!(
                "  line={} col={} blank_before={} text={:?}",
                c.line, c.col, c.blank_line_before, c.text
            );
        }
        for (i, doc) in stream.documents.iter().enumerate() {
            eprintln!("DOC {}: end_comments={}", i, doc.end_comments.len());
            for c in &doc.end_comments {
                eprintln!(
                    "  line={} col={} blank_before={} text={:?}",
                    c.line, c.col, c.blank_line_before, c.text
                );
            }
        }
    }

    #[test]
    fn format_anchor_with_between_comment() {
        // Comment between anchor and first entry should be promoted to
        // middle_comment and printed inline with the anchor.
        let input =
            "key1: &default\n\n  # This key ...\n  subkey1: value1\n\nkey2:\n  <<: *default\n";
        let expected =
            "key1: &default # This key ...\n  subkey1: value1\n\nkey2:\n  <<: *default\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert_eq!(result, expected);
    }

    #[test]
    fn format_directives_comments() {
        // directives-and-comments.yml: comment after --- before body
        let input = "# 123\n%YAML 1.2\n# 456\n---\n# 789\ntest\n# 000\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(result, input);
    }

    #[test]
    fn format_omap_flow_style_comment() {
        let input = "omap:\n  Bestiary: !!omap\n    - aardvark: ant eater\n    - anaconda: snake\n    # Etc.\n  # Flow style\n  Numbers: !!omap [one: 1, two: 2, three: 3]\n";
        let events = crate::parser::collect_events(input).expect("parse");
        let comments = crate::comments::extract_comments(input);
        eprintln!("COMMENTS:");
        for (i, c) in comments.iter().enumerate() {
            eprintln!(
                "  {} line={} col={} whole={} text={:?}",
                i, c.line, c.col, c.whole_line, c.text
            );
        }
        eprintln!("EVENTS:");
        for (i, (ev, sp)) in events.iter().enumerate() {
            eprintln!(
                "  {} {:?} {}:{}-{}:{}",
                i,
                ev,
                sp.start.line(),
                sp.start.col(),
                sp.end.line(),
                sp.end.col()
            );
        }
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(result, input);
    }

    #[test]
    fn format_spec_6_21_local_tag_prefix() {
        let input = "%TAG !m! !my-\n--- # Bulb here\n!m!light fluorescent\n...\n%TAG !m! !my-\n--- # Color here\n!m!light green\n";
        let events = crate::parser::collect_events(input).expect("parse");
        eprintln!("EVENTS:");
        for (i, (ev, sp)) in events.iter().enumerate() {
            eprintln!(
                "  {} {:?} {}:{}-{}:{}",
                i,
                ev,
                sp.start.line(),
                sp.start.col(),
                sp.end.line(),
                sp.end.col()
            );
        }
        let comments = crate::comments::extract_comments(input);
        eprintln!("COMMENTS:");
        for (i, c) in comments.iter().enumerate() {
            eprintln!(
                "  {} line={} col={} whole={} text={:?}",
                i, c.line, c.col, c.whole_line, c.text
            );
        }
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result: {result:?}");
        assert_eq!(result, input);
    }

    #[test]
    fn format_flow_seq_with_comments() {
        let input = "a: [\n    check-format,\n    check-lint,\n    check-spelling,\n    # coverage,\n    # install-and-run-from-git,\n  ]\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result:\n{result}");
        assert_eq!(result, input);
    }

    #[test]
    fn format_prettier_ignore_flow_seq() {
        let input = "d:\n  # prettier-ignore\n  [\n        check-format, check-lint,\n        check-spelling,\n        # coverage,\n        # install-and-run-from-git,\n      ]\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("input:\n{input}");
        eprintln!("result:\n{result}");
        assert_eq!(result, input);
    }

    #[test]
    fn format_explicit_key_flow_seq() {
        let input = "[aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n\n[aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]:\n  aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n\n? [\n    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\n  ]\n: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n";
        let expected = "[aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n\n[aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]:\n  aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n\n? [\n    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\n  ]\n: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result:\n{result}");
        assert_eq!(result, expected);
    }

    #[test]
    fn format_flow_map_with_comments() {
        let input = "b: {\n    a: check-format,\n    b: check-lint,\n    c: check-spelling,\n    # d: coverage,\n    # e: install-and-run-from-git,\n  }\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("result:\n{result}");
        assert_eq!(result, input);
    }

    #[test]
    fn quoted_string_in_flow_sequence() {
        let input = "# Outside flow collection:\n- ::vector\n- \": - ()\"\n- Up, up, and away!\n- -123\n- http://example.com/foo#bar\n# Inside flow collection:\n- [ ::vector,\n  \": - ()\",\n  \"Up, up and away!\",\n  -123,\n  http://example.com/foo#bar ]\n";
        let options = PrettierConfig {
            use_tabs: true,
            ..Default::default()
        };
        let result = format_yaml(input, &options).expect("format");
        assert!(result.contains("Up, up and away!"), "String content lost!");
    }

    #[test]
    fn bare_documents_93() {
        let input =
            "Bare\ndocument\n...\n# No document\n...\n|\n%!PS-Adobe-2.0 # Not the first line\n";
        let options = PrettierConfig {
            prose_wrap: ProseWrap::Always,
            ..Default::default()
        };
        let result = format_yaml(input, &options).expect("format");
        eprintln!("result: {result:?}");
        let expected =
            "Bare document\n...\n# No document\n...\n|\n  %!PS-Adobe-2.0 # Not the first line\n";
        assert_eq!(result, expected);
    }

    #[test]
    fn trailing_comment_multiline_quoted_scalar() {
        let input = "a: \"double\n  quotes\" # lala\nb: plain\n value  # lala\n";
        let options = PrettierConfig {
            prose_wrap: ProseWrap::Always,
            ..Default::default()
        };
        let result = format_yaml(input, &options).expect("format");
        let expected = "a: \"double quotes\" # lala\nb: plain value # lala\n";
        assert_eq!(result, expected);
    }

    #[test]
    fn trailing_comments_explicit_key_value() {
        let input = "? # lala\n - seq1\n: # lala\n - #lala\n  seq2\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        let expected = "? # lala\n  - seq1\n: # lala\n  - #lala\n    seq2\n";
        assert_eq!(result, expected);
    }

    #[test]
    fn comment_between_set_entries() {
        let input = "set:\n  a: !!set\n    ? X\n    ? Y\n  # Flow\n  b: !!set { Z }\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert!(result.contains("# Flow"), "missing comment");
    }

    #[test]
    fn spec_818_trailing_comment_on_empty_key_value() {
        let input = "plain key: in-line value\n: # Both empty\n\"quoted key\":\n  - entry\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert!(result.contains("# Both empty"), "trailing comment dropped");
    }

    #[test]
    fn indicator_comment_on_seq_mapping_item() {
        let input = "merge:\n  - # Explicit keys\n    x: 1\n    y: 2\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert!(
            result.contains("# Explicit keys"),
            "indicator comment dropped on mapping item"
        );
    }

    #[test]
    fn flow_seq_trailing_comment_after_bracket() {
        let input =
            "key: !!seq [\n    Mars, # Rocks\n    Neptune, # Gas\n    Pluto,\n  ] # Overrated\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        eprintln!("RESULT:\n{result}");
        assert!(
            result.contains("# Overrated"),
            "# Overrated comment dropped"
        );
        assert!(result.contains("# Rocks"), "# Rocks comment dropped");
        assert!(result.contains("# Gas"), "# Gas comment dropped");
    }

    #[test]
    fn flow_seq_comments_with_tag() {
        let input = "seq:\n  Block style: !!seq\n    - Mercury # Rotates\n    - Pluto\n  Flow style: !!seq [\n      Mars, # Rocks\n      Neptune, # Gas\n      Pluto,\n    ] # Overrated\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        assert!(result.contains("Mars, # Rocks"), "# Rocks misplaced");
        assert!(result.contains("] # Overrated"), "# Overrated misplaced");
    }

    #[test]
    fn blank_line_after_block_scalar() {
        // Block scalar ends with `}`, then blank line, then comment
        let input = "fn: !!js/function >\n  function foobar() {\n    return 'Wow!';\n  }\n\n\n# Custom types\nkey: val\n";
        let result = format_yaml(input, &PrettierConfig::default()).expect("format");
        // Should have a blank line before `# Custom types`
        assert!(
            result.contains("}\n\n# Custom types"),
            "Missing blank line after block scalar body: {result}",
        );
    }
}
