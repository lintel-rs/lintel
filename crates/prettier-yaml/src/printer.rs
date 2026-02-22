use core::fmt::Write;

use saphyr_parser::ScalarStyle;

use crate::ast::{Node, ScalarNode, SequenceNode, YamlStream};
use crate::print::block::format_block_scalar;
use crate::print::flow::{format_flow_mapping, format_flow_sequence};
use crate::print::mapping_item::format_block_mapping;
use crate::print::misc::{comment_indent, comment_indent_capped, indent_str};
use crate::utilities::{
    has_node_props, is_block_collection, is_block_scalar_value, is_null_value, is_simple_value,
    needs_space_before_colon,
};
use prettier_config::{PrettierConfig, ProseWrap};

pub(crate) fn format_stream(stream: &YamlStream, options: &PrettierConfig) -> String {
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
            if let Some(comment) = &doc.start_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
        }

        // Write body leading comments (between --- and root body)
        for comment in &doc.body_leading_comments {
            output.push_str(&comment.text);
            output.push('\n');
        }

        // Write root node
        if let Some(raw_body) = &doc.raw_body_source {
            // prettier-ignore: output raw body source
            output.push_str(raw_body);
        } else if let Some(root) = &doc.root {
            format_node(root, &mut output, 0, options, true, false);
        }

        // Write root trailing comment (e.g. `!!int 1 - 3 # comment`)
        if let Some(comment) = &doc.root_trailing_comment {
            // Remove trailing newline before appending comment
            if output.ends_with('\n') {
                output.pop();
            }
            output.push(' ');
            output.push_str(comment);
        }

        // Ensure content ends with newline before next doc
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }

        // Write document end marker
        if doc.explicit_end {
            output.push_str("...");
            if let Some(comment) = &doc.end_marker_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
        }

        // Write end comments (comments after root, before next doc)
        for comment in &doc.end_comments {
            if comment.blank_line_before && !output.ends_with("\n\n") {
                output.push('\n');
            }
            output.push_str(&comment.text);
            output.push('\n');
        }

        let _ = i; // suppress unused warning
    }

    // Write trailing stream comments (always at column 0 — stream level)
    for comment in &stream.trailing_comments {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        // Preserve blank line before stream-level comments
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
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
    indent: usize,
    options: &PrettierConfig,
    is_top: bool,
    inline: bool,
) {
    match node {
        Node::Scalar(s) => format_scalar(s, output, indent, options, 0),
        Node::Mapping(m) => {
            if m.flow {
                format_flow_mapping(m, output, indent, options);
            } else {
                format_block_mapping(m, output, indent, options, is_top, inline);
            }
        }
        Node::Sequence(s) => {
            if s.flow {
                format_flow_sequence(s, output, indent, options);
            } else {
                format_block_sequence(s, output, indent, options, is_top, inline);
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
    indent: usize,
    options: &PrettierConfig,
    first_line_prefix: usize,
) {
    // Write tag
    if let Some(tag) = &s.tag {
        output.push_str(tag);
        if !s.is_implicit_null || s.anchor.is_some() {
            output.push(' ');
        }
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
        // Implicit null: don't write anything after tag/anchor
        return;
    }

    // Write middle comments (between tag/anchor and content)
    if !s.middle_comments.is_empty() {
        let indent_s = indent_str(indent);
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
                output.push_str(&indent_s);
                output.push_str(&comment.text);
                output.push('\n');
            }
        }
        // Value starts on new line with indent
        output.push_str(&indent_s);
    }

    match s.style {
        ScalarStyle::Plain => {
            if s.value == "~" {
                // Tilde null - keep as-is
                output.push('~');
            } else {
                format_plain_scalar(s, output, indent, options, first_line_prefix);
            }
        }
        ScalarStyle::SingleQuoted => {
            format_quoted_scalar(
                &s.value,
                s.quoted_source.as_deref(),
                output,
                indent,
                options,
                true,
            );
        }
        ScalarStyle::DoubleQuoted => {
            format_quoted_scalar(
                &s.value,
                s.quoted_source.as_deref(),
                output,
                indent,
                options,
                false,
            );
        }
        ScalarStyle::Literal | ScalarStyle::Folded => {
            format_block_scalar(s, output, indent, options);
        }
    }
}

/// Format a plain scalar value with proseWrap awareness.
fn format_plain_scalar(
    s: &ScalarNode,
    output: &mut String,
    indent: usize,
    options: &PrettierConfig,
    first_line_prefix: usize,
) {
    let indent = indent_str(indent);

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
    indent: usize,
    options: &PrettierConfig,
    was_single_quoted: bool,
) {
    let contains_single = value.contains('\'');
    let contains_double = value.contains('"');
    let contains_newline = value.contains('\n');

    // Check if we have a multi-line raw source (originally multi-line in source)
    let raw_is_multiline = raw_source.is_some_and(|r| r.contains('\n'));

    // Check if raw source uses \ continuation (backslash at end of line folds newline)
    let has_backslash_continuation =
        raw_source.is_some_and(|r| r.split('\n').any(|line| line.trim_end().ends_with('\\')));

    // Enter multiline path when:
    // 1. Value has actual newlines AND raw is multiline (original case, all modes)
    // 2. Raw has \ continuation (always preserve multiline form regardless of prose wrap)
    // 3. Raw is multiline but value folded (no newlines) in preserve mode
    let enter_multiline = raw_is_multiline
        && (contains_newline
            || has_backslash_continuation
            || options.prose_wrap == ProseWrap::Preserve);

    if enter_multiline && let Some(raw) = raw_source {
        // For \ continuation without actual newlines: always use preserve mode
        // since word-wrapping doesn't understand escape continuations
        if has_backslash_continuation && !contains_newline {
            format_multiline_double_quoted_continuation(raw, output, indent);
            return;
        }

        // Use single quotes if singleQuote option is set and content has no single quotes
        let use_single_multiline =
            options.single_quote && !contains_single && !value.contains('\\');
        if use_single_multiline {
            format_multiline_single_quoted(raw, output, indent, options);
        } else {
            format_multiline_double_quoted(raw, output, indent, options);
        }
        return;
    }

    // Determine which quote style to use (only for single-line strings)
    let use_single = if contains_newline {
        // Value has newlines but raw source was single-line (escape sequences)
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
/// Applies prose wrapping: under "always", joins consecutive non-empty lines;
/// under "never", joins all lines per paragraph onto one line.
fn format_multiline_double_quoted(
    raw_source: &str,
    output: &mut String,
    indent: usize,
    options: &PrettierConfig,
) {
    let indent = if indent == 0 {
        String::new()
    } else {
        indent_str(indent)
    };
    let lines: Vec<&str> = raw_source.split('\n').collect();

    // Collect trimmed lines (strip indentation). Preserve trailing whitespace
    // only on the last content line before the closing quote — trailing space
    // there is significant content (e.g. `3rd non-empty "`).
    let mut trimmed_lines: Vec<&str> = Vec::with_capacity(lines.len());
    let last_content_idx = if lines.len() > 1 && lines.last() == Some(&"") {
        lines.len() - 2
    } else {
        lines.len() - 1
    };
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            trimmed_lines.push(line.trim_end());
        } else if i == last_content_idx {
            // Last content line before closing quote: preserve trailing space
            trimmed_lines.push(line.trim_start());
        } else {
            trimmed_lines.push(line.trim());
        }
    }

    match options.prose_wrap {
        ProseWrap::Always => {
            format_multiline_quoted_wrap(&trimmed_lines, output, &indent, options.print_width);
        }
        ProseWrap::Never => {
            format_multiline_quoted_never(&trimmed_lines, output, &indent);
        }
        ProseWrap::Preserve => {
            format_multiline_quoted_preserve(&trimmed_lines, output, &indent);
        }
    }
}

/// Preserve mode: keep original line structure, just re-indent.
fn format_multiline_quoted_preserve(lines: &[&str], output: &mut String, indent: &str) {
    output.push('"');
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            // First line: content right after opening quote (may be empty)
            output.push_str(line);
        } else if i == lines.len() - 1 && line.is_empty() {
            // Last line is just the closing quote position — skip, we add `"` after
        } else {
            output.push('\n');
            if !line.is_empty() {
                output.push_str(indent);
                output.push_str(line);
            }
        }
    }
    // Closing quote: if last raw line was empty (closing quote on its own line),
    // put it on a new indented line
    let last_line = lines.last().copied().unwrap_or("");
    if lines.len() > 1 && last_line.is_empty() {
        output.push('\n');
        output.push_str(indent);
    }
    output.push('"');
}

/// Format a double-quoted string with `\` continuation (backslash-escaped newlines).
/// Always preserves the multiline form regardless of prose wrap mode,
/// since word-wrapping doesn't understand escape continuations.
fn format_multiline_double_quoted_continuation(
    raw_source: &str,
    output: &mut String,
    indent: usize,
) {
    let indent_s = if indent == 0 {
        String::new()
    } else {
        indent_str(indent)
    };
    let lines: Vec<&str> = raw_source.split('\n').collect();

    // Trim lines: first line trim end, middle lines trim both, last content line trim start
    let last_content_idx = if lines.len() > 1 && lines.last() == Some(&"") {
        lines.len() - 2
    } else {
        lines.len() - 1
    };
    let mut trimmed: Vec<&str> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            trimmed.push(line.trim_end());
        } else if i == last_content_idx {
            trimmed.push(line.trim_start());
        } else {
            trimmed.push(line.trim());
        }
    }

    format_multiline_quoted_preserve(&trimmed, output, &indent_s);
}

/// Always mode: join consecutive non-empty lines with space, re-wrap at `print_width`.
fn format_multiline_quoted_wrap(
    lines: &[&str],
    output: &mut String,
    indent: &str,
    print_width: usize,
) {
    // Build content lines (skip last if it's the closing-quote empty line)
    let content_lines = if lines.len() > 1 && lines.last() == Some(&"") {
        &lines[..lines.len() - 1]
    } else {
        lines
    };
    let closing_on_own_line = lines.len() > 1 && lines.last() == Some(&"");

    // Group content lines into segments separated by blank lines.
    let segments = group_quoted_segments(content_lines);

    output.push('"');
    for (si, segment) in segments.iter().enumerate() {
        if segment.is_empty() {
            // Blank line
            output.push('\n');
            continue;
        }
        if si > 0 {
            output.push('\n');
        }
        let is_first_segment = si == 0;
        let first_line = segment[0];

        // Check if first line of first segment is empty (opening quote on its own line)
        if is_first_segment && first_line.is_empty() && segment.len() == 1 {
            // Just an empty first line (opening quote alone)
            continue;
        }
        if is_first_segment && first_line.is_empty() {
            // Opening quote on its own line, content starts on next line
            output.push('\n');
            let rest = &segment[1..];
            output_quoted_wrap_segment(rest, output, indent, print_width, false);
            continue;
        }
        // If any line in this segment ends with `\` (backslash continuation),
        // preserve the segment's line structure instead of word-wrapping it,
        // since wrapping would break the escape continuation syntax.
        let has_continuation = segment.iter().any(|l| l.trim_end().ends_with('\\'));
        if has_continuation {
            output_quoted_preserve_segment(segment, output, indent, is_first_segment);
        } else {
            output_quoted_wrap_segment(segment, output, indent, print_width, is_first_segment);
        }
    }
    if closing_on_own_line {
        output.push('\n');
        output.push_str(indent);
    }
    output.push('"');
}

fn output_quoted_wrap_segment(
    segment: &[&str],
    output: &mut String,
    indent: &str,
    print_width: usize,
    is_first_in_output: bool,
) {
    let joined = segment.join(" ");
    let words: Vec<&str> = joined.split_whitespace().collect();
    if words.is_empty() {
        if !is_first_in_output {
            output.push_str(indent);
        }
        return;
    }

    let mut line_len = if is_first_in_output { 1 } else { indent.len() };
    if !is_first_in_output {
        output.push_str(indent);
    }

    // Check for significant leading whitespace
    let starts_with_space =
        !segment.is_empty() && segment[0].starts_with(' ') && is_first_in_output;
    if starts_with_space {
        output.push(' ');
        line_len += 1;
    }

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
    // Trailing space if original segment ended with one
    if segment.last().is_some_and(|l| l.ends_with(' ')) {
        output.push(' ');
    }
}

/// Preserve a segment's line structure (used for `\` continuation lines).
fn output_quoted_preserve_segment(
    segment: &[&str],
    output: &mut String,
    indent: &str,
    is_first_in_output: bool,
) {
    for (i, line) in segment.iter().enumerate() {
        if i == 0 && is_first_in_output {
            output.push_str(line);
        } else if i == 0 {
            output.push_str(indent);
            output.push_str(line);
        } else {
            output.push('\n');
            if !line.is_empty() {
                output.push_str(indent);
            }
            output.push_str(line);
        }
    }
}

/// Never mode: join all lines per paragraph onto one line.
fn format_multiline_quoted_never(lines: &[&str], output: &mut String, indent: &str) {
    let content_lines = if lines.len() > 1 && lines.last() == Some(&"") {
        &lines[..lines.len() - 1]
    } else {
        lines
    };
    let closing_on_own_line = lines.len() > 1 && lines.last() == Some(&"");

    let segments = group_quoted_segments(content_lines);

    output.push('"');
    for (si, segment) in segments.iter().enumerate() {
        if segment.is_empty() {
            output.push('\n');
            continue;
        }
        if si > 0 {
            output.push('\n');
        }
        let is_first_segment = si == 0;
        let first_line = segment[0];

        if is_first_segment && first_line.is_empty() && segment.len() == 1 {
            continue;
        }
        if is_first_segment && first_line.is_empty() {
            output.push('\n');
            let rest = &segment[1..];
            let joined = rest.join(" ");
            output.push_str(indent);
            output.push_str(joined.trim());
            if rest.last().is_some_and(|l| l.ends_with(' ')) {
                output.push(' ');
            }
            continue;
        }

        let joined = segment.join(" ");
        if !is_first_segment {
            output.push_str(indent);
        }
        // Preserve leading space if present
        if !segment.is_empty() && segment[0].starts_with(' ') && is_first_segment {
            let trimmed = joined.trim_start();
            output.push(' ');
            output.push_str(trimmed);
        } else {
            output.push_str(joined.trim());
        }
        if segment.last().is_some_and(|l| l.ends_with(' ')) {
            output.push(' ');
        }
    }
    if closing_on_own_line {
        output.push('\n');
        output.push_str(indent);
    }
    output.push('"');
}

/// Group lines into segments separated by blank lines.
fn group_quoted_segments<'a>(lines: &[&'a str]) -> Vec<Vec<&'a str>> {
    let mut segments: Vec<Vec<&'a str>> = Vec::new();
    let mut current: Vec<&'a str> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            current.push(line);
            continue;
        }
        if line.is_empty() {
            if !current.is_empty() {
                segments.push(core::mem::take(&mut current));
            }
            segments.push(Vec::new());
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

/// Format a multiline single-quoted string (preserving single-quote style).
fn format_multiline_single_quoted(
    raw_source: &str,
    output: &mut String,
    indent: usize,
    _options: &PrettierConfig,
) {
    let indent = if indent == 0 {
        String::new()
    } else {
        indent_str(indent)
    };
    let lines: Vec<&str> = raw_source.split('\n').collect();
    let mut trimmed_lines: Vec<&str> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            trimmed_lines.push(line.trim_end());
        } else {
            trimmed_lines.push(line.trim());
        }
    }

    // Single-quoted multiline uses the same preserve/wrap/never logic
    // but with single quotes instead of double quotes
    output.push('\'');
    for (i, line) in trimmed_lines.iter().enumerate() {
        if i == 0 {
            output.push_str(line);
        } else if i == trimmed_lines.len() - 1 && line.is_empty() {
            // Closing quote on its own line
        } else {
            output.push('\n');
            if !line.is_empty() {
                output.push_str(&indent);
                output.push_str(line);
            }
        }
    }
    let last_line = trimmed_lines.last().copied().unwrap_or("");
    if trimmed_lines.len() > 1 && last_line.is_empty() {
        output.push('\n');
        output.push_str(&indent);
    }
    output.push('\'');
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
    indent: usize,
    options: &PrettierConfig,
    is_top: bool,
    inline: bool,
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
            let indent_s = indent_str(indent);
            for comment in &seq.middle_comments {
                output.push_str(&indent_s);
                output.push_str(&comment.text);
                output.push('\n');
            }
        }
    } else if !is_top && !inline {
        output.push('\n');
    }

    let indent_s = indent_str(indent);
    // Sequence items add 2 columns ("- " prefix), not tab_width
    let item_content_indent = indent + 2;
    for (i, item) in seq.items.iter().enumerate() {
        // Leading comments
        for comment in &item.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            // First item's leading comments use structural indent (normalize source
            // indentation). Later items preserve source column for comments between items.
            let ci = if i == 0 {
                indent_str(indent)
            } else {
                comment_indent(comment, indent, options)
            };
            output.push_str(&ci);
            output.push_str(&comment.text);
            output.push('\n');
        }

        if item.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        if inline && i == 0 && !has_props {
            // First item inline — no indent, already positioned after `: `
        } else if !is_top || i > 0 || has_props {
            output.push_str(&indent_s);
        }

        match &item.value {
            Node::Mapping(m) if !m.flow && !m.entries.is_empty() => {
                format_sequence_mapping_item(m, item, output, indent, &indent_s, options);
            }
            Node::Sequence(s) if !s.flow && !s.items.is_empty() => {
                if s.tag.is_some() || s.anchor.is_some() {
                    // Tagged/anchored sequence: output props on `- ` line, items below
                    output.push_str("- ");
                    format_node(
                        &item.value,
                        output,
                        item_content_indent,
                        options,
                        false,
                        true,
                    );
                } else {
                    output.push_str("- ");
                    format_nested_sequence_inline(s, output, item_content_indent, options);
                }
            }
            _ => {
                if let Some(ind_comment) = &item.indicator_comment {
                    // Comment on the `- ` indicator line with value on next line:
                    // output `- #comment\n<indent>value`
                    output.push_str("- ");
                    output.push_str(ind_comment);
                    output.push('\n');
                    let ci = indent_str(item_content_indent);
                    output.push_str(&ci);
                    format_node(
                        &item.value,
                        output,
                        item_content_indent,
                        options,
                        false,
                        true,
                    );
                } else if is_null_value(&item.value) && !has_node_props(&item.value) {
                    // Prettier uses "- " prefix even for null items when there's
                    // a trailing comment, so the comment starts at column indent+2
                    if item.trailing_comment.is_some() {
                        output.push_str("- ");
                    } else {
                        output.push('-');
                    }
                } else if is_null_value(&item.value) && has_node_props(&item.value) {
                    // Null value with tag/anchor: `- !!str` or `- &anchor`
                    output.push_str("- ");
                    format_node(
                        &item.value,
                        output,
                        item_content_indent,
                        options,
                        false,
                        true,
                    );
                } else if is_block_scalar_value(&item.value) {
                    output.push_str("- ");
                    format_node(
                        &item.value,
                        output,
                        item_content_indent,
                        options,
                        false,
                        true,
                    );
                    if let Some(comment) = &item.trailing_comment {
                        output.push_str(&indent_s);
                        output.push_str(comment);
                        output.push('\n');
                    }
                    continue;
                } else {
                    output.push_str("- ");
                    format_node(
                        &item.value,
                        output,
                        item_content_indent,
                        options,
                        false,
                        true,
                    );
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
        let ci = comment_indent(comment, indent, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }
}

/// Format a mapping that's the value of a sequence item (inline first entry after dash).
/// `seq_indent` is the column position of the sequence's `- ` prefix.
#[allow(clippy::too_many_lines)]
fn format_sequence_mapping_item(
    m: &crate::ast::MappingNode,
    item: &crate::ast::SequenceItem,
    output: &mut String,
    seq_indent: usize,
    seq_indent_s: &str,
    options: &PrettierConfig,
) {
    let tw = options.tab_width;
    // Entry indent is sequence indent + 2 (for "- " prefix)
    let entry_indent = seq_indent + 2;
    let entry_indent_s = indent_str(entry_indent);
    // Nested values are one tab_width deeper than the entry
    let value_indent = entry_indent + tw;

    output.push_str("- ");

    // Handle indicator comment: `- # comment` on its own line, entries below
    let _has_indicator_comment = if let Some(ind_comment) = &item.indicator_comment {
        output.push_str(ind_comment);
        output.push('\n');
        output.push_str(&entry_indent_s);
        true
    } else {
        false
    };
    let has_props = m.tag.is_some() || m.anchor.is_some();
    if let Some(tag) = &m.tag {
        output.push_str(tag);
    }
    if let Some(anchor) = &m.anchor {
        if m.tag.is_some() {
            output.push(' ');
        }
        output.push('&');
        output.push_str(anchor);
    }
    if has_props {
        // Middle comments between props and entries
        if m.middle_comments.len() == 1 {
            output.push(' ');
            output.push_str(&m.middle_comments[0].text);
        } else if !m.middle_comments.is_empty() {
            for comment in &m.middle_comments {
                output.push('\n');
                output.push_str(&entry_indent_s);
                output.push_str(&comment.text);
            }
        } else if let Some(comment) = &item.trailing_comment {
            // Trailing comment on the sequence item goes on the tag line
            output.push(' ');
            output.push_str(comment);
        }
        output.push('\n');
        output.push_str(&entry_indent_s);
    }
    let first = &m.entries[0];

    // Check if the first entry needs explicit key format (? key : value)
    let first_needs_explicit = first.is_explicit_key
        && (!is_null_value(&first.value)
            || is_block_collection(&first.key)
            || is_block_scalar_value(&first.key)
            || !first.between_comments.is_empty())
        || first.question_mark_comment.is_some();

    if first_needs_explicit {
        // Compact explicit key: `- ? key\n  : value`
        // Use entry_indent so `:` aligns with `?` at seq_indent+2
        crate::print::mapping_item::format_explicit_key_entry(first, output, entry_indent, options);
    } else {
        format_node(&first.key, output, entry_indent, options, false, true);
        if needs_space_before_colon(&first.key) {
            output.push(' ');
        }
        output.push(':');

        if is_block_scalar_value(&first.value) {
            output.push(' ');
            format_node(&first.value, output, value_indent, options, false, true);
        } else if is_simple_value(&first.value) {
            output.push(' ');
            format_node(&first.value, output, entry_indent, options, false, true);
            if let Some(comment) = &first.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
        } else if is_null_value(&first.value) {
            if has_node_props(&first.value) {
                output.push(' ');
                format_node(&first.value, output, entry_indent, options, false, true);
            }
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
            format_node(&first.value, output, value_indent, options, false, false);
        }
    }

    // Remaining entries at entry indent (seq_indent + 2)
    for entry in m.entries.iter().skip(1) {
        for comment in &entry.leading_comments {
            if comment.blank_line_before && !output.ends_with("\n\n") {
                output.push('\n');
            }
            output.push_str(&entry_indent_s);
            output.push_str(&comment.text);
            output.push('\n');
        }
        if entry.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }

        // Check if this entry needs explicit key format
        let entry_needs_explicit = entry.is_explicit_key
            && (!is_null_value(&entry.value)
                || is_block_collection(&entry.key)
                || is_block_scalar_value(&entry.key)
                || !entry.between_comments.is_empty())
            || entry.question_mark_comment.is_some();

        if entry_needs_explicit {
            output.push_str(&entry_indent_s);
            crate::print::mapping_item::format_explicit_key_entry(
                entry,
                output,
                entry_indent,
                options,
            );
            continue;
        }

        output.push_str(&entry_indent_s);
        format_node(&entry.key, output, entry_indent, options, false, true);
        if needs_space_before_colon(&entry.key) {
            output.push(' ');
        }
        output.push(':');

        if is_block_scalar_value(&entry.value) {
            output.push(' ');
            format_node(&entry.value, output, value_indent, options, false, true);
        } else if is_simple_value(&entry.value) {
            output.push(' ');
            format_node(&entry.value, output, entry_indent, options, false, true);
            if let Some(comment) = &entry.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
        } else if is_null_value(&entry.value) {
            if has_node_props(&entry.value) {
                output.push(' ');
                format_node(&entry.value, output, entry_indent, options, false, true);
            }
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
            format_node(&entry.value, output, value_indent, options, false, false);
        }
    }

    // Write trailing comments of the mapping
    for comment in &m.trailing_comments {
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        let ci = comment_indent(comment, entry_indent, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }

    if let Some(comment) = &item.trailing_comment {
        output.push_str(seq_indent_s);
        output.push_str(comment);
        output.push('\n');
    }
}

/// Format a nested sequence inline: `- item1\n  - item2` etc.
/// `indent` is the column position where `- ` indicators are placed.
fn format_nested_sequence_inline(
    seq: &SequenceNode,
    output: &mut String,
    indent: usize,
    options: &PrettierConfig,
) {
    let indent_s = indent_str(indent);
    let item_content_indent = indent + 2;
    for (i, item) in seq.items.iter().enumerate() {
        for comment in &item.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            let ci = comment_indent_capped(comment, indent, indent, options);
            output.push_str(&ci);
            output.push_str(&comment.text);
            output.push('\n');
        }

        if item.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        if i > 0 {
            output.push_str(&indent_s);
        }
        match &item.value {
            Node::Sequence(s) if !s.flow && !s.items.is_empty() => {
                output.push_str("- ");
                format_nested_sequence_inline(s, output, item_content_indent, options);
            }
            _ => {
                output.push_str("- ");
                if is_null_value(&item.value) {
                    output.pop();
                }
                format_node(
                    &item.value,
                    output,
                    item_content_indent,
                    options,
                    false,
                    true,
                );
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
        let ci = comment_indent_capped(comment, indent, indent, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }
}
