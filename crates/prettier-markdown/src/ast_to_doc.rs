use comrak::arena_tree::Node;
use comrak::nodes::{
    Ast, ListDelimType, ListType, NodeCode, NodeCodeBlock, NodeHeading, NodeHtmlBlock, NodeLink,
    NodeList, NodeValue,
};
use comrak::{Arena, Options, parse_document};
use core::cell::RefCell;
use core::fmt::Write as _;
use prettier_config::{PrettierConfig, ProseWrap};
use unicode_width::UnicodeWidthStr;
use wadler_lindig::Doc;

/// Convert a markdown string to a wadler-lindig `Doc` IR.
pub fn markdown_to_doc(content: &str, options: &PrettierConfig) -> Doc {
    let arena = Arena::new();
    let opts = comrak_options();
    let root = parse_document(&arena, content, &opts);

    let children: Vec<&Node<RefCell<Ast>>> = root.children().collect();
    if children.is_empty() {
        return Doc::text("");
    }

    let mut docs = render_block_sequence(&children, options, content, true);

    // Trailing newline
    docs.push(Doc::Hardline);

    Doc::concat(docs)
}

/// Create comrak `Options` for parsing.
fn comrak_options() -> Options<'static> {
    let mut opts = Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.autolink = true;
    opts.extension.tasklist = true;
    opts.render.hardbreaks = false;
    opts
}

/// Render a sequence of block-level children, managing consecutive unordered list
/// bullet alternation to prevent merging (prettier's list separation behavior).
fn render_block_sequence<'a>(
    children: &[&'a Node<'a, RefCell<Ast>>],
    options: &PrettierConfig,
    source: &str,
    blank_line_between: bool,
) -> Vec<Doc> {
    let mut docs = Vec::new();
    let mut consecutive_bullet_lists = 0usize;

    for (i, child) in children.iter().enumerate() {
        // Check if previous child was a list followed by this unfenced code block
        let prev_was_list_before_code = i > 0 && {
            let prev_is_list = {
                let d = children[i - 1].data.borrow();
                matches!(&d.value, NodeValue::List(_))
            };
            let this_is_unfenced_code = {
                let d = child.data.borrow();
                matches!(&d.value, NodeValue::CodeBlock(cb) if !cb.fenced)
            };
            prev_is_list && this_is_unfenced_code
        };

        if i > 0 {
            docs.push(Doc::Hardline);
            if blank_line_between {
                docs.push(Doc::Hardline);
            }
            // Extra blank line between list and following unfenced code block
            if prev_was_list_before_code {
                docs.push(Doc::Hardline);
            }
        }

        let data = child.data.borrow();
        let is_bullet_list =
            matches!(&data.value, NodeValue::List(l) if l.list_type == ListType::Bullet);
        drop(data);

        // Check if this list is followed by an unfenced code block
        let followed_by_code_indent = if i + 1 < children.len() {
            let is_list = {
                let d = child.data.borrow();
                matches!(&d.value, NodeValue::List(_))
            };
            if is_list {
                let next = children[i + 1];
                let nd = next.data.borrow();
                if let NodeValue::CodeBlock(cb) = &nd.value {
                    if cb.fenced {
                        None
                    } else {
                        // Compute actual source indent: 4 (stripped by parser) + leading
                        // spaces remaining in the literal content
                        let extra_spaces = cb.literal.bytes().take_while(|&b| b == b' ').count();
                        Some(4 + extra_spaces)
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if is_bullet_list {
            let bullet_override = match consecutive_bullet_lists % 3 {
                0 => '-',
                1 => '*',
                _ => '+',
            };
            consecutive_bullet_lists += 1;
            docs.push(block_to_doc_with_options(
                child,
                options,
                source,
                Some(bullet_override),
                followed_by_code_indent,
            ));
        } else {
            consecutive_bullet_lists = 0;
            if followed_by_code_indent.is_some() {
                docs.push(block_to_doc_with_options(
                    child,
                    options,
                    source,
                    None,
                    followed_by_code_indent,
                ));
            } else {
                docs.push(block_to_doc(child, options, source));
            }
        }
    }

    docs
}

/// Convert a block-level node to Doc.
fn block_to_doc<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    options: &PrettierConfig,
    source: &str,
) -> Doc {
    block_to_doc_with_options(node, options, source, None, None)
}

/// Convert a block-level node to Doc, with optional bullet char override and
/// code block indent preservation for lists followed by unfenced code blocks.
#[allow(clippy::too_many_arguments)]
fn block_to_doc_with_options<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    options: &PrettierConfig,
    source: &str,
    bullet_override: Option<char>,
    followed_by_code_indent: Option<usize>,
) -> Doc {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Paragraph => paragraph_to_doc(node, options, source),
        NodeValue::Heading(heading) => heading_to_doc(node, *heading, options, source),
        NodeValue::CodeBlock(_) => {
            drop(data);
            code_block_to_doc_node(node, source)
        }
        NodeValue::BlockQuote => blockquote_to_doc(node, options, source),
        NodeValue::List(list) => list_to_doc(
            node,
            list,
            options,
            source,
            bullet_override,
            followed_by_code_indent,
        ),
        NodeValue::Item(_) | NodeValue::TaskItem(_) => {
            render_block_children(node, options, false, source)
        }
        NodeValue::ThematicBreak => {
            // Use *** inside `-` bulleted list items to avoid confusion with `---`.
            // Inside `*`/`+` bulleted items, `---` is unambiguous.
            let in_dash_list = node.parent().is_some_and(|item| {
                let id = item.data.borrow();
                matches!(&id.value, NodeValue::Item(_) | NodeValue::TaskItem(_))
                    && item.parent().is_some_and(|list| {
                        let ld = list.data.borrow();
                        matches!(&ld.value, NodeValue::List(l) if l.bullet_char == b'-')
                    })
            });
            if in_dash_list {
                Doc::text("***")
            } else {
                Doc::text("---")
            }
        }
        NodeValue::HtmlBlock(html) => html_block_to_doc(html),
        NodeValue::Table(_) => table_to_doc(node, source, options),
        NodeValue::Document => {
            let children: Vec<&Node<RefCell<Ast>>> = node.children().collect();
            Doc::concat(render_block_sequence(&children, options, source, true))
        }
        _ => Doc::text(""),
    }
}

// ─── Paragraphs ──────────────────────────────────────────────────────────

fn paragraph_to_doc<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    options: &PrettierConfig,
    source: &str,
) -> Doc {
    match options.prose_wrap {
        ProseWrap::Always => wrap_paragraph_always_inline(node, options, source),
        ProseWrap::Never | ProseWrap::Preserve => {
            let inline_docs = collect_inline_children(node, options, source);
            Doc::concat(inline_docs)
        }
    }
}

/// Walk inline children and build Fill parts for `proseWrap: "always"`.
///
/// Inline nodes like links, emphasis, and code spans are treated as atomic units
/// (they won't be broken across lines). Text nodes are split on spaces into
/// individual word parts. `SoftBreak` becomes a Line (wrap point).
/// `LineBreak` (hard break) forces a line break.
fn wrap_paragraph_always_inline<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    options: &PrettierConfig,
    source: &str,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();

    // Track whether there's a pending space boundary before the next content.
    // This handles space preservation at text↔inline-node boundaries.
    #[allow(clippy::too_many_arguments, clippy::items_after_statements)]
    fn collect_fill_parts<'a>(
        node: &'a Node<'a, RefCell<Ast>>,
        parts: &mut Vec<Doc>,
        pending_space: &mut bool,
        options: &PrettierConfig,
        source: &str,
    ) {
        for child in node.children() {
            let data = child.data.borrow();
            match &data.value {
                NodeValue::Text(text) => {
                    let starts_with_space = text.starts_with(' ');
                    let ends_with_space = text.ends_with(' ');
                    let words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();

                    // Leading space in text + existing content → space boundary
                    if starts_with_space && !parts.is_empty() {
                        *pending_space = true;
                    }

                    for (i, word) in words.iter().enumerate() {
                        if *pending_space || i > 0 {
                            parts.push(Doc::Line);
                            *pending_space = false;
                        }
                        parts.push(Doc::text(*word));
                    }

                    // Trailing space → pending for next node
                    if ends_with_space && !words.is_empty() {
                        *pending_space = true;
                    }
                }
                NodeValue::SoftBreak => {
                    parts.push(Doc::Line);
                    *pending_space = false;
                }
                NodeValue::LineBreak => {
                    let sourcepos = data.sourcepos;
                    let uses_backslash = detect_hard_break_style(source, &sourcepos);
                    *pending_space = false;
                    if uses_backslash {
                        parts.push(Doc::text("\\"));
                    } else {
                        parts.push(Doc::text("  "));
                    }
                    parts.push(Doc::Hardline);
                }
                // All other inline nodes (Link, Image, Emph, Strong, Code, etc.)
                // are treated as atomic units — they won't be broken during wrapping
                _ => {
                    drop(data);
                    if *pending_space {
                        parts.push(Doc::Line);
                        *pending_space = false;
                    }
                    let doc = inline_to_doc(child, options, source);
                    parts.push(doc);
                }
            }
        }
    }

    let mut pending_space = false;
    collect_fill_parts(node, &mut parts, &mut pending_space, options, source);

    if parts.is_empty() {
        return Doc::text("");
    }

    // Fill expects alternating [content, separator, content, ...]. Merge adjacent
    // non-separator (non-Line) parts into a single Concat so the pattern holds.
    // This happens when e.g. a period directly follows a link without space.
    let parts = merge_adjacent_content(parts);
    let parts = prevent_accidental_prefixes(parts);
    let parts = replace_cjk_line_separators(parts);

    Doc::fill(parts)
}

/// Detect whether a hard break in the source uses backslash (`\`) or trailing spaces (`  `).
///
/// comrak sometimes collapses multi-line content (e.g., setext headings) into
/// a single sourcepos line range, making the reported line number unreliable.
/// We handle this by scanning forward through source lines when the column
/// exceeds the current line's length.
fn detect_hard_break_style(source: &str, sourcepos: &comrak::nodes::Sourcepos) -> bool {
    if sourcepos.start.line == 0 {
        return false;
    }

    let lines: Vec<&str> = source.lines().collect();
    let mut effective_line = sourcepos.start.line; // 1-indexed
    let mut remaining_col = sourcepos.start.column; // 1-indexed

    // Scan forward if the column is beyond the current line's length.
    // This corrects for comrak's collapsed sourcepos in setext headings.
    while effective_line <= lines.len() {
        let line_len = lines.get(effective_line - 1).map_or(0, |l| l.len());
        if remaining_col <= line_len {
            break;
        }
        remaining_col -= line_len + 1; // skip line + newline
        effective_line += 1;
    }

    lines
        .get(effective_line - 1)
        .is_some_and(|line| line.trim_end().ends_with('\\'))
}

/// Check if a text token would be interpreted as markdown syntax at line start.
///
/// Dangerous prefixes: `>` (blockquote), `-`/`*`/`+` (list markers),
/// `#` through `######` (headings), `\d+.`/`\d+)` (ordered list markers).
fn is_dangerous_at_line_start(text: &str) -> bool {
    // Blockquote markers: one or more `>`
    if !text.is_empty() && text.bytes().all(|b| b == b'>') {
        return true;
    }
    // Unordered list markers
    if text == "-" || text == "*" || text == "+" {
        return true;
    }
    // Heading markers: 1-6 `#` chars
    if !text.is_empty() && text.len() <= 6 && text.bytes().all(|b| b == b'#') {
        return true;
    }
    // Ordered list markers: digits followed by `.` or `)`
    let bytes = text.as_bytes();
    if bytes.len() >= 2 {
        let last = bytes[bytes.len() - 1];
        if (last == b'.' || last == b')') && bytes[..bytes.len() - 1].iter().all(u8::is_ascii_digit)
        {
            return true;
        }
    }
    false
}

/// Get the first text content of a Doc, if any.
fn first_text_of(doc: &Doc) -> Option<&str> {
    match doc {
        Doc::Text(s) => Some(s.as_str()),
        Doc::Concat(parts) => parts.iter().find_map(first_text_of),
        _ => None,
    }
}

/// Prevent accidental markdown syntax when Fill wraps text.
///
/// When a word that looks like a markdown prefix (e.g., `>`, `-`, `#`)
/// would appear at the start of a line after wrapping, we glue it to
/// the preceding word using a hard space so they stay on the same line.
///
/// Parts must be in alternating [content, Line, content, Line, ...] form.
fn prevent_accidental_prefixes(parts: Vec<Doc>) -> Vec<Doc> {
    if parts.len() < 3 {
        return parts;
    }

    let mut result: Vec<Doc> = Vec::new();
    let mut iter = parts.into_iter();

    // First element is always content
    if let Some(first) = iter.next() {
        result.push(first);
    }

    // Process remaining pairs: [separator, content, separator, content, ...]
    while let Some(sep) = iter.next() {
        if let Some(content) = iter.next() {
            let is_dangerous = matches!(&sep, Doc::Line)
                && first_text_of(&content).is_some_and(is_dangerous_at_line_start);

            if is_dangerous {
                // Glue to previous content with a non-breaking space
                let prev = result.pop().unwrap_or_else(|| Doc::text(""));
                result.push(Doc::concat(vec![prev, Doc::text(" "), content]));
            } else {
                result.push(sep);
                result.push(content);
            }
        } else {
            // Trailing separator (shouldn't happen but handle gracefully)
            result.push(sep);
        }
    }

    result
}

/// Merge adjacent non-separator (non-Line/non-Hardline) parts into a single
/// Concat so that Fill's alternating [content, separator, content, ...] contract holds.
fn merge_adjacent_content(parts: Vec<Doc>) -> Vec<Doc> {
    let mut merged: Vec<Doc> = Vec::new();
    for part in parts {
        let is_separator = matches!(&part, Doc::Line | Doc::Hardline);
        if is_separator {
            merged.push(part);
        } else if let Some(last) = merged.last() {
            if matches!(last, Doc::Line | Doc::Hardline) {
                // Previous was separator — start new content
                merged.push(part);
            } else {
                // Previous was content — merge
                let prev = merged.pop().unwrap_or_else(|| Doc::text(""));
                merged.push(Doc::concat(vec![prev, part]));
            }
        } else {
            merged.push(part);
        }
    }
    merged
}

// ─── Headings ─────────────────────────────────────────────────────────────

/// Collect inline children for setext headings with sequential line tracking.
///
/// comrak collapses multi-line setext heading content into a single sourcepos
/// range, giving all `LineBreak` nodes incorrect line numbers. This function
/// tracks the current source line by incrementing after each `LineBreak`,
/// starting from the heading's start line.
fn collect_setext_inline_children<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    options: &PrettierConfig,
    source: &str,
) -> Vec<Doc> {
    let start_line = node.data.borrow().sourcepos.start.line;
    let source_lines: Vec<&str> = source.lines().collect();
    let mut current_line = start_line;
    let mut docs = Vec::new();

    for child in node.children() {
        let data = child.data.borrow();
        if matches!(&data.value, NodeValue::LineBreak) {
            // Determine break style from the correct source line
            let uses_backslash = source_lines
                .get(current_line - 1)
                .is_some_and(|line| line.trim_end().ends_with('\\'));
            drop(data);
            if uses_backslash {
                docs.push(Doc::concat(vec![Doc::text("\\"), Doc::Hardline]));
            } else {
                docs.push(Doc::concat(vec![Doc::text("  "), Doc::Hardline]));
            }
            current_line += 1;
        } else {
            drop(data);
            docs.push(inline_to_doc(child, options, source));
        }
    }
    docs
}

fn heading_to_doc<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    heading: NodeHeading,
    options: &PrettierConfig,
    source: &str,
) -> Doc {
    let inline = if heading.setext {
        // Setext headings: comrak collapses multi-line content into incorrect
        // sourcepos (all LineBreak nodes report wrong line numbers). Use
        // sequential line tracking to correctly determine hard break style.
        collect_setext_inline_children(node, options, source)
    } else {
        collect_inline_children(node, options, source)
    };

    if heading.setext {
        // Setext heading: preserve original style with underline
        let underline_char = if heading.level == 1 { "=" } else { "-" };

        // Get underline length from source text (count trailing = or - chars,
        // to handle blockquote/list prefixes on the source line)
        let underline_c = if heading.level == 1 { '=' } else { '-' };
        let sourcepos = node.data.borrow().sourcepos;
        let source_lines: Vec<&str> = source.lines().collect();
        let underline_len = if sourcepos.end.line > 0 {
            source_lines
                .get(sourcepos.end.line - 1)
                .map(|line| {
                    line.trim_end()
                        .chars()
                        .rev()
                        .take_while(|c| *c == underline_c)
                        .count()
                })
                .filter(|&len| len > 0)
                .unwrap_or(3)
        } else {
            3
        };

        return Doc::concat(vec![
            Doc::concat(inline),
            Doc::Hardline,
            Doc::text(underline_char.repeat(underline_len)),
        ]);
    }

    // ATX heading
    let prefix = "#".repeat(usize::from(heading.level));
    Doc::concat(vec![Doc::text(prefix), Doc::text(" "), Doc::concat(inline)])
}

// ─── Code blocks ──────────────────────────────────────────────────────────

/// Code block rendering with blockquote-awareness.
///
/// comrak strips the "optional space" after `>` in blockquote continuation lines
/// (per `CommonMark` spec), but prettier preserves it for code blocks. This function
/// detects blockquote ancestry and restores the stripped space by checking source lines.
fn code_block_to_doc_node<'a>(node: &'a Node<'a, RefCell<Ast>>, source: &str) -> Doc {
    let data = node.data.borrow();
    let NodeValue::CodeBlock(cb) = &data.value else {
        return Doc::text("");
    };

    // Check if this code block is inside a blockquote
    let in_blockquote = {
        let mut current = node.parent();
        let mut found = false;
        while let Some(parent) = current {
            let pd = parent.data.borrow();
            if matches!(&pd.value, NodeValue::BlockQuote) {
                found = true;
                break;
            }
            drop(pd);
            current = parent.parent();
        }
        found
    };

    if !in_blockquote || !cb.fenced {
        return code_block_to_doc(cb);
    }

    // For fenced code blocks inside blockquotes, fix the stripped optional space.
    // Map each content line to its source line and check if `>` was followed by a space.
    let sp = data.sourcepos;
    let source_lines: Vec<&str> = source.lines().collect();
    let literal_lines: Vec<&str> = cb.literal.split('\n').collect();

    // Content lines in source are between the fence open and fence close lines.
    // The code block sourcepos covers the entire block including fences.
    // Fence open is at sp.start.line, so first content line is sp.start.line + 1.
    let content_start_line = sp.start.line + 1; // first content line (1-indexed)

    let mut fixed_lines: Vec<String> = Vec::new();
    for (i, lit_line) in literal_lines.iter().enumerate() {
        let src_line_idx = content_start_line + i; // 1-indexed source line for this content
        let needs_space = source_lines
            .get(src_line_idx.wrapping_sub(1))
            .is_some_and(|src_line| {
                // Find the `>` prefix and check if followed by a space
                let trimmed = src_line.trim_start();
                trimmed.starts_with('>') && trimmed.as_bytes().get(1) == Some(&b' ')
            });

        if needs_space && !lit_line.is_empty() {
            fixed_lines.push(format!(" {lit_line}"));
        } else {
            fixed_lines.push((*lit_line).to_string());
        }
    }

    let fixed_literal = fixed_lines.join("\n");
    let fixed_cb = NodeCodeBlock {
        fenced: cb.fenced,
        fence_char: cb.fence_char,
        fence_length: cb.fence_length,
        fence_offset: cb.fence_offset,
        info: cb.info.clone(),
        literal: fixed_literal,
    };
    drop(data);
    code_block_to_doc(&fixed_cb)
}

fn code_block_to_doc(cb: &NodeCodeBlock) -> Doc {
    // Indented code blocks: preserve as-is with 4-space indent
    if !cb.fenced {
        let literal = cb.literal.strip_suffix('\n').unwrap_or(&cb.literal);
        let lines: Vec<&str> = literal.split('\n').collect();
        let mut parts = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                parts.push(Doc::Hardline);
            }
            if line.is_empty() {
                parts.push(Doc::text(""));
            } else {
                parts.push(Doc::text(format!("    {line}")));
            }
        }
        return Doc::concat(parts);
    }

    let info = cb.info.trim();

    // Strip common leading whitespace from the literal.
    // Comrak sometimes leaves residual whitespace (e.g., when tabs are used
    // for indentation inside list items), so we normalize it.
    let literal = strip_common_leading_whitespace(&cb.literal);

    // Determine fence character: use ``` by default, ```` (quadruple) if code contains ```
    let fence = if literal.contains("```") || cb.literal.contains("```") {
        "````"
    } else {
        "```"
    };

    let mut parts = Vec::new();
    parts.push(Doc::text(format!("{fence}{info}")));
    parts.push(Doc::Hardline);

    let code = literal.strip_suffix('\n').unwrap_or(&literal);
    if !code.is_empty() {
        // Prettier collapses consecutive blank lines inside code blocks to at most 1,
        // but preserves trailing blank lines.
        let lines: Vec<&str> = code.split('\n').collect();
        let mut consecutive_blanks = 0u32;
        for (i, line) in lines.iter().enumerate() {
            let is_blank = line.is_empty();
            if is_blank {
                consecutive_blanks += 1;
                // Find if there are any non-blank lines after this point
                let has_nonblank_after = lines[i + 1..].iter().any(|l| !l.is_empty());
                if consecutive_blanks > 1 && has_nonblank_after {
                    // Collapse internal consecutive blanks: skip this one
                    continue;
                }
            } else {
                consecutive_blanks = 0;
            }
            if i > 0 {
                parts.push(Doc::Hardline);
            }
            parts.push(Doc::text(line.to_string()));
        }
        parts.push(Doc::Hardline);
    }

    parts.push(Doc::text(fence.to_string()));

    Doc::concat(parts)
}

/// Strip common leading whitespace from code block content.
/// Handles residual whitespace from tab expansion in comrak.
fn strip_common_leading_whitespace(s: &str) -> String {
    let lines: Vec<&str> = s.split('\n').collect();
    // Find minimum indent across non-blank lines (excluding trailing empty line)
    let min_indent = lines
        .iter()
        .take(if lines.last() == Some(&"") {
            lines.len() - 1
        } else {
            lines.len()
        })
        .filter(|line| !line.is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    if min_indent == 0 {
        return s.to_string();
    }
    lines
        .iter()
        .map(|line| {
            if line.len() >= min_indent {
                &line[min_indent..]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ─── Blockquotes ──────────────────────────────────────────────────────────

fn blockquote_to_doc<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    options: &PrettierConfig,
    source: &str,
) -> Doc {
    // Build children as Doc IR, then print with reduced width and prefix lines
    let children: Vec<&Node<RefCell<Ast>>> = node.children().collect();
    let mut child_docs = render_block_sequence(&children, options, source, true);
    // Add trailing hardline so printer outputs trailing newline
    child_docs.push(Doc::Hardline);
    let content_doc = Doc::concat(child_docs);

    // Print content with reduced width (accounting for "> " prefix)
    let bq_options = PrettierConfig {
        print_width: options.print_width.saturating_sub(2),
        ..options.clone()
    };
    let printed = wadler_lindig::print(&content_doc, &bq_options);

    // Prefix each line with "> " or ">"
    let trimmed = printed.trim_end_matches('\n');
    let lines: Vec<&str> = trimmed.split('\n').collect();
    let mut parts = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            parts.push(Doc::Hardline);
        }
        if line.is_empty() {
            parts.push(Doc::text(">"));
        } else {
            parts.push(Doc::text(format!("> {line}")));
        }
    }

    Doc::concat(parts)
}

// ─── Lists ────────────────────────────────────────────────────────────────

#[allow(
    clippy::too_many_arguments,
    clippy::cognitive_complexity,
    clippy::too_many_lines
)]
fn list_to_doc<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    list: &NodeList,
    options: &PrettierConfig,
    source: &str,
    bullet_override: Option<char>,
    followed_by_code_indent: Option<usize>,
) -> Doc {
    let items: Vec<&Node<RefCell<Ast>>> = node.children().collect();
    let is_loose = !list.tight;
    let mut docs = Vec::new();

    let git_diff_friendly = is_git_diff_friendly_ordered_list(list, &items);

    // Determine if this list is "aligned" (uses tabWidth-padded prefixes).
    // Prettier's algorithm: unordered lists are always aligned; ordered lists
    // check whether the source content starts at a tabWidth-aligned column.
    let is_aligned = is_list_aligned(node, list, &items, options);

    // When followed by an unfenced code block, preserve original list padding
    // to avoid changing the code block's interpretation (inside vs outside list).
    let preserve_prefix = followed_by_code_indent.map(|code_indent| {
        // Use max padding across all items (handles renumbered ordered lists
        // where wider markers like "100." create wider padding than "1.")
        let max_padding = items
            .iter()
            .filter_map(|item| {
                let d = item.data.borrow();
                match &d.value {
                    NodeValue::Item(il) => Some(il.padding),
                    _ => None,
                }
            })
            .max()
            .unwrap_or(list.padding);
        // padding = marker_len + spaces_after_marker (excludes marker_offset)
        // So with marker_offset=0, content starts at column `max_padding`
        let base_content_col = max_padding;
        // If stripping marker_offset keeps code outside, use marker_offset=0
        // Otherwise, compute minimum marker_offset needed
        let new_marker_offset = if base_content_col > code_indent {
            0
        } else {
            code_indent + 1 - base_content_col
        };
        // Content column = marker_offset + max_padding
        let content_col = new_marker_offset + base_content_col;
        (new_marker_offset, content_col)
    });

    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            docs.push(Doc::Hardline);
            if is_loose {
                docs.push(Doc::Hardline);
            }
        }

        let marker = match list.list_type {
            ListType::Bullet => String::from(bullet_override.unwrap_or('-')),
            ListType::Ordered => {
                let num = if git_diff_friendly {
                    if i == 0 { list.start } else { 1 }
                } else {
                    list.start + i
                };
                let delim = match list.delimiter {
                    ListDelimType::Period => ".",
                    ListDelimType::Paren => ")",
                };
                format!("{num}{delim}")
            }
        };

        // Check for task list checkbox
        let checkbox_prefix = extract_task_list_marker(item);

        // Check if item has content
        let has_content = item.children().next().is_some();

        if !has_content {
            docs.push(Doc::text(marker));
            continue;
        }

        let (marker_prefix, marker_prefix_len) = if let Some((mo, content_col)) = preserve_prefix {
            // Preserve original padding: marker_offset spaces + marker + spaces to reach content_col
            let spaces_after = content_col.saturating_sub(mo + marker.len()).max(1);
            let prefix = format!("{}{}{}", " ".repeat(mo), marker, " ".repeat(spaces_after));
            let len = prefix.len();
            (prefix, len)
        } else {
            // Normal prefix computation
            let raw_prefix = format!("{marker} ");
            let prefix = if is_aligned && list.list_type == ListType::Ordered {
                align_list_prefix(&raw_prefix, options.tab_width)
            } else {
                raw_prefix
            };
            let len = prefix.len();
            (prefix, len)
        };

        let full_first_line_prefix = if let Some(cb) = &checkbox_prefix {
            format!("{marker_prefix}{cb} ")
        } else {
            marker_prefix
        };

        // First paragraph: align at full prefix width (marker + alignment + checkbox)
        let first_align = full_first_line_prefix.len();

        // Subsequent blocks: prefix_len + clamp(tabWidth - prefix_len, 0, 3)
        let extra_align = clamp_usize(options.tab_width.saturating_sub(marker_prefix_len), 0, 3);
        let nest_indent = marker_prefix_len + extra_align;

        let children: Vec<&Node<RefCell<Ast>>> = item.children().collect();

        let mut item_parts = Vec::new();
        item_parts.push(Doc::text(full_first_line_prefix));

        // First child gets full prefix alignment
        let first_child_doc = block_to_doc(children[0], options, source);
        item_parts.push(Doc::align(first_align, first_child_doc));

        // Subsequent children get tabWidth-based indentation
        // Use per-child alignment: unfenced code blocks use marker_prefix_len,
        // other content uses nest_indent (includes tabWidth extra alignment).
        #[allow(clippy::needless_range_loop)]
        for j in 1..children.len() {
            let child = children[j];
            let child_is_list = {
                let d = child.data.borrow();
                matches!(&d.value, NodeValue::List(_))
            };
            let child_is_unfenced_code = {
                let d = child.data.borrow();
                matches!(&d.value, NodeValue::CodeBlock(cb) if !cb.fenced)
            };
            let prev_is_code_block = {
                let d = children[j - 1].data.borrow();
                matches!(&d.value, NodeValue::CodeBlock(_))
            };
            // Check if previous child was a list followed by this unfenced code block
            let prev_was_list_before_code = j > 1 && {
                let prev_is_list = {
                    let d = children[j - 1].data.borrow();
                    matches!(&d.value, NodeValue::List(_))
                };
                prev_is_list && child_is_unfenced_code
            };

            let mut child_parts = Vec::new();
            // Always add at least one Hardline to end the previous line
            child_parts.push(Doc::Hardline);
            // Add a blank line between item children in loose lists.
            // Exception: suppress blank line before sublists — the
            // sublist handles its own loose/tight spacing. A blank line
            // IS kept when a sublist follows a code block (fenced or
            // indented), as code blocks are visually distinct.
            if is_loose && (!child_is_list || prev_is_code_block) {
                child_parts.push(Doc::Hardline);
            }
            // Extra blank line between list and following unfenced code block
            if prev_was_list_before_code {
                child_parts.push(Doc::Hardline);
            }

            // Check if this child is a list followed by an unfenced code block
            let child_code_indent = if child_is_list && j + 1 < children.len() {
                let next_child = children[j + 1];
                let nd = next_child.data.borrow();
                if let NodeValue::CodeBlock(cb) = &nd.value {
                    if cb.fenced {
                        None
                    } else {
                        let extra_spaces = cb.literal.bytes().take_while(|&b| b == b' ').count();
                        Some(4 + extra_spaces)
                    }
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(code_indent) = child_code_indent {
                // Render nested list with preserved padding
                let cd = child.data.borrow();
                let is_bullet =
                    matches!(&cd.value, NodeValue::List(l) if l.list_type == ListType::Bullet);
                drop(cd);
                let bullet = if is_bullet { Some('-') } else { None };
                child_parts.push(block_to_doc_with_options(
                    child,
                    options,
                    source,
                    bullet,
                    Some(code_indent),
                ));
            } else {
                child_parts.push(block_to_doc(child, options, source));
            }

            // Use marker_prefix_len for unfenced code blocks (no extra tabWidth padding),
            // nest_indent for everything else
            let align_val = if child_is_unfenced_code {
                marker_prefix_len
            } else {
                nest_indent
            };
            item_parts.push(Doc::align(align_val, Doc::concat(child_parts)));
        }

        docs.push(Doc::concat(item_parts));
    }

    Doc::concat(docs)
}

/// Pad list prefix to the nearest `tab_width` boundary (prettier's `alignListPrefix`).
///
/// Adds spaces so the prefix length rounds up to a `tab_width` multiple.
/// If more than 3 extra spaces would be needed, adds nothing (to avoid
/// triggering markdown's 4-space indented code block rule).
fn align_list_prefix(raw_prefix: &str, tab_width: usize) -> String {
    if tab_width == 0 {
        return raw_prefix.to_string();
    }
    let rest = raw_prefix.len() % tab_width;
    let additional = if rest == 0 { 0 } else { tab_width - rest };
    if additional >= 4 {
        raw_prefix.to_string()
    } else {
        format!("{raw_prefix}{}", " ".repeat(additional))
    }
}

/// Determine if a list is "aligned" (should use tabWidth-padded prefixes).
///
/// Prettier's algorithm:
/// - Unordered lists: always aligned
/// - Ordered lists: check source content column alignment and spacing
/// - If parent ordered list was NOT aligned, children inherit non-aligned status
fn is_list_aligned<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    list: &NodeList,
    items: &[&'a Node<'a, RefCell<Ast>>],
    _options: &PrettierConfig,
) -> bool {
    // Unordered lists are always aligned
    if list.list_type != ListType::Ordered {
        return true;
    }

    // If the nearest ancestor ordered list is not aligned, inherit that status
    if ancestor_ordered_list_is_not_aligned(node) {
        return false;
    }

    check_list_self_alignment(list, items)
}

/// Check if any ancestor ordered list uses non-aligned formatting.
fn ancestor_ordered_list_is_not_aligned<'a>(node: &'a Node<'a, RefCell<Ast>>) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        let is_ordered = {
            let data = parent.data.borrow();
            matches!(&data.value, NodeValue::List(l) if l.list_type == ListType::Ordered)
        };
        if is_ordered {
            return !is_node_list_self_aligned(parent);
        }
        current = parent.parent();
    }
    false
}

/// Check if a list node's own items use extra spacing (aligned formatting).
fn is_node_list_self_aligned<'a>(list_node: &'a Node<'a, RefCell<Ast>>) -> bool {
    let data = list_node.data.borrow();
    let NodeValue::List(list) = &data.value else {
        return false;
    };
    let items: Vec<&Node<RefCell<Ast>>> = list_node.children().collect();
    if items.is_empty() {
        return false;
    }
    let first_leading = item_leading_spaces(items[0], list);
    if first_leading > 1 {
        return true;
    }
    if items.len() > 1 {
        let second_leading = item_leading_spaces(items[1], list);
        if second_leading > 1 {
            return true;
        }
    }
    false
}

/// Check if a list's own items use extra spacing (aligned formatting).
fn check_list_self_alignment<'a>(list: &NodeList, items: &[&'a Node<'a, RefCell<Ast>>]) -> bool {
    if items.is_empty() {
        return false;
    }

    let first_leading = item_leading_spaces(items[0], list);
    if first_leading > 1 {
        return true;
    }
    if items.len() > 1 {
        let second_leading = item_leading_spaces(items[1], list);
        if second_leading > 1 {
            return true;
        }
    }
    false
}

/// Get the number of spaces between the marker end and content start.
/// For task list items (e.g., `1. [ ] text`), excludes the checkbox width.
fn item_leading_spaces<'a>(item: &'a Node<'a, RefCell<Ast>>, list: &NodeList) -> usize {
    let item_col = item.data.borrow().sourcepos.start.column;
    let content_col = item
        .children()
        .next()
        .map_or(0, |child| child.data.borrow().sourcepos.start.column);
    let source_prefix = content_col.saturating_sub(item_col);
    let marker_len = match list.list_type {
        ListType::Bullet => 1,
        ListType::Ordered => {
            let data = item.data.borrow();
            if let NodeValue::Item(il) = &data.value {
                // digits + "." or ")"
                digit_count(il.start) + 1
            } else {
                // TaskItem doesn't store the original number; use list.start
                // as a close approximation (same digit count for most cases)
                digit_count(list.start) + 1
            }
        }
    };
    // Subtract checkbox width for task items: "[ ] " or "[x] " = 4 chars
    let checkbox_len = {
        let data = item.data.borrow();
        if matches!(&data.value, NodeValue::TaskItem(_)) {
            4
        } else {
            0
        }
    };
    source_prefix
        .saturating_sub(marker_len)
        .saturating_sub(checkbox_len)
}

fn digit_count(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut count = 0;
    let mut val = n;
    while val > 0 {
        count += 1;
        val /= 10;
    }
    count
}

fn clamp_usize(value: usize, min: usize, max: usize) -> usize {
    value.max(min).min(max)
}

/// Check if an ordered list uses git-diff-friendly numbering (all 1s after first).
fn is_git_diff_friendly_ordered_list(list: &NodeList, items: &[&Node<RefCell<Ast>>]) -> bool {
    if list.list_type != ListType::Ordered || items.len() < 2 {
        return false;
    }

    let item_start = |idx: usize| -> usize {
        let data = items[idx].data.borrow();
        match &data.value {
            NodeValue::Item(il) => il.start,
            // TaskItem doesn't store the original number. We check the list's
            // padding to infer: if the list starts at 1 and padding matches a
            // single-digit marker, assume all items use "1." (git-diff-friendly).
            NodeValue::TaskItem(_) => {
                // If list starts at 1 and this is idx > 0, comrak doesn't tell us
                // the original number. Default to 1 (common in git-diff-friendly lists).
                if list.start == 1 { 1 } else { list.start + idx }
            }
            _ => 0,
        }
    };

    let second = item_start(1);
    if second != 1 {
        return false;
    }

    let first = item_start(0);
    if first != 0 {
        return true;
    }

    // First is 0, check if third item is also 1
    items.len() > 2 && item_start(2) == 1
}

/// Extract task list checkbox marker if present.
fn extract_task_list_marker<'a>(item: &'a Node<'a, RefCell<Ast>>) -> Option<String> {
    let data = item.data.borrow();
    match &data.value {
        NodeValue::TaskItem(Some(c)) => {
            let symbol = if *c == 0 as char { " " } else { "x" };
            Some(format!("[{symbol}]"))
        }
        NodeValue::TaskItem(None) => Some("[ ]".to_string()),
        _ => None,
    }
}

/// Render block-level children as Doc IR (preserving printer wrapping).
fn render_block_children<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    options: &PrettierConfig,
    is_loose: bool,
    source: &str,
) -> Doc {
    let children: Vec<&Node<RefCell<Ast>>> = node.children().collect();
    let docs = render_block_sequence(&children, options, source, is_loose);
    Doc::concat(docs)
}

// ─── HTML blocks ──────────────────────────────────────────────────────────

fn html_block_to_doc(html: &NodeHtmlBlock) -> Doc {
    let literal = html.literal.trim_end_matches('\n');
    let lines: Vec<&str> = literal.split('\n').collect();
    let mut parts = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            parts.push(Doc::Hardline);
        }
        parts.push(Doc::text(line.to_string()));
    }
    Doc::concat(parts)
}

// ─── Tables (basic) ──────────────────────────────────────────────────────

fn table_to_doc<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    source: &str,
    options: &PrettierConfig,
) -> Doc {
    // Collect rows (header + body)
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut alignments: Vec<comrak::nodes::TableAlignment> = Vec::new();

    for child in node.children() {
        let row_data = child.data.borrow();
        if let NodeValue::TableRow(header) = &row_data.value {
            let mut cells = Vec::new();
            for cell in child.children() {
                let cell_data = cell.data.borrow();
                if let NodeValue::TableCell = &cell_data.value {
                    // Extract cell content from source to preserve escapes
                    let sp = cell_data.sourcepos;
                    let content = extract_source_range(source, &sp);
                    cells.push(content.trim().to_string());
                }
            }
            if *header && alignments.is_empty() {
                // Get alignments from the table node
                let table_data = node.data.borrow();
                if let NodeValue::Table(table) = &table_data.value {
                    alignments.clone_from(&table.alignments);
                }
            }
            rows.push(cells);
        }
    }

    if rows.is_empty() {
        return Doc::text("");
    }

    // Calculate column widths using Unicode display width
    let num_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut col_widths = vec![3usize; num_cols]; // minimum 3 for ---
    for row in &rows {
        for (j, cell) in row.iter().enumerate() {
            if j < num_cols {
                col_widths[j] = col_widths[j].max(cell_display_width(cell));
            }
        }
    }

    let mut parts = Vec::new();

    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            parts.push(Doc::Hardline);
        }

        // Build row — trim trailing empty cells for body rows
        let effective_cols = if i == 0 {
            num_cols
        } else {
            // Find last non-empty cell
            let last_nonempty = row.iter().rposition(|c| !c.is_empty()).map_or(0, |p| p + 1);
            last_nonempty.max(1).min(num_cols)
        };
        let mut row_str = String::from("|");
        #[allow(clippy::needless_range_loop)]
        for j in 0..effective_cols {
            let cell = row.get(j).map_or("", String::as_str);
            let width = col_widths[j];
            let align = alignments
                .get(j)
                .copied()
                .unwrap_or(comrak::nodes::TableAlignment::None);
            let padded = pad_cell(cell, width, align);
            let _ = write!(row_str, " {padded} |");
        }
        parts.push(Doc::text(row_str));

        // After header row, insert separator
        if i == 0 {
            parts.push(Doc::Hardline);
            // Calculate full table width to decide on compact separators
            let full_table_width: usize = col_widths.iter().sum::<usize>()
                + 1 // leading |
                + col_widths.len() * 3; // " " + " |" per column
            // Use compact separators when proseWrap=never and table exceeds printWidth
            let compact_seps =
                options.prose_wrap == ProseWrap::Never && full_table_width > options.print_width;
            let mut sep = String::from("|");
            for (j, width) in col_widths.iter().enumerate() {
                let align = alignments
                    .get(j)
                    .copied()
                    .unwrap_or(comrak::nodes::TableAlignment::None);
                let sep_width = if compact_seps { 3 } else { *width };
                let dashes = separator_cell(sep_width, align);
                let _ = write!(sep, " {dashes} |");
            }
            parts.push(Doc::text(sep));
        }
    }

    Doc::concat(parts)
}

fn pad_cell(content: &str, width: usize, align: comrak::nodes::TableAlignment) -> String {
    let padding = width.saturating_sub(cell_display_width(content));
    match align {
        comrak::nodes::TableAlignment::Center => {
            let left = padding / 2;
            let right = padding - left;
            format!("{}{}{}", " ".repeat(left), content, " ".repeat(right))
        }
        comrak::nodes::TableAlignment::Right => {
            format!("{}{}", " ".repeat(padding), content)
        }
        _ => {
            format!("{}{}", content, " ".repeat(padding))
        }
    }
}

fn separator_cell(width: usize, align: comrak::nodes::TableAlignment) -> String {
    match align {
        comrak::nodes::TableAlignment::Left => format!(":{}", "-".repeat(width - 1)),
        comrak::nodes::TableAlignment::Right => format!("{}:", "-".repeat(width - 1)),
        comrak::nodes::TableAlignment::Center => {
            format!(":{}:", "-".repeat(width.saturating_sub(2)))
        }
        comrak::nodes::TableAlignment::None => "-".repeat(width),
    }
}

/// Flatten a Doc tree into a plain string (for text content extraction).
fn flatten_doc_to_string(doc: &Doc, out: &mut String) {
    match doc {
        Doc::Text(s) => out.push_str(s),
        Doc::Concat(docs) => {
            for d in docs {
                flatten_doc_to_string(d, out);
            }
        }
        Doc::Line | Doc::Softline => out.push(' '),
        Doc::Hardline => out.push('\n'),
        Doc::Group(inner) | Doc::Indent(inner) | Doc::Align(_, inner) => {
            flatten_doc_to_string(inner, out);
        }
        Doc::IfBreak { flat, .. } => flatten_doc_to_string(flat, out),
        Doc::Fill(parts) => {
            for d in parts {
                flatten_doc_to_string(d, out);
            }
        }
        Doc::BreakParent => {}
    }
}

/// Extract source text for a given sourcepos range.
fn extract_source_range(source: &str, sp: &comrak::nodes::Sourcepos) -> String {
    let lines: Vec<&str> = source.lines().collect();
    if sp.start.line == 0 || sp.start.line > lines.len() || sp.end.line == 0 || sp.start.column == 0
    {
        return String::new();
    }
    if sp.start.line == sp.end.line {
        let line = lines[sp.start.line - 1];
        let start = (sp.start.column - 1).min(line.len());
        let end = sp.end.column.min(line.len());
        if start >= end {
            return String::new();
        }
        return line[start..end].to_string();
    }
    // Multi-line cell (rare for tables but handle it)
    let mut result = String::new();
    for line_idx in sp.start.line..=sp.end.line {
        if line_idx > lines.len() {
            break;
        }
        let line = lines[line_idx - 1];
        if line_idx == sp.start.line {
            let start = (sp.start.column - 1).min(line.len());
            result.push_str(&line[start..]);
        } else if line_idx == sp.end.line {
            let end = sp.end.column.min(line.len());
            result.push_str(&line[..end]);
        } else {
            result.push_str(line);
        }
        if line_idx < sp.end.line {
            result.push('\n');
        }
    }
    result
}

/// Calculate the display width of a table cell, accounting for Unicode width
/// (CJK characters = 2 columns). Uses raw character widths since cell content
/// is extracted from source text and escapes take up display space.
fn cell_display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

// ─── Inline elements ─────────────────────────────────────────────────────

fn collect_inline_children<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    options: &PrettierConfig,
    source: &str,
) -> Vec<Doc> {
    let mut docs = Vec::new();
    for child in node.children() {
        docs.push(inline_to_doc(child, options, source));
    }
    docs
}

fn inline_to_doc<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    options: &PrettierConfig,
    source: &str,
) -> Doc {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Text(text) => Doc::text(text.clone()),
        NodeValue::SoftBreak => {
            drop(data);
            match options.prose_wrap {
                ProseWrap::Always => Doc::Line,
                ProseWrap::Never => {
                    // Between two CJK characters, use no space
                    let prev_cjk = node
                        .previous_sibling()
                        .and_then(|s| trailing_char_of_node(s))
                        .is_some_and(is_cjk_char);
                    let next_cjk = node
                        .next_sibling()
                        .and_then(|s| leading_char_of_node(s))
                        .is_some_and(is_cjk_char);
                    if prev_cjk && next_cjk {
                        Doc::text("")
                    } else {
                        Doc::text(" ")
                    }
                }
                ProseWrap::Preserve => Doc::Hardline,
            }
        }
        NodeValue::LineBreak => {
            // Hard break — preserve original style (backslash or trailing spaces)
            let sourcepos = data.sourcepos;
            if detect_hard_break_style(source, &sourcepos) {
                Doc::concat(vec![Doc::text("\\"), Doc::Hardline])
            } else {
                Doc::concat(vec![Doc::text("  "), Doc::Hardline])
            }
        }
        NodeValue::Code(NodeCode {
            literal,
            num_backticks,
            ..
        }) => {
            let ticks = "`".repeat(*num_backticks);
            if literal.starts_with('`')
                || literal.ends_with('`')
                || (literal.starts_with(' ') && literal.ends_with(' ') && literal.len() > 1)
            {
                Doc::text(format!("{ticks} {literal} {ticks}"))
            } else {
                Doc::text(format!("{ticks}{literal}{ticks}"))
            }
        }
        NodeValue::Emph => {
            let inner = collect_inline_children(node, options, source);
            // Use * when adjacent to word characters (digits, etc.) where _ wouldn't work
            let delim = emphasis_delimiter(node, "_", "*");
            Doc::concat(vec![
                Doc::text(delim.clone()),
                Doc::concat(inner),
                Doc::text(delim),
            ])
        }
        NodeValue::Strong => {
            let inner = collect_inline_children(node, options, source);
            Doc::concat(vec![Doc::text("**"), Doc::concat(inner), Doc::text("**")])
        }
        NodeValue::Link(link) => link_to_doc(node, link, options, source),
        NodeValue::Image(link) => image_to_doc(node, link, options, source),
        NodeValue::HtmlInline(html) => Doc::text(html.clone()),
        NodeValue::Strikethrough => {
            let inner = collect_inline_children(node, options, source);
            Doc::concat(vec![Doc::text("~~"), Doc::concat(inner), Doc::text("~~")])
        }
        _ => {
            let inner_docs = collect_inline_children(node, options, source);
            Doc::concat(inner_docs)
        }
    }
}

fn link_to_doc<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    link: &NodeLink,
    options: &PrettierConfig,
    source: &str,
) -> Doc {
    let inner = collect_inline_children(node, options, source);
    let inner_text = {
        let mut s = String::new();
        for d in &inner {
            flatten_doc_to_string(d, &mut s);
        }
        s
    };

    // Check if this is an autolink (URL text matches URL)
    if inner_text == link.url
        || (link.url.starts_with("mailto:") && inner_text == link.url["mailto:".len()..])
    {
        if link.url.starts_with("mailto:") {
            return Doc::text(format!("<{inner_text}>"));
        }
        // Check if the source used angle bracket syntax <url>
        // Comrak's sourcepos for the Link node starts INSIDE the angle brackets,
        // so the `<` is at column - 2 (1-indexed column to 0-indexed, minus 1 for `<`).
        let sourcepos = node.data.borrow().sourcepos;
        let is_angle_bracket = sourcepos.start.line > 0
            && sourcepos.start.column >= 2
            && source
                .lines()
                .nth(sourcepos.start.line - 1)
                .is_some_and(|line| {
                    let col = sourcepos.start.column - 2;
                    line.as_bytes().get(col) == Some(&b'<')
                });
        if is_angle_bracket {
            return Doc::text(format!("<{}>", link.url));
        }
        return Doc::text(link.url.clone());
    }

    // Check if URL contains special characters that need angle brackets
    let needs_angle_brackets =
        link.url.contains(' ') || (link.url.contains('(') && link.url.contains(')'));

    let url_part = if needs_angle_brackets {
        // Encode > as %3E inside angle brackets to prevent premature closing
        format!("<{}>", link.url.replace('>', "%3E"))
    } else {
        link.url.clone()
    };

    let mut parts = vec![
        Doc::text("["),
        Doc::concat(inner),
        Doc::text("]("),
        Doc::text(url_part),
    ];
    if !link.title.is_empty() {
        let quote = pick_title_quote(&link.title, options);
        let escaped = escape_title(&link.title, quote);
        parts.push(Doc::text(format!(" {quote}{escaped}{quote}")));
    }
    parts.push(Doc::text(")"));
    Doc::concat(parts)
}

/// Pick the quote character for a link/image title.
/// Uses `singleQuote` option, but falls back if the title contains the chosen quote.
fn pick_title_quote(title: &str, options: &PrettierConfig) -> &'static str {
    let preferred = if options.single_quote { "'" } else { "\"" };
    let other = if options.single_quote { "\"" } else { "'" };

    if !title.contains(preferred) {
        preferred
    } else if !title.contains(other) {
        other
    } else {
        // Both quotes present — use preferred and it'll be handled by the renderer
        preferred
    }
}

/// Escape a link/image title for output.
/// Backslashes and the chosen quote character need to be escaped.
fn escape_title(title: &str, quote: &str) -> String {
    let quote_char = quote.chars().next().unwrap_or('"');
    let mut result = String::with_capacity(title.len());
    for c in title.chars() {
        if c == '\\' || c == quote_char {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

fn image_to_doc<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    link: &NodeLink,
    options: &PrettierConfig,
    source: &str,
) -> Doc {
    let inner = collect_inline_children(node, options, source);

    // Check if URL needs angle brackets (contains spaces or unbalanced parens)
    let needs_angle_brackets =
        link.url.contains(' ') || (link.url.contains('(') && link.url.contains(')'));

    let url_part = if needs_angle_brackets {
        format!("<{}>", link.url.replace('>', "%3E"))
    } else {
        link.url.clone()
    };

    let mut parts = vec![
        Doc::text("!["),
        Doc::concat(inner),
        Doc::text("]("),
        Doc::text(url_part),
    ];
    if !link.title.is_empty() {
        let quote = pick_title_quote(&link.title, options);
        let escaped = escape_title(&link.title, quote);
        parts.push(Doc::text(format!(" {quote}{escaped}{quote}")));
    }
    parts.push(Doc::text(")"));
    Doc::concat(parts)
}

// ─── Emphasis delimiter selection ─────────────────────────────────────

/// Choose `_` or `*` for emphasis based on surrounding context.
/// In `CommonMark`, `_` emphasis requires word boundaries. Use `*` when
/// adjacent to word characters (digits, letters) that would prevent
/// `_` from being recognized as emphasis.
fn emphasis_delimiter<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    underscore: &str,
    star: &str,
) -> String {
    // Check previous sibling's trailing text
    if let Some(prev) = node.previous_sibling() {
        let prev_text = trailing_char_of_node(prev);
        if let Some(c) = prev_text
            && (c.is_alphanumeric() || c == '_')
        {
            return star.to_string();
        }
    }
    // Check next sibling's leading text
    if let Some(next) = node.next_sibling() {
        let next_text = leading_char_of_node(next);
        if let Some(c) = next_text
            && (c.is_alphanumeric() || c == '_')
        {
            return star.to_string();
        }
    }
    underscore.to_string()
}

fn trailing_char_of_node<'a>(node: &'a Node<'a, RefCell<Ast>>) -> Option<char> {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Text(s) => s.chars().last(),
        NodeValue::Code(c) => c.literal.chars().last().or(Some('`')),
        _ => None,
    }
}

fn leading_char_of_node<'a>(node: &'a Node<'a, RefCell<Ast>>) -> Option<char> {
    let data = node.data.borrow();
    match &data.value {
        NodeValue::Text(s) => s.chars().next(),
        NodeValue::Code(c) => c.literal.chars().next().or(Some('`')),
        _ => None,
    }
}

// ─── CJK handling ─────────────────────────────────────────────────────────

/// Check if a character is CJK (Chinese/Japanese/Korean ideograph, kana, or
/// CJK punctuation). Used to determine if a soft break between two characters
/// should insert a space or not — CJK-to-CJK boundaries need no space.
fn is_cjk_char(c: char) -> bool {
    let cp = c as u32;
    matches!(cp,
        // CJK Radicals Supplement, Kangxi Radicals, CJK Symbols, Hiragana, Katakana, etc.
        0x2E80..=0x33FF |
        // CJK Unified Ideographs Extension A, Yijing, CJK Unified Ideographs
        0x3400..=0x9FFF |
        // CJK Compatibility Ideographs
        0xF900..=0xFAFF |
        // CJK Compatibility Forms
        0xFE30..=0xFE4F |
        // Halfwidth/Fullwidth Forms
        0xFF01..=0xFF60 | 0xFFE0..=0xFFEF |
        // CJK Unified Ideographs Extension B-H, CJK Compatibility Supplement
        0x20000..=0x323AF
    )
}

/// Get the last character from a Doc's text content.
fn last_text_of(doc: &Doc) -> Option<&str> {
    match doc {
        Doc::Text(s) => Some(s.as_str()),
        Doc::Concat(parts) => parts.iter().rev().find_map(last_text_of),
        _ => None,
    }
}

/// Replace `Line` separators with `Softline` when both adjacent parts are CJK.
/// In Fill, `Softline` prints as "" when flat (no space between CJK chars) and
/// as newline when the line needs to break.
fn replace_cjk_line_separators(parts: Vec<Doc>) -> Vec<Doc> {
    if parts.len() < 3 {
        return parts;
    }

    let mut result = Vec::with_capacity(parts.len());
    let mut iter = parts.into_iter().peekable();

    // First element is always content
    if let Some(first) = iter.next() {
        result.push(first);
    }

    while let Some(sep) = iter.next() {
        if let Some(content) = iter.next() {
            let should_softline = matches!(&sep, Doc::Line)
                && last_text_of(result.last().unwrap_or(&Doc::text("")))
                    .and_then(|s| s.chars().last())
                    .is_some_and(is_cjk_char)
                && first_text_of(&content)
                    .and_then(|s| s.chars().next())
                    .is_some_and(is_cjk_char);

            if should_softline {
                result.push(Doc::Softline);
            } else {
                result.push(sep);
            }
            result.push(content);
        } else {
            result.push(sep);
        }
    }

    result
}

// ─── Helpers ──────────────────────────────────────────────────────────────
