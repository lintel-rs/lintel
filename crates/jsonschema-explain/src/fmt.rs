pub(crate) const BOLD: &str = "\x1b[1m";
pub(crate) const DIM: &str = "\x1b[2m";
pub(crate) const RESET: &str = "\x1b[0m";
pub(crate) const CYAN: &str = "\x1b[36m";
pub(crate) const GREEN: &str = "\x1b[32m";
pub(crate) const YELLOW: &str = "\x1b[33m";
pub(crate) const MAGENTA: &str = "\x1b[35m";
#[cfg(test)]
pub(crate) const BLUE: &str = "\x1b[34m";
pub(crate) const RED: &str = "\x1b[31m";

/// Formatting context passed through the rendering functions.
pub(crate) struct Fmt<'a> {
    pub bold: &'a str,
    pub dim: &'a str,
    pub reset: &'a str,
    pub cyan: &'a str,    // type annotations
    pub green: &'a str,   // property names
    pub yellow: &'a str,  // section headers
    pub magenta: &'a str, // values (defaults, enums, constants)
    pub red: &'a str,     // required field markers
    pub syntax_highlight: bool,
}

impl Fmt<'_> {
    pub fn color() -> Self {
        Fmt {
            bold: BOLD,
            dim: DIM,
            reset: RESET,
            cyan: CYAN,
            green: GREEN,
            yellow: YELLOW,
            magenta: MAGENTA,
            red: RED,
            syntax_highlight: true,
        }
    }

    pub fn plain() -> Self {
        Fmt {
            bold: "",
            dim: "",
            reset: "",
            cyan: "",
            green: "",
            yellow: "",
            magenta: "",
            red: "",
            syntax_highlight: false,
        }
    }

    /// Whether color output is enabled.
    pub(crate) fn is_color(&self) -> bool {
        !self.reset.is_empty()
    }

    /// Build `markdown_to_ansi::Options` from this formatting context.
    ///
    /// Pass `None` for unconstrained width, or `Some(cols)` to enable word-wrapping.
    pub(crate) fn md_opts(&self, width: Option<usize>) -> markdown_to_ansi::Options {
        markdown_to_ansi::Options {
            syntax_highlight: self.syntax_highlight,
            width,
        }
    }
}

/// Format a type string with color.
///
/// Splits on ` | ` to colorize each alternative, and handles
/// compound types like `array of string`.
pub(crate) fn format_type(ty: &str, f: &Fmt<'_>) -> String {
    if ty.is_empty() {
        return String::new();
    }
    if ty.contains(" | ") {
        let separator = format!(" {}|{} ", f.dim, f.reset);
        ty.split(" | ")
            .map(|p| format!("{}{p}{}", f.cyan, f.reset))
            .collect::<Vec<_>>()
            .join(&separator)
    } else if let Some(rest) = ty.strip_prefix("array of ") {
        format!(
            "{}array{} {}of{} {}",
            f.cyan,
            f.reset,
            f.dim,
            f.reset,
            format_type(rest, f)
        )
    } else {
        format!("{}{ty}{}", f.cyan, f.reset)
    }
}

/// Format a parenthesized type suffix, e.g. ` (string)`.
///
/// Returns an empty string when `ty` is empty.
pub(crate) fn format_type_suffix(ty: &str, f: &Fmt<'_>) -> String {
    if ty.is_empty() {
        String::new()
    } else {
        format!(" ({})", format_type(ty, f))
    }
}

/// Format a JSON value for display.
pub(crate) fn format_value(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => format!("\"{s}\""),
        other => other.to_string(),
    }
}

/// JSON Schema composition keywords, in the order they should be rendered.
pub(crate) const COMPOSITION_KEYWORDS: &[&str] = &["oneOf", "anyOf", "allOf"];

/// Format a centered header line: `LEFT      CENTER      LEFT`
pub(crate) fn format_header(left: &str, center: &str) -> String {
    let width = 76;
    let total_content = left.len() * 2 + center.len();
    if total_content >= width {
        return format!("{left}  {center}  {left}");
    }
    let total_space = width - total_content;
    let pad = total_space / 2;
    format!(
        "{left}{}{center}{}{left}",
        " ".repeat(pad),
        " ".repeat(total_space - pad)
    )
}
