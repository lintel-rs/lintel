use core::fmt::Write;

use crate::fmt::Fmt;

/// Write a yellow section header.
pub(crate) fn write_section(out: &mut String, label: &str, f: &Fmt<'_>) {
    let _ = writeln!(out, "{}{label}{}", f.yellow, f.reset);
}

/// Write a metadata label with a pre-formatted value.
///
/// The label is plain text; the caller pre-formats the value with color.
pub(crate) fn write_label(out: &mut String, indent: &str, label: &str, value: &str) {
    let _ = writeln!(out, "{indent}{label}: {value}");
}

/// Write a multi-line description to the output buffer.
///
/// When color is enabled, markdown is rendered to ANSI with syntax-highlighted
/// code blocks sized to fit within the available width minus the indent.
/// When color is off, raw markdown text is written with indentation.
pub(crate) fn write_description(out: &mut String, text: &str, f: &Fmt<'_>, indent: &str) {
    let rendered = if f.is_color() {
        let available = f.width.saturating_sub(indent.len());
        markdown_to_ansi::render(text, &f.md_opts(Some(available)))
    } else {
        text.to_string()
    };
    // Trim trailing newlines so callers can rely on `out.push('\n')` as the
    // sole section separator — without this, markdown rendering's trailing `\n`
    // produces an extra blank line.
    let trimmed = rendered.trim_end_matches('\n');
    for line in trimmed.split('\n') {
        if line.trim().is_empty() {
            out.push('\n');
        } else {
            let _ = writeln!(out, "{indent}{line}");
        }
    }
}

/// Write a metadata label with a value, wrapping if it exceeds the line width.
///
/// Short values stay on one line: `    Default: "es2015"`
/// Long values wrap onto the next line:
/// ```text
///     Default:
///       "First of: `tsconfig.json` ..."
/// ```
///
/// The `value` should be plain text (no ANSI escapes); magenta coloring is
/// applied automatically for the single-line case, and markdown rendering
/// handles formatting in the wrapped case.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_label_wrapped(
    out: &mut String,
    indent: &str,
    label: &str,
    value: &str,
    f: &Fmt<'_>,
) {
    let prefix_len = indent.len() + label.len() + 2; // +2 for ": "
    if prefix_len + value.len() <= f.width {
        let _ = writeln!(out, "{indent}{label}: {}{value}{}", f.magenta, f.reset);
    } else {
        let _ = writeln!(out, "{indent}{label}:");
        let inner_indent = format!("{indent}  ");
        write_description(out, value, f, &inner_indent);
    }
}
