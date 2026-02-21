use alloc::borrow::Cow;

use saphyr_parser::{ScalarStyle, Tag};

use crate::ast::Node;

pub(crate) fn is_simple_value(node: &Node) -> bool {
    match node {
        Node::Scalar(s) => !s.is_implicit_null,
        Node::Alias(_) => true,
        Node::Mapping(m) => m.flow,
        Node::Sequence(s) => s.flow,
    }
}

pub(crate) fn is_null_value(node: &Node) -> bool {
    match node {
        Node::Scalar(s) => s.is_implicit_null,
        _ => false,
    }
}

pub(crate) fn is_block_scalar_value(node: &Node) -> bool {
    matches!(
        node,
        Node::Scalar(s) if matches!(s.style, ScalarStyle::Literal | ScalarStyle::Folded)
    )
}

/// Check if a node has properties (anchor, tag) that would need a space separator.
pub(crate) fn has_node_props(node: &Node) -> bool {
    match node {
        Node::Mapping(m) => m.anchor.is_some() || m.tag.is_some(),
        Node::Sequence(s) => s.anchor.is_some() || s.tag.is_some(),
        Node::Scalar(s) => s.anchor.is_some() || s.tag.is_some(),
        Node::Alias(_) => false,
    }
}

/// Check if we need a space before the `:` mapping value indicator.
/// This is needed when the key ends with characters that would be
/// ambiguous without a space (tagged null keys, aliases).
pub(crate) fn needs_space_before_colon(node: &Node) -> bool {
    match node {
        Node::Scalar(s) if s.is_implicit_null && s.tag.is_some() => true,
        Node::Alias(_) => true,
        _ => false,
    }
}

/// Check if a node is a collection (mapping or sequence).
pub(crate) fn is_collection(node: &Node) -> bool {
    matches!(node, Node::Mapping(_) | Node::Sequence(_))
}

/// Check if a node is a block (non-flow) collection — only these need explicit key format.
pub(crate) fn is_block_collection(node: &Node) -> bool {
    matches!(node, Node::Mapping(m) if !m.flow) || matches!(node, Node::Sequence(s) if !s.flow)
}

/// Check if a character is valid in a YAML anchor/alias name.
/// YAML spec: any character except flow indicators ([]{},) and whitespace.
pub(crate) fn is_anchor_char(c: char) -> bool {
    !c.is_whitespace() && !matches!(c, '[' | ']' | '{' | '}' | ',')
}

#[allow(dead_code, clippy::ptr_arg)]
pub(crate) fn format_tag(tag: &Cow<'_, Tag>) -> String {
    if tag.handle.is_empty() && tag.suffix == "!" {
        // Non-specific tag: just "!"
        "!".to_string()
    } else if tag.handle.is_empty() {
        // Verbatim tag: !<suffix> (saphyr strips the angle brackets)
        format!("!<{}>", tag.suffix)
    } else if tag.handle == "!" {
        format!("!{}", tag.suffix)
    } else if tag.handle == "!!" || tag.handle == "tag:yaml.org,2002:" {
        format!("!!{}", tag.suffix)
    } else {
        format!("{}!{}", tag.handle, tag.suffix)
    }
}
