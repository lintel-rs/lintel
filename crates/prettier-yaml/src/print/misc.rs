use crate::YamlFormatOptions;
use crate::ast::{Comment, Node};

pub(crate) fn indent_str(depth: usize, options: &YamlFormatOptions) -> String {
    // YAML does not support tab indentation, so always use spaces
    // (useTabs is ignored for YAML, matching prettier's behavior)
    " ".repeat(depth * options.tab_width)
}

/// Compute the correct indent for a comment based on both structural depth and
/// the comment's original source column. Prettier normalizes comment indentation
/// to the nearest structural level that is >= the comment's source depth.
pub(crate) fn comment_indent(
    comment: &Comment,
    min_depth: usize,
    options: &YamlFormatOptions,
) -> String {
    let tw = options.tab_width;
    // Compute the depth implied by the comment's source column
    let source_depth = if tw > 0 { comment.col.div_ceil(tw) } else { 0 };
    let depth = min_depth.max(source_depth);
    indent_str(depth, options)
}

/// Like `comment_indent` but caps the depth to prevent comments from rendering
/// deeper than the structural context allows.
pub(crate) fn comment_indent_capped(
    comment: &Comment,
    min_depth: usize,
    max_depth: usize,
    options: &YamlFormatOptions,
) -> String {
    let tw = options.tab_width;
    let source_depth = if tw > 0 { comment.col.div_ceil(tw) } else { 0 };
    let depth = min_depth.max(source_depth).min(max_depth);
    indent_str(depth, options)
}

/// Check if a node would render as multi-line (contains newlines).
pub(crate) fn renders_multiline(node: &Node, depth: usize, options: &YamlFormatOptions) -> bool {
    let mut buf = String::new();
    crate::printer::format_node(node, &mut buf, depth, options, false, true);
    buf.contains('\n')
}
