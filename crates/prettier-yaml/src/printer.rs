use std::fmt::Write;

use saphyr_parser::ScalarStyle;

use crate::ProseWrap;
use crate::YamlFormatOptions;
use crate::ast::{Node, ScalarNode, SequenceNode, YamlStream};
use crate::print::block::format_block_scalar;
use crate::print::flow::{format_flow_mapping, format_flow_sequence};
use crate::print::mapping_item::format_block_mapping;
use crate::print::misc::{comment_indent, comment_indent_capped, indent_str};
use crate::utilities::{has_node_props, is_block_scalar_value, is_null_value, is_simple_value};

pub(crate) fn format_stream(stream: &YamlStream, options: &YamlFormatOptions) -> String {
    let mut output = String::new();

    for (i, doc) in stream.documents.iter().enumerate() {
        // Blank line between documents only when the next document has a preamble
        // (comments) and the prior document didn't end with `...` separator.
        let prev_had_end_marker = i > 0 && stream.documents[i - 1].explicit_end;
        if i > 0
            && !doc.preamble.is_empty()
            && !prev_had_end_marker
            && !output.is_empty()
            && !output.ends_with("\n\n")
        {
            output.push('\n');
        }

        // Write preamble (directives and comments before ---)
        for line in &doc.preamble {
            output.push_str(line);
            output.push('\n');
        }

        // Write document start marker
        if doc.explicit_start {
            output.push_str("---");
            output.push('\n');
        }

        // Write root node
        if let Some(root) = &doc.root {
            format_node(root, &mut output, 0, options, true, false);
        }

        // Ensure content ends with newline before next doc
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }

        // Write document end marker
        if doc.explicit_end {
            output.push_str("...\n");
        }

        // Write end comments
        for comment in &doc.end_comments {
            output.push_str(&comment.text);
            output.push('\n');
        }

        let _ = i; // suppress unused warning
    }

    // Write trailing stream comments
    for comment in &stream.trailing_comments {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        // Preserve blank line before stream-level comments
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        let ci = comment_indent(comment, 0, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }

    // Ensure output ends with newline (don't strip extra newlines - may be from |+ block scalars)
    if !output.ends_with('\n') && !output.is_empty() {
        output.push('\n');
    }

    output
}

pub(crate) fn format_node(
    node: &Node,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
    is_top: bool,
    inline: bool,
) {
    match node {
        Node::Scalar(s) => format_scalar(s, output, depth, options, 0),
        Node::Mapping(m) => {
            if m.flow {
                format_flow_mapping(m, output, depth, options);
            } else {
                format_block_mapping(m, output, depth, options, is_top, inline);
            }
        }
        Node::Sequence(s) => {
            if s.flow {
                format_flow_sequence(s, output, depth, options);
            } else {
                format_block_sequence(s, output, depth, options, is_top, inline);
            }
        }
        Node::Alias(a) => {
            output.push('*');
            output.push_str(&a.name);
        }
    }
}

pub(crate) fn format_scalar(
    s: &ScalarNode,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
    first_line_prefix: usize,
) {
    // Write tag
    if let Some(tag) = &s.tag {
        output.push_str(tag);
        output.push(' ');
    }

    // Write anchor
    if let Some(anchor) = &s.anchor {
        output.push('&');
        output.push_str(anchor);
        if !s.is_implicit_null {
            output.push(' ');
        }
    }

    if s.is_implicit_null {
        // Implicit null: don't write anything
        return;
    }

    // Write middle comments (between tag/anchor and content)
    if !s.middle_comments.is_empty() {
        let indent = indent_str(depth, options);
        if s.middle_comments.len() == 1 {
            // Single comment: on same line as tag/anchor, then newline
            // Remove trailing space from tag/anchor
            if output.ends_with(' ') {
                output.pop();
            }
            output.push(' ');
            output.push_str(&s.middle_comments[0].text);
            output.push('\n');
        } else {
            // Multiple comments: tag/anchor on own line, then each comment
            // Remove trailing space from tag/anchor
            if output.ends_with(' ') {
                output.pop();
            }
            output.push('\n');
            for comment in &s.middle_comments {
                output.push_str(&indent);
                output.push_str(&comment.text);
                output.push('\n');
            }
        }
        // Value starts on new line with indent
        output.push_str(&indent);
    }

    match s.style {
        ScalarStyle::Plain => {
            if s.value == "~" {
                // Tilde null - keep as-is
                output.push('~');
            } else {
                format_plain_scalar(s, output, depth, options, first_line_prefix);
            }
        }
        ScalarStyle::SingleQuoted => {
            format_quoted_scalar(
                &s.value,
                s.quoted_source.as_deref(),
                output,
                depth,
                options,
                true,
            );
        }
        ScalarStyle::DoubleQuoted => {
            format_quoted_scalar(
                &s.value,
                s.quoted_source.as_deref(),
                output,
                depth,
                options,
                false,
            );
        }
        ScalarStyle::Literal | ScalarStyle::Folded => {
            format_block_scalar(s, output, depth, options);
        }
    }
}

/// Format a plain scalar value with proseWrap awareness.
fn format_plain_scalar(
    s: &ScalarNode,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
    first_line_prefix: usize,
) {
    let indent = indent_str(depth, options);

    match options.prose_wrap {
        ProseWrap::Always => {
            format_plain_wrap(
                &s.value,
                output,
                &indent,
                options.print_width,
                first_line_prefix,
            );
        }
        ProseWrap::Never => {
            format_plain_never(&s.value, output, &indent);
        }
        ProseWrap::Preserve => {
            if let Some(ref source_lines) = s.plain_source_lines {
                format_plain_preserve(source_lines, output, &indent);
            } else {
                // Single-line plain scalar: output as-is
                output.push_str(&s.value);
            }
        }
    }
}

/// `ProseWrap::Always` — Re-wrap at `print_width`.
/// Paragraph breaks (\n in value) are preserved as blank lines.
fn format_plain_wrap(
    value: &str,
    output: &mut String,
    indent: &str,
    print_width: usize,
    first_line_prefix: usize,
) {
    let parts: Vec<&str> = value.split('\n').collect();
    let mut first_content = true;
    let mut pending_blanks = 0usize;

    for part in &parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            if !first_content {
                pending_blanks += 1;
            }
            continue;
        }

        if !first_content {
            output.push_str("\n\n");
            for _ in 0..pending_blanks {
                output.push('\n');
            }
            output.push_str(indent);
        }
        pending_blanks = 0;

        // Word-wrap this paragraph
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let mut line_len = if first_content {
            first_line_prefix
        } else {
            indent.len()
        };
        let mut first_word = true;

        for word in &words {
            if first_word {
                output.push_str(word);
                line_len += word.len();
                first_word = false;
            } else if line_len + 1 + word.len() > print_width {
                output.push('\n');
                output.push_str(indent);
                output.push_str(word);
                line_len = indent.len() + word.len();
            } else {
                output.push(' ');
                output.push_str(word);
                line_len += 1 + word.len();
            }
        }

        first_content = false;
    }
}

/// `ProseWrap::Never` — Join words in each paragraph on one line.
fn format_plain_never(value: &str, output: &mut String, indent: &str) {
    let parts: Vec<&str> = value.split('\n').collect();
    let mut first_content = true;
    let mut pending_blanks = 0usize;

    for part in &parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            if !first_content {
                pending_blanks += 1;
            }
            continue;
        }

        if !first_content {
            output.push_str("\n\n");
            for _ in 0..pending_blanks {
                output.push('\n');
            }
            output.push_str(indent);
        }
        pending_blanks = 0;

        // Join all words on one line (no wrapping)
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        output.push_str(&words.join(" "));

        first_content = false;
    }
}

/// `ProseWrap::Preserve` — Use original source line structure.
fn format_plain_preserve(source_lines: &[String], output: &mut String, indent: &str) {
    let mut first_content = true;
    let mut pending_blanks = 0usize;

    for line in source_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !first_content {
                pending_blanks += 1;
            }
            continue;
        }

        if !first_content {
            output.push('\n');
            // Blank lines for paragraph breaks
            for _ in 0..pending_blanks {
                output.push('\n');
            }
            output.push_str(indent);
        }
        pending_blanks = 0;

        output.push_str(trimmed);
        first_content = false;
    }
}

/// Format a quoted scalar, choosing between single and double quotes based on prettier rules.
fn format_quoted_scalar(
    value: &str,
    raw_source: Option<&str>,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
    was_single_quoted: bool,
) {
    let contains_single = value.contains('\'');
    let contains_double = value.contains('"');
    let contains_newline = value.contains('\n');

    // Check if we have a multi-line raw source (originally multi-line in source)
    let raw_is_multiline = raw_source.is_some_and(|r| r.contains('\n'));

    // Determine which quote style to use
    let use_single = if contains_newline {
        false
    } else if options.single_quote {
        !contains_single || contains_double
    } else if contains_single && contains_double {
        let single_escape_count = value.chars().filter(|&c| c == '\'').count();
        let double_escape_count = value.chars().filter(|&c| c == '"' || c == '\\').count();
        single_escape_count <= double_escape_count
    } else {
        (contains_double || value.contains('\\')) && !contains_single
    };

    // For multi-line double-quoted strings, output in multi-line form
    if contains_newline
        && raw_is_multiline
        && !use_single
        && let Some(raw) = raw_source
    {
        format_multiline_double_quoted(raw, output, depth, options);
        return;
    }

    // Only use raw_source for single-line strings
    let raw_single_line = raw_source.filter(|r| !r.contains('\n'));

    if use_single {
        if was_single_quoted && let Some(raw) = raw_single_line {
            output.push('\'');
            output.push_str(raw);
            output.push('\'');
            return;
        }
        output.push('\'');
        output.push_str(&value.replace('\'', "''"));
        output.push('\'');
    } else {
        // Double-quoted output
        if !was_single_quoted && let Some(raw) = raw_single_line {
            output.push('"');
            output.push_str(raw);
            output.push('"');
            return;
        }
        output.push('"');
        output.push_str(&escape_double_quoted(value));
        output.push('"');
    }
}

/// Format a multi-line double-quoted string.
fn format_multiline_double_quoted(
    raw_source: &str,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
) {
    let indent = if depth == 0 {
        String::new()
    } else {
        indent_str(depth, options)
    };
    let lines: Vec<&str> = raw_source.split('\n').collect();

    output.push('"');
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            output.push_str(line);
        } else {
            output.push('\n');
            let trimmed = line.trim();
            if trimmed.is_empty() {
                // Blank line: keep empty
            } else {
                output.push_str(&indent);
                output.push_str(trimmed);
            }
        }
    }
    output.push('"');
}

fn escape_double_quoted(s: &str) -> String {
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
                let _ = write!(result, "\\x{:02X}", c as u32);
            }
            c => result.push(c),
        }
    }
    result
}

// ─── Block sequence formatting ─────────────────────────────────────────────────

#[allow(clippy::too_many_lines)]
pub(crate) fn format_block_sequence(
    seq: &SequenceNode,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
    is_top: bool,
    _inline: bool,
) {
    if seq.items.is_empty() {
        output.push_str("[]");
        return;
    }

    // Write tag and anchor
    let has_props = seq.tag.is_some() || seq.anchor.is_some();
    if has_props {
        if let Some(tag) = &seq.tag {
            output.push_str(tag);
        }
        if let Some(anchor) = &seq.anchor {
            if seq.tag.is_some() {
                output.push(' ');
            }
            output.push('&');
            output.push_str(anchor);
        }
        // Middle comments
        if seq.middle_comments.len() == 1 {
            output.push(' ');
            output.push_str(&seq.middle_comments[0].text);
            output.push('\n');
        } else if seq.middle_comments.is_empty() {
            output.push('\n');
        } else {
            output.push('\n');
            let indent = indent_str(depth, options);
            for comment in &seq.middle_comments {
                output.push_str(&indent);
                output.push_str(&comment.text);
                output.push('\n');
            }
        }
    } else if !is_top {
        output.push('\n');
    }

    let indent = indent_str(depth, options);
    for (i, item) in seq.items.iter().enumerate() {
        // Leading comments
        for comment in &item.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            let ci = comment_indent(comment, depth, options);
            output.push_str(&ci);
            output.push_str(&comment.text);
            output.push('\n');
        }

        if item.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        if !is_top || i > 0 || has_props {
            output.push_str(&indent);
        }

        match &item.value {
            Node::Mapping(m) if !m.flow && !m.entries.is_empty() => {
                format_sequence_mapping_item(m, item, output, depth, &indent, options);
            }
            Node::Sequence(s) if !s.flow && !s.items.is_empty() => {
                output.push_str("- ");
                format_nested_sequence_inline(s, output, depth + 1, options);
            }
            _ => {
                if is_null_value(&item.value) {
                    output.push('-');
                } else if is_block_scalar_value(&item.value) {
                    output.push_str("- ");
                    format_node(&item.value, output, depth + 1, options, false, true);
                    if let Some(comment) = &item.trailing_comment {
                        output.push_str(&indent);
                        output.push_str(comment);
                        output.push('\n');
                    }
                    continue;
                } else {
                    output.push_str("- ");
                    format_node(&item.value, output, depth + 1, options, false, true);
                }
                if let Some(comment) = &item.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                output.push('\n');
            }
        }
    }

    // Write trailing comments
    for comment in &seq.trailing_comments {
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        let ci = comment_indent(comment, depth, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }
}

/// Format a mapping that's the value of a sequence item (inline first entry after dash).
fn format_sequence_mapping_item(
    m: &crate::ast::MappingNode,
    item: &crate::ast::SequenceItem,
    output: &mut String,
    depth: usize,
    indent: &str,
    options: &YamlFormatOptions,
) {
    output.push_str("- ");
    if let Some(anchor) = &m.anchor {
        output.push('&');
        output.push_str(anchor);
        output.push('\n');
        let entry_indent = indent_str(depth + 1, options);
        output.push_str(&entry_indent);
    }
    let first = &m.entries[0];
    format_node(&first.key, output, depth + 1, options, false, true);
    output.push(':');

    if is_block_scalar_value(&first.value) {
        output.push(' ');
        format_node(&first.value, output, depth + 2, options, false, true);
    } else if is_simple_value(&first.value) {
        output.push(' ');
        format_node(&first.value, output, depth + 1, options, false, true);
        if let Some(comment) = &first.trailing_comment {
            output.push(' ');
            output.push_str(comment);
        }
        output.push('\n');
    } else if is_null_value(&first.value) {
        if let Some(comment) = &first.trailing_comment {
            output.push(' ');
            output.push_str(comment);
        }
        output.push('\n');
    } else {
        if has_node_props(&first.value) {
            output.push(' ');
        }
        if let Some(comment) = &first.trailing_comment {
            output.push(' ');
            output.push_str(comment);
        }
        format_node(&first.value, output, depth + 2, options, false, false);
    }

    // Remaining entries at deeper indent
    let entry_indent = indent_str(depth + 1, options);
    for entry in m.entries.iter().skip(1) {
        for comment in &entry.leading_comments {
            if comment.blank_line_before && !output.ends_with("\n\n") {
                output.push('\n');
            }
            output.push_str(&entry_indent);
            output.push_str(&comment.text);
            output.push('\n');
        }
        if entry.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        output.push_str(&entry_indent);
        format_node(&entry.key, output, depth + 1, options, false, true);
        output.push(':');

        if is_block_scalar_value(&entry.value) {
            output.push(' ');
            format_node(&entry.value, output, depth + 2, options, false, true);
        } else if is_simple_value(&entry.value) {
            output.push(' ');
            format_node(&entry.value, output, depth + 1, options, false, true);
            if let Some(comment) = &entry.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
        } else if is_null_value(&entry.value) {
            if let Some(comment) = &entry.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
        } else {
            if has_node_props(&entry.value) {
                output.push(' ');
            }
            if let Some(comment) = &entry.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            format_node(&entry.value, output, depth + 2, options, false, false);
        }
    }

    // Write trailing comments of the mapping
    for comment in &m.trailing_comments {
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        let ci = comment_indent(comment, depth + 1, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }

    if let Some(comment) = &item.trailing_comment {
        output.push_str(indent);
        output.push_str(comment);
        output.push('\n');
    }
}

/// Format a nested sequence inline: `- item1\n  - item2` etc.
fn format_nested_sequence_inline(
    seq: &SequenceNode,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
) {
    let indent = indent_str(depth, options);
    for (i, item) in seq.items.iter().enumerate() {
        for comment in &item.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            let ci = comment_indent_capped(comment, depth, depth, options);
            output.push_str(&ci);
            output.push_str(&comment.text);
            output.push('\n');
        }

        if item.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        if i > 0 {
            output.push_str(&indent);
        }
        match &item.value {
            Node::Sequence(s) if !s.flow && !s.items.is_empty() => {
                output.push_str("- ");
                format_nested_sequence_inline(s, output, depth + 1, options);
            }
            _ => {
                output.push_str("- ");
                if is_null_value(&item.value) {
                    output.pop();
                }
                format_node(&item.value, output, depth + 1, options, false, true);
                if let Some(comment) = &item.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                output.push('\n');
            }
        }
    }

    // Trailing comments of the nested sequence
    for comment in &seq.trailing_comments {
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        let ci = comment_indent_capped(comment, depth, depth, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }
}
