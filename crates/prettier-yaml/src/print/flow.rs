use crate::YamlFormatOptions;
use crate::ast::{MappingNode, Node, SequenceNode};
use crate::print::misc::{indent_str, renders_multiline};
use crate::printer::format_node;
use crate::utilities::{has_node_props, is_collection, is_null_value, needs_space_before_colon};

pub(crate) fn format_flow_mapping(
    mapping: &MappingNode,
    output: &mut String,
    indent: usize,
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
        format_flow_mapping_broken(mapping, output, indent, options);
        return;
    }

    // Try flat format first
    let flat = format_flow_mapping_flat(mapping, indent, options);

    // If flat result contains newlines (nested broken collections), go to broken
    if !flat.contains('\n') && indent + flat.len() <= options.print_width {
        output.push_str(&flat);
    } else {
        // Break to multi-line
        format_flow_mapping_broken(mapping, output, indent, options);
    }
}

fn format_flow_mapping_flat(
    mapping: &MappingNode,
    indent: usize,
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
        format_node(&entry.key, &mut part, indent, options, false, true);
        if is_null_value(&entry.value) && !has_node_props(&entry.value) {
            // Null value without tag/anchor: just the key, no ": ~"
        } else if is_null_value(&entry.value) && has_node_props(&entry.value) {
            // Null value with tag/anchor (e.g. `foo: !!str`): keep colon and props
            if needs_space_before_colon(&entry.key) {
                part.push_str(" : ");
            } else {
                part.push_str(": ");
            }
            format_node(&entry.value, &mut part, indent, options, false, true);
            // Remove trailing empty value, keep just the props
            part.push(' ');
        } else {
            // Alias/tagged keys need space before colon
            if needs_space_before_colon(&entry.key) {
                part.push_str(" : ");
            } else {
                part.push_str(": ");
            }
            format_node(&entry.value, &mut part, indent, options, false, true);
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
    indent: usize,
    options: &YamlFormatOptions,
) {
    let tw = options.tab_width;
    let inner_indent = indent + tw;
    let inner_indent_s = indent_str(inner_indent);
    let outer_indent_s = indent_str(indent);

    output.push_str("{\n");
    for (i, entry) in mapping.entries.iter().enumerate() {
        // Leading comments (each may have its own blank_line_before)
        for comment in &entry.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            output.push_str(&inner_indent_s);
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
            !value_is_null && renders_multiline(&entry.value, inner_indent + tw, options);

        let has_between =
            !entry.between_comments.is_empty() || entry.key_trailing_comment.is_some();

        if key_is_null && value_is_null {
            // null key + null value = `: `
            output.push_str(&inner_indent_s);
            output.push_str(": ");
        } else if (key_is_complex || has_between) && !value_is_null {
            // Complex key or comments between key-value: use ? key \n [comments] \n : value
            output.push_str(&inner_indent_s);
            output.push_str("? ");
            format_node(&entry.key, output, inner_indent + 2, options, false, true);
            if let Some(comment) = &entry.key_trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            for comment in &entry.between_comments {
                output.push('\n');
                output.push_str(&inner_indent_s);
                output.push_str(&comment.text);
            }
            output.push('\n');
            output.push_str(&inner_indent_s);
            output.push_str(": ");
            format_node(&entry.value, output, inner_indent + 2, options, false, true);
        } else if key_is_complex {
            // Complex key with null value: just the key
            output.push_str(&inner_indent_s);
            format_node(&entry.key, output, inner_indent, options, false, true);
        } else if value_is_multiline {
            // Simple key with multiline value: key:\n  value
            output.push_str(&inner_indent_s);
            format_node(&entry.key, output, inner_indent, options, false, true);
            output.push_str(":\n");
            let value_indent = inner_indent + tw;
            let value_indent_s = indent_str(value_indent);
            output.push_str(&value_indent_s);
            format_node(&entry.value, output, value_indent, options, false, true);
        } else {
            // Simple key with simple value (or null)
            output.push_str(&inner_indent_s);
            format_node(&entry.key, output, inner_indent, options, false, true);
            if !value_is_null {
                // Alias keys need space before colon
                if matches!(&entry.key, Node::Alias(_)) {
                    output.push_str(" : ");
                } else {
                    output.push_str(": ");
                }
                format_node(&entry.value, output, inner_indent, options, false, true);
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
    // Print trailing comments (comments after last entry, before `}`)
    for comment in &mapping.trailing_comments {
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        output.push_str(&inner_indent_s);
        output.push_str(&comment.text);
        output.push('\n');
    }
    output.push_str(&outer_indent_s);
    output.push('}');
}

pub(crate) fn format_flow_sequence(
    seq: &SequenceNode,
    output: &mut String,
    indent: usize,
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
        format_flow_sequence_broken(seq, output, indent, options);
        return;
    }

    // Try flat format
    let flat = format_flow_sequence_flat(seq, indent, options);

    // If flat result contains newlines (nested broken collections), go to broken
    if !flat.contains('\n') && indent + flat.len() <= options.print_width {
        output.push_str(&flat);
    } else {
        format_flow_sequence_broken(seq, output, indent, options);
    }
}

fn format_flow_sequence_flat(
    seq: &SequenceNode,
    indent: usize,
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
                format_node(&item.value, &mut part, indent, options, false, true);
                parts.push(part);
                continue;
            }
            if key_is_null && is_null_value(&entry.value) {
                // null key + null value = `: `
                part.push_str(": ");
                parts.push(part);
                continue;
            }
            let value_is_null = is_null_value(&entry.value) && !has_node_props(&entry.value);
            let key_renders_multiline = renders_multiline(&entry.key, indent, options);
            // Keep `?` for explicit keys with null values (e.g. `[? 1, ? 2]`)
            // or when the key renders multiline
            if entry.is_explicit_key && (value_is_null || key_renders_multiline) {
                part.push_str("? ");
            }
            format_node(&entry.key, &mut part, indent, options, false, true);
            if !value_is_null {
                // Alias/tagged keys need space before colon
                if needs_space_before_colon(&entry.key) {
                    part.push_str(" : ");
                } else {
                    part.push_str(": ");
                }
                if is_null_value(&entry.value) && has_node_props(&entry.value) {
                    format_node(&entry.value, &mut part, indent, options, false, true);
                    part.push(' ');
                } else {
                    format_node(&entry.value, &mut part, indent, options, false, true);
                }
            }
            parts.push(part);
            continue;
        }
        format_node(&item.value, &mut part, indent, options, false, true);
        parts.push(part);
    }
    format!("[{}]", parts.join(", "))
}

#[allow(clippy::too_many_lines)]
fn format_flow_sequence_broken(
    seq: &SequenceNode,
    output: &mut String,
    indent: usize,
    options: &YamlFormatOptions,
) {
    let tw = options.tab_width;
    let inner_indent = indent + tw;
    let inner_indent_s = indent_str(inner_indent);
    let outer_indent_s = indent_str(indent);

    output.push_str("[\n");
    for (i, item) in seq.items.iter().enumerate() {
        // Leading comments (each may have its own blank_line_before)
        for comment in &item.leading_comments {
            if comment.blank_line_before && i > 0 && !output.ends_with("\n\n") {
                output.push('\n');
            }
            output.push_str(&inner_indent_s);
            output.push_str(&comment.text);
            output.push('\n');
        }

        // Blank line between last leading comment and the item
        if item.blank_line_before && i > 0 && !output.ends_with("\n\n") {
            output.push('\n');
        }

        // prettier-ignore: output raw source for the value
        if item.prettier_ignore {
            output.push_str(&inner_indent_s);
            let used_raw = match &item.value {
                Node::Mapping(m) if m.flow_source.is_some() => {
                    if let Some(fs) = m.flow_source.as_ref() {
                        output.push_str(fs);
                    }
                    true
                }
                Node::Sequence(s) if s.flow_source.is_some() => {
                    if let Some(fs) = s.flow_source.as_ref() {
                        output.push_str(fs);
                    }
                    true
                }
                _ => false,
            };
            if !used_raw {
                format_node(&item.value, output, inner_indent, options, false, true);
            }
            output.push(',');
            // Check item's trailing comment, or look inside flow mapping/sequence entries
            if let Some(comment) = &item.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            } else if let Node::Mapping(m) = &item.value
                && m.flow
                && m.entries.len() == 1
                && let Some(comment) = &m.entries[0].trailing_comment
            {
                output.push(' ');
                output.push_str(comment);
            } else if let Node::Sequence(s) = &item.value
                && s.flow
                && let Some(last_item) = s.items.last()
                && let Some(comment) = &last_item.trailing_comment
            {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
            continue;
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
                output.push_str(&inner_indent_s);
                format_node(&item.value, output, inner_indent, options, false, true);
                output.push(',');
                output.push('\n');
                continue;
            }

            if key_is_null && value_is_null {
                // null:null -> ": "
                output.push_str(&inner_indent_s);
                output.push_str(": ");
            } else if (key_is_complex || has_between) && !value_is_null {
                // ? key \n [comments] \n : value
                output.push_str(&inner_indent_s);
                output.push_str("? ");
                format_node(&entry.key, output, inner_indent + 2, options, false, true);
                if let Some(comment) = &entry.key_trailing_comment {
                    output.push(' ');
                    output.push_str(comment);
                }
                for comment in &entry.between_comments {
                    output.push('\n');
                    output.push_str(&inner_indent_s);
                    output.push_str(&comment.text);
                }
                output.push('\n');
                output.push_str(&inner_indent_s);
                output.push_str(": ");
                format_node(&entry.value, output, inner_indent + 2, options, false, true);
            } else if key_is_complex || (entry.is_explicit_key && value_is_null) {
                // ? key (null value) — explicit key syntax for long keys
                output.push_str(&inner_indent_s);
                output.push_str("? ");
                format_node(&entry.key, output, inner_indent + 2, options, false, true);
            } else if value_is_null {
                // Single key with null value: output as { key } to preserve braces
                output.push_str(&inner_indent_s);
                format_node(&item.value, output, inner_indent, options, false, true);
            } else {
                // simple key: value
                output.push_str(&inner_indent_s);
                let key_start_pos = output.len();
                format_node(&entry.key, output, inner_indent, options, false, true);
                // Alias keys need space before colon
                if matches!(&entry.key, Node::Alias(_)) {
                    output.push_str(" : ");
                } else {
                    output.push_str(": ");
                }
                // Value indent = position after "key: " (relative to line start)
                let key_width = output.len() - key_start_pos;
                let value_indent = inner_indent + key_width;
                format_node(&entry.value, output, value_indent, options, false, true);
            }
            output.push(',');
            if let Some(comment) = &entry.trailing_comment {
                output.push(' ');
                output.push_str(comment);
            }
            output.push('\n');
            continue;
        }

        output.push_str(&inner_indent_s);
        format_node(&item.value, output, inner_indent, options, false, true);
        output.push(',');
        if let Some(comment) = &item.trailing_comment {
            output.push(' ');
            output.push_str(comment);
        }
        output.push('\n');
    }
    // Print trailing comments (comments after last item, before `]`)
    for comment in &seq.trailing_comments {
        if comment.blank_line_before && !output.ends_with("\n\n") {
            output.push('\n');
        }
        output.push_str(&inner_indent_s);
        output.push_str(&comment.text);
        output.push('\n');
    }
    output.push_str(&outer_indent_s);
    output.push(']');
    if let Some(comment) = &seq.closing_comment {
        output.push(' ');
        output.push_str(comment);
    }
}
