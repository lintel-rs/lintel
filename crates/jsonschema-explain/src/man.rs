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
    for line in rendered.split('\n') {
        if line.trim().is_empty() {
            out.push('\n');
        } else if line.starts_with("\x1b[48;2;") {
            // Code block line — caller adds indent; background starts at indent
            let _ = writeln!(out, "{indent}{line}");
        } else {
            let _ = writeln!(out, "{indent}{line}");
        }
    }
}
