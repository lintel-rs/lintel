use crate::YamlFormatOptions;
use crate::ast::{MappingEntry, MappingNode, Node};
use crate::print::misc::{comment_indent, comment_indent_capped, indent_str, renders_multiline};
use crate::printer::{format_node, format_scalar};
use crate::utilities::{
    has_node_props, is_block_collection, is_block_scalar_value, is_null_value, is_simple_value,
    needs_space_before_colon,
};
use saphyr_parser::ScalarStyle;

/// Check if a key node is multiline (plain scalar spanning multiple source lines).
fn is_multiline_key(node: &Node) -> bool {
    match node {
        Node::Scalar(s) => {
            if s.style == ScalarStyle::Plain {
                // A plain scalar is multiline if it has source lines
                s.plain_source_lines.is_some()
            } else {
                false
            }
        }
        _ => false,
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn format_block_mapping(
    mapping: &MappingNode,
    output: &mut String,
    indent: usize,
    options: &YamlFormatOptions,
    is_top: bool,
    inline: bool,
) {
    if mapping.entries.is_empty() {
        output.push_str("{}");
        return;
    }

    let tw = options.tab_width;

    // Check if this mapping is a !!set (preserve ? key format for null values)
    let is_set = mapping.tag.as_deref().is_some_and(|t| t.contains("set"));

    // Write tag and anchor
    let has_props = mapping.tag.is_some() || mapping.anchor.is_some();
    if has_props {
        if let Some(tag) = &mapping.tag {
            output.push_str(tag);
        }
        if let Some(anchor) = &mapping.anchor {
            if mapping.tag.is_some() {
                output.push(' ');
            }
            output.push('&');
            output.push_str(anchor);
        }
        // Middle comments
        if mapping.middle_comments.len() == 1 {
            // Single comment: on same line as props
            output.push(' ');
            output.push_str(&mapping.middle_comments[0].text);
            output.push('\n');
        } else if mapping.middle_comments.is_empty() {
            output.push('\n');
        } else {
            // Multiple: props on own line, then each comment
            output.push('\n');
            for comment in &mapping.middle_comments {
                output.push_str(&comment.text);
                output.push('\n');
            }
        }
    } else if !is_top && !inline {
        output.push('\n');
    }

    let indent_s = indent_str(indent);
    let value_indent = indent + tw;
    for (i, entry) in mapping.entries.iter().enumerate() {
        // Leading comments (each may have its own blank_line_before)
        for (ci_idx, comment) in entry.leading_comments.iter().enumerate() {
            // Emit blank line if flagged, unless it's the very first comment of
            // the very first entry (which would produce a spurious blank line at
            // the start of the mapping).
            let allow_blank = i > 0 || ci_idx > 0;
            if comment.blank_line_before && allow_blank && !output.ends_with("\n\n") {
                output.push('\n');
            }
            // For the first entry (i==0), leading comments should use structural
            // indent (source may have arbitrary indentation). For subsequent entries,
            // comments between entries preserve their source indentation.
            let ci = if i == 0 {
                indent_str(indent)
            } else {
                comment_indent(comment, indent, options)
            };
            output.push_str(&ci);
            output.push_str(&comment.text);
            output.push('\n');
        }

        // Blank line between last leading comment and the entry key.
        // For i > 0: always emit if flagged.
        // For i == 0: only emit if there are leading comments (so blank line
        // goes between the comment group and the key, not at document start).
        let allow_entry_blank = i > 0 || !entry.leading_comments.is_empty();
        if entry.blank_line_before && allow_entry_blank && !output.ends_with("\n\n") {
            output.push('\n');
        }

        if !is_top || i > 0 || has_props {
            output.push_str(&indent_s);
        }

        // prettier-ignore: output raw source instead of formatting
        if let Some(raw) = &entry.raw_source {
            for (j, line) in raw.lines().enumerate() {
                if j > 0 {
                    output.push('\n');
                    output.push_str(&indent_s);
                }
                output.push_str(line);
            }
            output.push('\n');
            continue;
        }

        // Determine if we need explicit key format (? key\n: value)
        let needs_explicit_key = (entry.is_explicit_key
            && is_null_value(&entry.value)
            && !has_node_props(&entry.value)
            && (is_set
                || entry.trailing_comment.is_some()
                || entry.key_trailing_comment.is_some()
                || is_multiline_key(&entry.key)))
            || (entry.is_explicit_key
                && (is_block_collection(&entry.key) || is_block_scalar_value(&entry.key)))
            || (entry.is_explicit_key && !entry.between_comments.is_empty())
            || entry.question_mark_comment.is_some()
            || (entry.is_explicit_key && !entry.leading_comments.is_empty())
            || renders_multiline(&entry.key, indent + 2, options);

        // Check if key has trailing comments that force value to the next line.
        // Exception: when the value is a block scalar, the key_trailing_comment
        // is part of the block scalar header (already in block_source), not a
        // standalone comment that should move the value.
        let is_block_val = is_block_scalar_value(&entry.value);
        let has_key_comments = (!is_block_val && entry.key_trailing_comment.is_some())
            || !entry.between_comments.is_empty()
            || entry.colon_comment.is_some();

        if needs_explicit_key && is_null_value(&entry.value) && !has_node_props(&entry.value) {
            // Explicit key with null value: ? key
            output.push_str("? ");
            // Key content after "? " is at indent + 2
            format_node(&entry.key, output, indent + 2, options, false, true);
            if let Some(comment) = &entry.key_trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            if let Some(comment) = &entry.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
        } else if needs_explicit_key {
            // Explicit key format for complex/multiline keys
            format_explicit_key_entry(entry, output, indent, options);
        } else {
            // Write key
            format_node(&entry.key, output, indent, options, false, true);
            if needs_space_before_colon(&entry.key) {
                output.push(' ');
            }
            output.push(':');

            // If key has trailing comments, output them and put value on next line
            if has_key_comments {
                format_key_comments_and_value(entry, output, indent, options);
            } else if is_block_scalar_value(&entry.value) {
                // Block scalar: inline after colon, body on next lines
                output.push(' ');
                format_node(&entry.value, output, value_indent, options, false, true);
                // Block scalar already outputs trailing newline
            } else if is_simple_value(&entry.value) {
                // For flow collection keys, check if key + ": " + value
                // exceeds print width. If so, break value to the next line.
                let key_is_flow_collection = matches!(
                    &entry.key,
                    Node::Sequence(s) if s.flow
                ) || matches!(
                    &entry.key,
                    Node::Mapping(m) if m.flow
                );
                let line_start = output.rfind('\n').map_or(0, |i| i + 1);
                let key_prefix_len = output.len() - line_start + 2; // +2 for ": "
                let value_len = match &entry.value {
                    Node::Scalar(s) => s.value.len(),
                    _ => 0,
                };
                if key_is_flow_collection
                    && key_prefix_len + value_len > options.print_width
                    && value_len > 0
                {
                    // Value goes to next line, indented
                    output.push('\n');
                    let val_indent_s = indent_str(indent + options.tab_width);
                    output.push_str(&val_indent_s);
                    format_node(
                        &entry.value,
                        output,
                        indent + options.tab_width,
                        options,
                        false,
                        true,
                    );
                    if let Some(comment) = &entry.trailing_comment {
                        output.push(' ');
                        output.push_str(comment);
                    }
                    output.push('\n');
                } else {
                    output.push(' ');
                    format_simple_value(entry, output, indent, options);
                }
            } else if is_null_value(&entry.value) {
                // Null value - but may still have anchor/tag
                if has_node_props(&entry.value) {
                    output.push(' ');
                    format_node(&entry.value, output, indent, options, false, true);
                }
                // Trailing comment for null value
                if let Some(comment) = &entry.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                output.push('\n');
            } else {
                // Complex value on next line
                let value_has_props = has_node_props(&entry.value);
                if value_has_props {
                    output.push(' ');
                }
                if let Some(comment) = &entry.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                format_node(
                    &entry.value,
                    output,
                    value_indent,
                    options,
                    false,
                    value_has_props,
                );
            }
        }
    }

    // Write trailing comments (comments after last entry in the mapping)
    for comment in &mapping.trailing_comments {
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        let ci = comment_indent_capped(comment, indent, indent + tw, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }
}

/// Format a simple scalar value inline after "key: " (no key comments).
fn format_simple_value(
    entry: &MappingEntry,
    output: &mut String,
    indent: usize,
    options: &YamlFormatOptions,
) {
    let tw = options.tab_width;
    let value_indent = indent + tw;
    // For plain scalars: pass the first-line prefix length for wrapping
    if let Node::Scalar(ref scalar) = entry.value {
        if scalar.style == ScalarStyle::Plain && scalar.value != "~" && !scalar.value.is_empty() {
            let line_start = output.rfind('\n').map_or(0, |i| i + 1);
            let first_line_prefix = output.len() - line_start;

            // Check if we need to break value to the next line.
            let should_break = {
                use crate::ProseWrap;
                let has_para_break = scalar.value.contains('\n');
                match options.prose_wrap {
                    ProseWrap::Always if !has_para_break => {
                        let len = scalar.value.trim().len();
                        let can_break = scalar.value.contains(' ');
                        first_line_prefix + len > options.print_width && can_break
                    }
                    ProseWrap::Never if has_para_break => {
                        let first_para = scalar.value.split('\n').next().unwrap_or("");
                        let len = first_para.trim().len();
                        first_line_prefix + len > options.print_width
                    }
                    _ => false,
                }
            };
            if should_break {
                // Break: remove the trailing space, add newline + indent
                output.pop(); // remove the ' ' we just pushed
                output.push('\n');
                let val_indent_s = indent_str(value_indent);
                output.push_str(&val_indent_s);
                format_scalar(scalar, output, value_indent, options, val_indent_s.len());
                if let Some(comment) = &entry.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                output.push('\n');
                return;
            }
            format_scalar(scalar, output, value_indent, options, first_line_prefix);
        } else {
            format_scalar(scalar, output, value_indent, options, 0);
        }
    } else {
        format_node(&entry.value, output, value_indent, options, false, true);
    }
    // Trailing comment
    if let Some(comment) = &entry.trailing_comment {
        output.push(' ');
        output.push_str(comment);
    }
    output.push('\n');
}

/// Format a mapping entry where the key has trailing/between comments.
/// Output: `key: # comment\n  # between\n  value\n`
fn format_key_comments_and_value(
    entry: &MappingEntry,
    output: &mut String,
    indent: usize,
    options: &YamlFormatOptions,
) {
    let tw = options.tab_width;
    let value_indent = indent + tw;

    // Key trailing comment on the same line as key:
    if let Some(comment) = &entry.key_trailing_comment {
        output.push(' ');
        output.push_str(comment);
    }
    output.push('\n');

    // Between comments (standalone comments between key and value)
    let val_indent_s = indent_str(value_indent);
    for comment in &entry.between_comments {
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        output.push_str(&val_indent_s);
        output.push_str(&comment.text);
        output.push('\n');
    }
    // Colon-line comment (from explicit key `: # comment` collapsed to implicit)
    if let Some(comment) = &entry.colon_comment {
        output.push_str(&val_indent_s);
        output.push_str(&comment.text);
        output.push('\n');
    }

    // Blank line between last between_comment and the value
    if entry.blank_line_before_value && !output.ends_with("\n\n") {
        output.push('\n');
    }

    // prettier-ignore in between_comments: preserve value raw source
    let value_prettier_ignore = entry
        .between_comments
        .iter()
        .any(|c| c.text.trim() == "# prettier-ignore");
    if value_prettier_ignore {
        let raw = match &entry.value {
            Node::Mapping(m) if m.flow_source.is_some() => m.flow_source.as_ref(),
            Node::Sequence(s) if s.flow_source.is_some() => s.flow_source.as_ref(),
            _ => None,
        };
        if let Some(raw_src) = raw {
            // Output first line at value indent, then preserve original
            // indentation for subsequent lines (flow_source retains source whitespace)
            output.push_str(&val_indent_s);
            for (j, line) in raw_src.lines().enumerate() {
                if j > 0 {
                    output.push('\n');
                }
                output.push_str(line);
            }
            output.push('\n');
            return;
        }
    }

    // Value on the next line, indented
    if is_null_value(&entry.value) && !has_node_props(&entry.value) {
        // Null value with comments — nothing more to output
        if let Some(comment) = &entry.trailing_comment {
            output.push_str(&val_indent_s);
            output.push_str(comment);
            output.push('\n');
        }
    } else if is_simple_value(&entry.value) || is_block_scalar_value(&entry.value) {
        output.push_str(&val_indent_s);
        format_node(&entry.value, output, value_indent, options, false, true);
        if let Some(comment) = &entry.trailing_comment {
            output.push(' ');
            output.push_str(comment);
        }
        // Block scalars add their own newline; simple values need one
        if !is_block_scalar_value(&entry.value) {
            output.push('\n');
        }
    } else {
        // Complex value (mapping/sequence) on next line, indented.
        // Pass inline=true since we're already on a new line (after
        // between_comments or key_trailing_comment).
        let is_block_seq = matches!(&entry.value, Node::Sequence(s) if !s.flow);
        if is_block_seq {
            // Block sequences with inline=true skip the first item's indent,
            // so push it explicitly here.
            output.push_str(&val_indent_s);
        }
        // Mappings with inline=true still indent entries internally (via !is_top).
        format_node(&entry.value, output, value_indent, options, false, true);
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn format_explicit_key_entry(
    entry: &MappingEntry,
    output: &mut String,
    indent: usize,
    options: &YamlFormatOptions,
) {
    let tw = options.tab_width;
    // Content after "? " is at indent + 2 (the width of "? ")
    let key_indent = indent + 2;
    let value_indent = indent + tw;

    // Output: ? [question_mark_comment]\n  key\n  [between_comments]\n: value
    output.push('?');

    let key_indent_s = indent_str(key_indent);

    // Handle question_mark_comment: inline comment on the `?` line
    // e.g. `? # comment\n  key`
    if let Some(comment) = &entry.question_mark_comment {
        output.push(' ');
        output.push_str(comment);
        output.push('\n');
        output.push_str(&key_indent_s);
    } else {
        output.push(' ');
    }

    // Key — for single-entry block mappings with simple entries, format
    // inline as `key: value` (compact block mapping syntax like `? earth: blue`)
    let is_compact_key_map = matches!(&entry.key, Node::Mapping(m) if !m.flow
        && m.entries.len() == 1
        && m.anchor.is_none() && m.tag.is_none()
        && is_simple_value(&m.entries[0].key)
        && (is_simple_value(&m.entries[0].value) || is_null_value(&m.entries[0].value)));
    if is_compact_key_map {
        // Format the single entry inline: `key: value`
        if let Node::Mapping(m) = &entry.key {
            let e = &m.entries[0];
            format_node(&e.key, output, key_indent, options, false, true);
            if needs_space_before_colon(&e.key) {
                output.push(' ');
            }
            output.push(':');
            if !is_null_value(&e.value) {
                output.push(' ');
                format_node(&e.value, output, key_indent, options, false, true);
            }
        }
    } else {
        format_node(&entry.key, output, key_indent, options, false, true);
    }
    if let Some(comment) = &entry.key_trailing_comment {
        output.push(' ');
        output.push_str(comment);
    }
    // Block scalar keys already end with a newline; don't add another
    if !output.ends_with('\n') {
        output.push('\n');
    }

    // Between comments (standalone comments between key and value)
    // e.g. `? key\n# comment\n: value`
    for comment in &entry.between_comments {
        let ci = comment_indent(comment, indent, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }

    let indent_s = indent_str(indent);
    output.push_str(&indent_s);
    output.push(':');

    // Colon-line comment (e.g. `: # comment`)
    if let Some(comment) = &entry.colon_comment {
        output.push(' ');
        output.push_str(&comment.text);
    }

    // When the colon line has a comment (`: # comment`), the value must go
    // on a new line — never inline after the colon.
    let colon_has_comment = entry.colon_comment.is_some();

    if is_simple_value(&entry.value) && !colon_has_comment {
        output.push(' ');
        format_node(&entry.value, output, value_indent, options, false, true);
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
    } else if is_block_scalar_value(&entry.value) && !colon_has_comment {
        // Block scalar value inline after `:`
        output.push(' ');
        format_node(&entry.value, output, value_indent, options, false, true);
    } else if colon_has_comment {
        // Colon has a comment — value goes on the next line
        format_node(&entry.value, output, value_indent, options, false, false);
    } else {
        // Compact form for sequences: `: - item`
        // For single-entry block mappings: `: key: value` (compact block mapping)
        // For other complex values: newline then indent
        let is_block_seq = matches!(&entry.value, Node::Sequence(s) if !s.flow);
        let is_compact_block_map = matches!(&entry.value, Node::Mapping(m) if !m.flow && m.entries.len() == 1
            && m.anchor.is_none() && m.tag.is_none()
            && is_simple_value(&m.entries[0].key)
            && (is_simple_value(&m.entries[0].value) || is_null_value(&m.entries[0].value)));
        if is_compact_block_map && !has_node_props(&entry.value) {
            // Format single-entry block mapping inline: `: key: value`
            output.push(' ');
            if let Node::Mapping(m) = &entry.value {
                let e = &m.entries[0];
                format_node(&e.key, output, value_indent, options, false, true);
                if needs_space_before_colon(&e.key) {
                    output.push(' ');
                }
                output.push(':');
                if !is_null_value(&e.value) {
                    output.push(' ');
                    format_node(&e.value, output, value_indent, options, false, true);
                }
            }
            if let Some(comment) = &entry.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
        } else if is_block_seq && !has_node_props(&entry.value) {
            output.push(' ');
            format_node(&entry.value, output, value_indent, options, false, true);
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
}
