use saphyr_parser::ScalarStyle;

use crate::ProseWrap;
use crate::YamlFormatOptions;
use crate::ast::ScalarNode;

/// Normalize block scalar header indicator order.
/// YAML spec says digit (indentation indicator) comes before chomping indicator.
/// e.g. `|-2` → `|2-`, `>-1` → `>1-`
pub(crate) fn normalize_block_header(header: &str) -> String {
    let mut chars = header.chars();
    let indicator = chars.next().unwrap_or('|'); // | or >
    let rest: String = chars.collect();

    let mut digit = None;
    let mut chomp = None;
    for c in rest.chars() {
        if c.is_ascii_digit() {
            digit = Some(c);
        } else if c == '+' || c == '-' {
            chomp = Some(c);
        }
    }

    let mut result = String::new();
    result.push(indicator);
    if let Some(d) = digit {
        result.push(d);
    }
    if let Some(c) = chomp {
        result.push(c);
    }
    result
}

/// Format a block scalar (literal `|` or folded `>`).
///
/// Re-indents the body from the raw source to use the correct indentation
/// based on depth and `tab_width`. Uses raw source to preserve trailing blank
/// lines (saphyr doesn't always include them in the parsed value).
#[allow(clippy::too_many_lines)]
pub(crate) fn format_block_scalar(
    s: &ScalarNode,
    output: &mut String,
    depth: usize,
    options: &YamlFormatOptions,
) {
    let Some(block_src) = &s.block_source else {
        // Fallback: reconstruct from value
        let indicator = if s.style == ScalarStyle::Literal {
            '|'
        } else {
            '>'
        };
        output.push(indicator);
        output.push('\n');
        let body_indent = " ".repeat(depth.max(1) * options.tab_width);
        for line in s.value.lines() {
            output.push_str(&body_indent);
            output.push_str(line);
            output.push('\n');
        }
        return;
    };

    // Use split('\n') instead of lines() to preserve trailing empty lines
    // (lines() strips the final newline as a terminator, losing trailing blank lines)
    let mut src_lines = block_src.split('\n');

    // First line is the header (|, |+, |-, >+, etc.), possibly with trailing comment
    let header_full = src_lines.next().unwrap_or("|");
    // Split indicator part from trailing comment (e.g. "> # hello" -> ">", "# hello")
    let (header, header_comment) = if let Some(comment_pos) = header_full.find(" #") {
        (
            &header_full[..comment_pos],
            Some(header_full[comment_pos..].trim()),
        )
    } else {
        (header_full, None)
    };
    // Normalize indicator order: digit before chomping (e.g. |-2 -> |2-)
    let normalized_header = normalize_block_header(header);
    output.push_str(&normalized_header);
    if let Some(comment) = header_comment {
        output.push(' ');
        output.push_str(comment);
    }
    output.push('\n');

    // Collect body lines from raw source
    let mut body_lines: Vec<&str> = src_lines.collect();

    // For keep chomping (|+, >+), preserve all trailing blank lines.
    // For clip/strip, remove trailing empty lines from the body (they come from
    // source text but should not appear in formatted output).
    let is_keep = header.contains('+');
    if !is_keep {
        while body_lines.last() == Some(&"") {
            body_lines.pop();
        }
    }

    // Check if header has an explicit indent indicator (a digit)
    // e.g. |2, |1+, >2-, etc. - the digit after the indicator char
    let has_explicit_indent = header
        .chars()
        .skip(1) // skip | or >
        .any(|c| c.is_ascii_digit());

    if has_explicit_indent {
        // With explicit indent, preserve body lines as-is from source
        for line in &body_lines {
            output.push_str(line);
            output.push('\n');
        }
    } else {
        // Re-indent: detect base indent from source and normalize to target
        let base_indent = body_lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);

        let target_indent = " ".repeat(depth.max(1) * options.tab_width);

        // For folded (>) scalars with proseWrap:always/never, re-wrap content
        if s.style == ScalarStyle::Folded
            && matches!(options.prose_wrap, ProseWrap::Always | ProseWrap::Never)
        {
            format_block_folded_rewrap(&body_lines, output, base_indent, &target_indent, options);
            return;
        }

        for line in &body_lines {
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                // Line is empty or whitespace-only.
                // Preserve relative indentation beyond the base for
                // whitespace-only lines (important for |+ keep chomping).
                if !line.is_empty() {
                    let line_indent = line.len();
                    let extra = line_indent.saturating_sub(base_indent);
                    if extra > 0 {
                        output.push_str(&target_indent);
                        // Preserve original extra whitespace chars (may include tabs)
                        output.push_str(&line[base_indent..line_indent]);
                    }
                }
                output.push('\n');
            } else {
                output.push_str(&target_indent);
                // Preserve relative indentation beyond the base, keeping original
                // whitespace characters (tabs, spaces) intact
                let line_indent = line.len() - trimmed.len();
                if line_indent > base_indent {
                    output.push_str(&line[base_indent..line_indent]);
                }
                output.push_str(trimmed);
                output.push('\n');
            }
        }
    }
}

/// Re-wrap folded block scalar content for Always/Never prose wrap modes.
fn format_block_folded_rewrap(
    body_lines: &[&str],
    output: &mut String,
    base_indent: usize,
    target_indent: &str,
    options: &YamlFormatOptions,
) {
    let mut i = 0;
    while i < body_lines.len() {
        let line = body_lines[i];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            // Blank line: preserve (paragraph break or keep-chomping trailing)
            if !line.is_empty() {
                let line_indent = line.len();
                let extra = line_indent.saturating_sub(base_indent);
                if extra > 0 {
                    output.push_str(target_indent);
                    output.push_str(&" ".repeat(extra));
                }
            }
            output.push('\n');
            i += 1;
        } else {
            let line_indent = line.len() - trimmed.len();
            let extra = line_indent.saturating_sub(base_indent);

            if extra > 0 {
                // More-indented line: preserve with re-indent
                output.push_str(target_indent);
                output.push_str(&" ".repeat(extra));
                output.push_str(trimmed);
                output.push('\n');
                i += 1;
            } else {
                // Regular content: collect consecutive regular lines, fold into paragraph
                let mut words = Vec::new();
                while i < body_lines.len() {
                    let l = body_lines[i];
                    let t = l.trim();
                    if t.is_empty() {
                        break;
                    }
                    let li = l.len() - t.len();
                    let ex = li.saturating_sub(base_indent);
                    if ex > 0 {
                        break;
                    }
                    words.extend(t.split_whitespace());
                    i += 1;
                }

                // Output the folded paragraph
                if matches!(options.prose_wrap, ProseWrap::Always) {
                    let mut line_len = target_indent.len();
                    output.push_str(target_indent);
                    let mut first_word = true;
                    for word in &words {
                        if first_word {
                            output.push_str(word);
                            line_len += word.len();
                            first_word = false;
                        } else if line_len + 1 + word.len() > options.print_width {
                            output.push('\n');
                            output.push_str(target_indent);
                            output.push_str(word);
                            line_len = target_indent.len() + word.len();
                        } else {
                            output.push(' ');
                            output.push_str(word);
                            line_len += 1 + word.len();
                        }
                    }
                    output.push('\n');
                } else {
                    // Never: all words on one line
                    output.push_str(target_indent);
                    let joined: Vec<&str> = words.into_iter().collect();
                    output.push_str(&joined.join(" "));
                    output.push('\n');
                }
            }
        }
    }
}
