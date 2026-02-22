use saphyr_parser::ScalarStyle;

use crate::ast::ScalarNode;
use prettier_config::{PrettierConfig, ProseWrap};

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
/// based on `indent` (column position). Uses raw source to preserve trailing
/// blank lines (saphyr doesn't always include them in the parsed value).
#[allow(clippy::too_many_lines)]
pub(crate) fn format_block_scalar(
    s: &ScalarNode,
    output: &mut String,
    indent: usize,
    options: &PrettierConfig,
) {
    let tw = options.tab_width;
    let Some(block_src) = &s.block_source else {
        // Fallback: reconstruct from value
        let indicator = if s.style == ScalarStyle::Literal {
            '|'
        } else {
            '>'
        };
        output.push(indicator);
        output.push('\n');
        let body_indent = " ".repeat(indent.max(tw));
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

    // For folded (>) scalars with proseWrap:always/never, re-wrap content
    // even when there's an explicit indent indicator
    let needs_rewrap = s.style == ScalarStyle::Folded
        && matches!(options.prose_wrap, ProseWrap::Always | ProseWrap::Never);

    if has_explicit_indent && !needs_rewrap {
        // With explicit indent, preserve body lines as-is from source
        for line in &body_lines {
            output.push_str(line);
            output.push('\n');
        }
    } else {
        let base_indent = if has_explicit_indent {
            // Explicit indent indicator gives us the exact indent value
            header
                .chars()
                .skip(1)
                .find(char::is_ascii_digit)
                .map_or(0, |c| (c as usize) - ('0' as usize))
        } else {
            // Re-indent: detect base indent from source
            body_lines
                .iter()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.len() - l.trim_start().len())
                .min()
                .unwrap_or(0)
        };

        let target_indent = if has_explicit_indent {
            // With explicit indent, keep original indent width
            " ".repeat(base_indent)
        } else {
            " ".repeat(indent.max(tw))
        };

        if needs_rewrap {
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

/// Split a string on single spaces only, preserving runs of multiple spaces.
///
/// Prettier's `splitWithSingleSpace` splits on `(?<!^| ) (?! |$)` — a space that
/// is NOT preceded by start-of-string/space and NOT followed by space/end-of-string.
/// So `"123   456   789"` is one chunk (multi-space runs preserved),
/// while `"123 456 789"` splits into `["123", "456", "789"]`.
fn split_with_single_space(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return vec![s];
    }

    let mut parts = Vec::new();
    let mut start = 0;

    let mut i = 0;
    while i < len {
        if bytes[i] == b' ' {
            // Check if this is a single space:
            // - NOT preceded by space (or at start of string)
            // - NOT followed by space (or at end of string)
            let prev_is_space = i == 0 || bytes[i - 1] == b' ';
            let next_is_space = i + 1 >= len || bytes[i + 1] == b' ';

            if !prev_is_space && !next_is_space {
                // Single space — split here
                parts.push(&s[start..i]);
                start = i + 1;
            }
        }
        i += 1;
    }
    parts.push(&s[start..]);
    parts
}

/// Re-wrap folded block scalar content for Always/Never prose wrap modes.
#[allow(clippy::too_many_arguments)]
fn format_block_folded_rewrap(
    body_lines: &[&str],
    output: &mut String,
    base_indent: usize,
    target_indent: &str,
    options: &PrettierConfig,
) {
    let mut i = 0;
    while i < body_lines.len() {
        let line = body_lines[i];
        // Use trim_start() not trim() — trailing whitespace must not affect indent calculation.
        // "  foo " has indent 2, not 3. trim() would give "foo" (len 3) making indent 6-3=3.
        let trimmed = line.trim_start();

        if trimmed.is_empty() {
            // Blank line: preserve original whitespace characters (may include tabs)
            if !line.is_empty() && line.len() > base_indent {
                output.push_str(target_indent);
                output.push_str(&line[base_indent..]);
            }
            output.push('\n');
            i += 1;
        } else {
            let line_indent = line.len() - trimmed.len();
            let extra = line_indent.saturating_sub(base_indent);

            if extra > 0 {
                // More-indented line: preserve with re-indent, keeping original
                // whitespace characters (tabs, spaces) beyond base indent
                output.push_str(target_indent);
                output.push_str(&line[base_indent..line_indent]);
                output.push_str(trimmed);
                output.push('\n');
                i += 1;
            } else {
                // Regular content: collect consecutive regular lines, fold into paragraph
                // Use splitWithSingleSpace to preserve runs of multiple spaces
                let mut words: Vec<&str> = Vec::new();
                while i < body_lines.len() {
                    let l = body_lines[i];
                    let t = l.trim_start();
                    if t.is_empty() {
                        break;
                    }
                    let li = l.len() - t.len();
                    let ex = li.saturating_sub(base_indent);
                    if ex > 0 {
                        break;
                    }
                    words.extend(split_with_single_space(t));
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
