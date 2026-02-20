use crate::YamlFormatOptions;
use crate::ast::{MappingNode, Node, SequenceNode};
use crate::print::misc::{indent_str, renders_multiline};
use crate::printer::format_node;
use crate::utilities::{is_collection, is_null_value};

pub(crate) fn format_flow_mapping(
    mapping: &MappingNode,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
) {
    let has_props = mapping.tag.is_some() || mapping.anchor.is_some();
    let has_middle_comments = !mapping.middle_comments.is_empty();

    // Write tag and anchor
    if let Some(tag) = &mapping.tag {
        output.push_str(tag);
        output.push(' ');
    }
    if let Some(anchor) = &mapping.anchor {
        output.push('&');
        output.push_str(anchor);
        output.push(' ');
    }

    // Middle comments go between props and content
    if has_middle_comments {
        if mapping.middle_comments.len() == 1 && has_props {
            // Single middle comment: on same line as props
            output.push_str(&mapping.middle_comments[0].text);
            output.push('\n');
        } else {
            // Multiple: props on own line, then each comment
            if has_props {
                // Trim trailing space added after tag/anchor
                while output.ends_with(' ') {
                    output.pop();
                }
                output.push('\n');
            }
            for comment in &mapping.middle_comments {
                output.push_str(&comment.text);
                output.push('\n');
            }
        }
    }

    if mapping.entries.is_empty() {
        output.push_str("{}");
        return;
    }

    // If there are comments or trailing comments, force broken format
    let has_entry_comments = mapping.entries.iter().any(|e| {
        e.trailing_comment.is_some()
            || e.key_trailing_comment.is_some()
            || !e.leading_comments.is_empty()
            || !e.between_comments.is_empty()
    });

    if has_middle_comments || has_entry_comments || !mapping.trailing_comments.is_empty() {
        format_flow_mapping_broken(mapping, output, depth, options);
        return;
    }

    // Try flat format first
    let flat = format_flow_mapping_flat(mapping, depth, options);

    // If flat result contains newlines (nested broken collections), go to broken
    let current_col = depth * options.tab_width;
    if !flat.contains('\n') && current_col + flat.len() <= options.print_width {
        output.push_str(&flat);
    } else {
        // Break to multi-line
        format_flow_mapping_broken(mapping, output, depth, options);
    }
}

fn format_flow_mapping_flat(
    mapping: &MappingNode,
    depth: usize,
    options: &YamlFormatOptions,
) -> String {
    let mut parts = Vec::new();
    for entry in &mapping.entries {
        let mut part = String::new();
        // In flow mappings, prettier drops `?` for explicit-key entries

        // Handle null key with null value (`: ` entry)
        let key_is_null = is_null_value(&entry.key);
        if key_is_null && is_null_value(&entry.value) {
            if options.bracket_spacing {
                part.push(':');
            } else {
                part.push_str(": ");
            }
            parts.push(part);
            continue;
        }
        format_node(&entry.key, &mut part, depth, options, false, true);
        if is_null_value(&entry.value) {
            // Null value: just the key, no ": ~"
        } else {
            // Alias keys need space before colon (e.g. `*foo : bar`)
            if matches!(&entry.key, Node::Alias(_)) {
                part.push_str(" : ");
            } else {
                part.push_str(": ");
            }
            format_node(&entry.value, &mut part, depth, options, false, true);
        }
        parts.push(part);
    }

    if options.bracket_spacing {
        format!("{{ {} }}", parts.join(", "))
    } else {
        format!("{{{}}}", parts.join(", "))
    }
}

fn format_flow_mapping_broken(
    mapping: &MappingNode,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
) {
    let inner_indent = indent_str(depth + 1, options);
    let outer_indent = indent_str(depth, options);

    output.push_str("{\n");
    for (i, entry) in mapping.entries.iter().enumerate() {
        // Leading comments (each may have its own blank_line_before)
        for comment in &entry.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            output.push_str(&inner_indent);
            output.push_str(&comment.text);
            output.push('\n');
        }

        // Blank line between last leading comment and the entry key
        if entry.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        let key_is_complex = is_collection(&entry.key);
        let key_is_null = is_null_value(&entry.key);
        let value_is_null = is_null_value(&entry.value);
        let value_is_multiline =
            !value_is_null && renders_multiline(&entry.value, depth + 2, options);

        let has_between =
            !entry.between_comments.is_empty() || entry.key_trailing_comment.is_some();

        if key_is_null && value_is_null {
            // null key + null value = `: `
            output.push_str(&inner_indent);
            output.push_str(": ");
        } else if (key_is_complex || has_between) && !value_is_null {
            // Complex key or comments between key-value: use ? key \n [comments] \n : value
            output.push_str(&inner_indent);
            output.push_str("? ");
            format_node(&entry.key, output, depth + 2, options, false, true);
            if let Some(comment) = &entry.key_trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            for comment in &entry.between_comments {
                output.push('\n');
                output.push_str(&inner_indent);
                output.push_str(&comment.text);
            }
            output.push('\n');
            output.push_str(&inner_indent);
            output.push_str(": ");
            format_node(&entry.value, output, depth + 2, options, false, true);
        } else if key_is_complex {
            // Complex key with null value: just the key
            output.push_str(&inner_indent);
            format_node(&entry.key, output, depth + 1, options, false, true);
        } else if value_is_multiline {
            // Simple key with multiline value: key:\n  value
            output.push_str(&inner_indent);
            format_node(&entry.key, output, depth + 1, options, false, true);
            output.push_str(":\n");
            let value_indent = indent_str(depth + 2, options);
            output.push_str(&value_indent);
            format_node(&entry.value, output, depth + 2, options, false, true);
        } else {
            // Simple key with simple value (or null)
            output.push_str(&inner_indent);
            format_node(&entry.key, output, depth + 1, options, false, true);
            if !value_is_null {
                // Alias keys need space before colon
                if matches!(&entry.key, Node::Alias(_)) {
                    output.push_str(" : ");
                } else {
                    output.push_str(": ");
                }
                format_node(&entry.value, output, depth + 1, options, false, true);
            }
        }
        // Always trailing comma (prettier style)
        output.push(',');
        if let Some(comment) = &entry.trailing_comment {
            output.push(' ');
            output.push_str(comment);
        }
        output.push('\n');
    }
    output.push_str(&outer_indent);
    output.push('}');
}

pub(crate) fn format_flow_sequence(
    seq: &SequenceNode,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
) {
    let has_props = seq.tag.is_some() || seq.anchor.is_some();
    let has_middle_comments = !seq.middle_comments.is_empty();

    // Write tag and anchor
    if let Some(tag) = &seq.tag {
        output.push_str(tag);
        output.push(' ');
    }
    if let Some(anchor) = &seq.anchor {
        output.push('&');
        output.push_str(anchor);
        output.push(' ');
    }

    // Middle comments go between props and content
    if has_middle_comments {
        if seq.middle_comments.len() == 1 && has_props {
            // Single middle comment: on same line as props
            output.push_str(&seq.middle_comments[0].text);
            output.push('\n');
        } else {
            // Multiple: props on own line, then each comment
            if has_props {
                // Trim trailing space added after tag/anchor
                while output.ends_with(' ') {
                    output.pop();
                }
                output.push('\n');
            }
            for comment in &seq.middle_comments {
                output.push_str(&comment.text);
                output.push('\n');
            }
        }
    }

    if seq.items.is_empty() {
        output.push_str("[]");
        return;
    }

    // If there are comments, force broken format
    let has_item_comments = seq
        .items
        .iter()
        .any(|item| item.trailing_comment.is_some() || !item.leading_comments.is_empty());
    // Also check for comments in implicit mapping entries
    let has_mapping_comments = seq.items.iter().any(|item| {
        if let Node::Mapping(m) = &item.value
            && m.flow
            && m.entries.len() == 1
        {
            let e = &m.entries[0];
            return e.trailing_comment.is_some()
                || e.key_trailing_comment.is_some()
                || !e.leading_comments.is_empty()
                || !e.between_comments.is_empty();
        }
        false
    });

    if has_middle_comments
        || has_item_comments
        || has_mapping_comments
        || !seq.trailing_comments.is_empty()
    {
        format_flow_sequence_broken(seq, output, depth, options);
        return;
    }

    // Try flat format
    let flat = format_flow_sequence_flat(seq, depth, options);

    // If flat result contains newlines (nested broken collections), go to broken
    let current_col = depth * options.tab_width;
    if !flat.contains('\n') && current_col + flat.len() <= options.print_width {
        output.push_str(&flat);
    } else {
        format_flow_sequence_broken(seq, output, depth, options);
    }
}

fn format_flow_sequence_flat(
    seq: &SequenceNode,
    depth: usize,
    options: &YamlFormatOptions,
) -> String {
    let mut parts = Vec::new();
    for item in &seq.items {
        let mut part = String::new();
        // Check if item is a single-entry flow mapping (key-value pair in sequence)
        if let Node::Mapping(m) = &item.value
            && m.flow
            && m.entries.len() == 1
        {
            let entry = &m.entries[0];
            let key_is_null = is_null_value(&entry.key);
            // If value is a collection and key is a simple scalar, wrap in {} for clarity
            let value_is_collection = matches!(&entry.value, Node::Mapping(_) | Node::Sequence(_));
            let key_is_simple = matches!(&entry.key, Node::Scalar(_)) && !key_is_null;
            if value_is_collection && key_is_simple && !entry.is_explicit_key {
                // Format as { key: value } (explicit mapping)
                format_node(&item.value, &mut part, depth, options, false, true);
                parts.push(part);
                continue;
            }
            if key_is_null && is_null_value(&entry.value) {
                // null key + null value = `: `
                part.push_str(": ");
                parts.push(part);
                continue;
            }
            if entry.is_explicit_key {
                part.push_str("? ");
            }
            format_node(&entry.key, &mut part, depth, options, false, true);
            if !is_null_value(&entry.value) {
                // Alias keys need space before colon
                if matches!(&entry.key, Node::Alias(_)) {
                    part.push_str(" : ");
                } else {
                    part.push_str(": ");
                }
                format_node(&entry.value, &mut part, depth, options, false, true);
            }
            parts.push(part);
            continue;
        }
        format_node(&item.value, &mut part, depth, options, false, true);
        parts.push(part);
    }
    format!("[{}]", parts.join(", "))
}

fn format_flow_sequence_broken(
    seq: &SequenceNode,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
) {
    let inner_indent = indent_str(depth + 1, options);
    let outer_indent = indent_str(depth, options);

    output.push_str("[\n");
    for (i, item) in seq.items.iter().enumerate() {
        // Leading comments (each may have its own blank_line_before)
        for comment in &item.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            output.push_str(&inner_indent);
            output.push_str(&comment.text);
            output.push('\n');
        }

        // Blank line between last leading comment and the item
        if item.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        // Check if item is a single-entry flow mapping (key-value pair in sequence)
        if let Node::Mapping(m) = &item.value
            && m.flow
            && m.entries.len() == 1
        {
            let entry = &m.entries[0];
            let key_is_complex = is_collection(&entry.key);
            let key_is_null = is_null_value(&entry.key);
            let value_is_null = is_null_value(&entry.value);
            let has_between =
                !entry.between_comments.is_empty() || entry.key_trailing_comment.is_some();
            let value_is_collection = matches!(&entry.value, Node::Mapping(_) | Node::Sequence(_));
            let key_is_simple = matches!(&entry.key, Node::Scalar(_)) && !key_is_null;

            // If value is a collection and key is simple, format as { key: value } for clarity
            if value_is_collection && key_is_simple && !entry.is_explicit_key {
                output.push_str(&inner_indent);
                format_node(&item.value, output, depth + 1, options, false, true);
                output.push(',');
                output.push('\n');
                continue;
            }

            if key_is_null && value_is_null {
                // null:null -> ": "
                output.push_str(&inner_indent);
                output.push_str(": ");
            } else if (key_is_complex || has_between) && !value_is_null {
                // ? key \n [comments] \n : value
                output.push_str(&inner_indent);
                output.push_str("? ");
                format_node(&entry.key, output, depth + 2, options, false, true);
                if let Some(comment) = &entry.key_trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                for comment in &entry.between_comments {
                    output.push('\n');
                    output.push_str(&inner_indent);
                    output.push_str(&comment.text);
                }
                output.push('\n');
                output.push_str(&inner_indent);
                output.push_str(": ");
                format_node(&entry.value, output, depth + 2, options, false, true);
            } else if key_is_complex || (entry.is_explicit_key && value_is_null) {
                // ? key (null value) — explicit key syntax for long keys
                output.push_str(&inner_indent);
                output.push_str("? ");
                format_node(&entry.key, output, depth + 2, options, false, true);
            } else {
                // simple key: value
                output.push_str(&inner_indent);
                format_node(&entry.key, output, depth + 1, options, false, true);
                if !value_is_null {
                    // Alias keys need space before colon
                    if matches!(&entry.key, Node::Alias(_)) {
                        output.push_str(" : ");
                    } else {
                        output.push_str(": ");
                    }
                    format_node(&entry.value, output, depth + 2, options, false, true);
                }
            }
            output.push(',');
            if let Some(comment) = &entry.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
            continue;
        }

        output.push_str(&inner_indent);
        format_node(&item.value, output, depth + 1, options, false, true);
        output.push(',');
        if let Some(comment) = &item.trailing_comment {
            output.push(' ');
            output.push_str(comment);
        }
        output.push('\n');
    }
    output.push_str(&outer_indent);
    output.push(']');
}
