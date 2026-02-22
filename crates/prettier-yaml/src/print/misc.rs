use crate::ast::{Comment, Node};
use prettier_config::PrettierConfig;

/// Create an indentation string of `indent` spaces.
/// `indent` is the actual column count (number of spaces), not a depth level.
pub(crate) fn indent_str(indent: usize) -> String {
    // YAML does not support tab indentation, so always use spaces
    // (useTabs is ignored for YAML, matching prettier's behavior)
    " ".repeat(indent)
}

/// Compute the correct indent for a comment based on both structural indent
/// and the comment's original source column. Prettier normalizes comment
/// indentation to the nearest `tab_width` boundary that is >= `min_indent`.
pub(crate) fn comment_indent(
    comment: &Comment,
    min_indent: usize,
    options: &PrettierConfig,
) -> String {
    let tw = options.tab_width;
    // Snap the source column UP to the nearest tab_width boundary.
    // This normalizes off-grid indentation to the nearest structural
    // indent level (e.g., 1-space indent with tab_width=2 becomes 2).
    let snapped = if tw > 0 {
        comment.col.div_ceil(tw) * tw
    } else {
        comment.col
    };
    let indent = min_indent.max(snapped);
    " ".repeat(indent)
}

/// Like `comment_indent` but caps the indent to prevent comments from rendering
/// deeper than the structural context allows.
pub(crate) fn comment_indent_capped(
    comment: &Comment,
    min_indent: usize,
    max_indent: usize,
    options: &PrettierConfig,
) -> String {
    let tw = options.tab_width;
    let snapped = if tw > 0 {
        comment.col.div_ceil(tw) * tw
    } else {
        comment.col
    };
    let indent = min_indent.max(snapped).min(max_indent);
    " ".repeat(indent)
}

/// Check if a node would render as multi-line (contains newlines).
pub(crate) fn renders_multiline(node: &Node, indent: usize, options: &PrettierConfig) -> bool {
    let mut buf = String::new();
    crate::printer::format_node(node, &mut buf, indent, options, false, true);
    buf.contains('\n')
}
