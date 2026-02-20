use crate::YamlFormatOptions;
use crate::ast::{MappingEntry, MappingNode, Node};
use crate::print::misc::{comment_indent, comment_indent_capped, indent_str};
use crate::printer::{format_node, format_scalar};
use crate::utilities::{
    has_node_props, is_block_scalar_value, is_collection, is_null_value, is_simple_value,
};
use saphyr_parser::ScalarStyle;

#[allow(clippy::too_many_lines)]
pub(crate) fn format_block_mapping(
    mapping: &MappingNode,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
    is_top: bool,
    inline: bool,
) {
    if mapping.entries.is_empty() {
        output.push_str("{}");
        return;
    }

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

    let indent = indent_str(depth, options);
    for (i, entry) in mapping.entries.iter().enumerate() {
        // Leading comments (each may have its own blank_line_before)
        for comment in &entry.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            let ci = comment_indent(comment, depth, options);
            output.push_str(&ci);
            output.push_str(&comment.text);
            output.push('\n');
        }

        // Blank line between last leading comment and the entry key
        if entry.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        if !is_top || i > 0 || has_props {
            output.push_str(&indent);
        }

        if entry.is_explicit_key
            && is_null_value(&entry.value)
            && !has_node_props(&entry.value)
            && is_set
        {
            // Set-style explicit key with null value: ? key
            output.push_str("? ");
            format_node(&entry.key, output, depth + 1, options, false, true);
            if let Some(comment) = &entry.key_trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            if let Some(comment) = &entry.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
        } else if entry.is_explicit_key
            && (is_collection(&entry.key) || is_block_scalar_value(&entry.key))
        {
            // Keep explicit key format for complex keys (mappings, sequences, block scalars)
            format_explicit_key_entry(entry, output, depth, options);
        } else {
            // Write key
            format_node(&entry.key, output, depth, options, false, true);
            output.push(':');

            // Write value
            if is_block_scalar_value(&entry.value) {
                // Block scalar: inline after colon, body on next lines
                output.push(' ');
                format_node(&entry.value, output, depth + 1, options, false, true);
                // Block scalar already outputs trailing newline
            } else if is_simple_value(&entry.value) {
                output.push(' ');
                // For plain scalars: pass the first-line prefix length for wrapping
                if let Node::Scalar(ref scalar) = entry.value {
                    if scalar.style == ScalarStyle::Plain
                        && scalar.value != "~"
                        && !scalar.value.is_empty()
                    {
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
                            let val_indent = indent_str(depth + 1, options);
                            output.push_str(&val_indent);
                            format_scalar(scalar, output, depth + 1, options, val_indent.len());
                            if let Some(comment) = &entry.trailing_comment {
                                output.push(' ');
                                output.push_str(comment);
                            }
                            output.push('\n');
                            continue;
                        }
                        format_scalar(scalar, output, depth + 1, options, first_line_prefix);
                    } else {
                        format_scalar(scalar, output, depth + 1, options, 0);
                    }
                } else {
                    format_node(&entry.value, output, depth, options, false, true);
                }
                // Trailing comment
                if let Some(comment) = &entry.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                output.push('\n');
            } else if is_null_value(&entry.value) {
                // Null value - but may still have anchor/tag
                if has_node_props(&entry.value) {
                    output.push(' ');
                    format_node(&entry.value, output, depth, options, false, true);
                }
                // Trailing comment for null value
                if let Some(comment) = &entry.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                output.push('\n');
            } else {
                // Complex value on next line
                if has_node_props(&entry.value) {
                    output.push(' ');
                }
                if !has_node_props(&entry.value)
                    && let Some(comment) = &entry.key_trailing_comment
                {
                    output.push(' ');
                    output.push_str(comment);
                }
                if let Some(comment) = &entry.trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                // Standalone between comments (between key and value, on their own lines)
                if !entry.between_comments.is_empty() {
                    for comment in &entry.between_comments {
                        output.push('\n');
                        let ci = comment_indent(comment, depth + 1, options);
                        output.push_str(&ci);
                        output.push_str(&comment.text);
                    }
                }
                format_node(&entry.value, output, depth + 1, options, false, false);
            }
        }
    }

    // Write trailing comments (comments after last entry in the mapping)
    for comment in &mapping.trailing_comments {
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        let ci = comment_indent_capped(comment, depth, depth + 1, options);
        output.push_str(&ci);
        output.push_str(&comment.text);
        output.push('\n');
    }
}

pub(crate) fn format_explicit_key_entry(
    entry: &MappingEntry,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
) {
    // ? key\n: value
    output.push_str("? ");
    format_node(&entry.key, output, depth + 1, options, false, true);
    output.push('\n');

    let indent = indent_str(depth, options);
    output.push_str(&indent);
    output.push(':');

    if is_simple_value(&entry.value) {
        output.push(' ');
        format_node(&entry.value, output, depth, options, false, true);
        output.push('\n');
    } else if is_null_value(&entry.value) {
        output.push('\n');
    } else {
        output.push('\n');
        format_node(&entry.value, output, depth + 1, options, false, false);
    }
}
