#[cfg(test)]
pub(crate) use ansi_term_styles::BLUE;
pub(crate) use ansi_term_styles::{BOLD, CYAN, DIM, GREEN, MAGENTA, RED, RESET, YELLOW};

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
    pub width: usize,
}

impl Fmt<'_> {
    /// Build a `Fmt` from [`ExplainOptions`](crate::ExplainOptions).
    pub fn from_opts(opts: &crate::ExplainOptions) -> Self {
        let mut f = if opts.color {
            Self::color(opts.width)
        } else {
            Self::plain(opts.width)
        };
        f.syntax_highlight = opts.syntax_highlight;
        f
    }

    pub fn color(width: usize) -> Self {
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
            width,
        }
    }

    pub fn plain(width: usize) -> Self {
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
            width,
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
            code_bg: true,
        }
    }
}

/// Format a type string with color.
///
/// Splits on ` | ` to colorize each alternative, and handles
/// compound types like `string[]`.
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
    } else if let Some(rest) = ty.strip_suffix("[]") {
        format!("{}{}[]{}", format_type(rest, f), f.cyan, f.reset)
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
pub(crate) fn format_header(left: &str, center: &str, width: usize) -> String {
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
