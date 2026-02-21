use std::sync::LazyLock;

use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use ansi_term_codes::RESET;

static DEFAULTS: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

/// Extra syntax set for languages not in syntect's defaults (e.g. TOML).
/// Precompiled at build time via the `sublime-syntaxes` crate.
static EXTRAS: LazyLock<SyntaxSet> = LazyLock::new(sublime_syntaxes::extra_syntax_set);

pub(crate) static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Look up a syntax by language token, searching defaults first, then extras.
fn find_syntax(
    lang: &str,
) -> (
    &'static SyntaxSet,
    &'static syntect::parsing::SyntaxReference,
) {
    if lang.is_empty() {
        return (&*DEFAULTS, DEFAULTS.find_syntax_plain_text());
    }
    if let Some(s) = DEFAULTS.find_syntax_by_token(lang) {
        return (&*DEFAULTS, s);
    }
    if let Some(s) = EXTRAS.find_syntax_by_token(lang) {
        return (&*EXTRAS, s);
    }
    (&*DEFAULTS, DEFAULTS.find_syntax_plain_text())
}

/// Returns true if a syntax definition exists for the given language token.
pub fn has_syntax(lang: &str) -> bool {
    DEFAULTS.find_syntax_by_token(lang).is_some() || EXTRAS.find_syntax_by_token(lang).is_some()
}

/// Compute the visible column width of a string, handling tabs (4-column stops)
/// and skipping ANSI escape sequences.
fn visible_width(s: &str) -> usize {
    let mut width = 0;
    let mut in_escape = false;
    for ch in s.chars() {
        if in_escape {
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else if ch == '\x1b' {
            in_escape = true;
        } else if ch == '\t' {
            width = (width + 4) & !3;
        } else if ch != '\n' && ch != '\r' {
            width += 1;
        }
    }
    width
}

/// Syntax-highlight a code block using syntect, with background padding to terminal width.
///
/// When color or syntax highlighting is disabled, returns the code unchanged.
/// Falls back to plain text if the language is unknown.
pub(crate) fn highlight_code_block(code: &str, lang: &str, width: Option<usize>) -> String {
    let (syntax_set, syntax) = find_syntax(lang);
    let theme = &THEME_SET.themes["base16-ocean.dark"];
    let mut h = syntect::easy::HighlightLines::new(syntax, theme);
    let mut out = String::new();

    let term_width = width.unwrap_or(80);

    let bg = theme
        .settings
        .background
        .map(|c| format!("\x1b[48;2;{};{};{}m", c.r, c.g, c.b));

    for line in syntect::util::LinesWithEndings::from(code) {
        match h.highlight_line(line, syntax_set) {
            Ok(ranges) => {
                let highlighted = syntect::util::as_24_bit_terminal_escaped(&ranges, true);
                let highlighted = highlighted.trim_end_matches('\n');

                if let Some(ref bg_code) = bg {
                    let padding = term_width.saturating_sub(visible_width(highlighted));
                    out.push_str(bg_code);
                    out.push_str(highlighted);
                    out.extend(core::iter::repeat_n(' ', padding));
                } else {
                    out.push_str(highlighted);
                }
                out.push_str(RESET);
                out.push('\n');
            }
            Err(_) => out.push_str(line),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_width_ascii() {
        assert_eq!(visible_width("hello"), 5);
    }

    #[test]
    fn visible_width_tabs() {
        // Tab at column 0 advances to column 4
        assert_eq!(visible_width("\t"), 4);
        // "ab" is 2 columns, tab advances to column 4
        assert_eq!(visible_width("ab\t"), 4);
        // "abcd" is 4 columns, tab advances to column 8
        assert_eq!(visible_width("abcd\t"), 8);
    }

    #[test]
    fn visible_width_strips_ansi() {
        assert_eq!(visible_width("\x1b[31mhello\x1b[0m"), 5);
        assert_eq!(visible_width("\x1b[38;2;100;200;50mfoo\x1b[0m"), 3);
    }

    #[test]
    fn visible_width_empty() {
        assert_eq!(visible_width(""), 0);
        assert_eq!(visible_width("\n"), 0);
    }
}
