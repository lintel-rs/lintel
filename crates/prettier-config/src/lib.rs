pub mod resolve;

use serde::{Deserialize, Serialize};

/// Prettier-compatible formatting configuration.
///
/// All fields use `#[serde(default)]` so that partial configs (e.g. from
/// `.prettierrc`) are filled in with defaults automatically. Field names
/// serialize/deserialize as camelCase to match prettier's config format.
///
/// Defaults match prettier 3.x behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct PrettierConfig {
    // ── Universal ───────────────────────────────────────────────────────
    /// Specify the line length that the printer will wrap on.
    pub print_width: usize,

    /// Specify the number of spaces per indentation-level.
    pub tab_width: usize,

    /// Indent lines with tabs instead of spaces.
    pub use_tabs: bool,

    /// Which end of line characters to apply.
    pub end_of_line: EndOfLine,

    // ── Quoting / spacing ───────────────────────────────────────────────
    /// Use single quotes instead of double quotes.
    pub single_quote: bool,

    /// Print spaces between brackets in object literals.
    pub bracket_spacing: bool,

    /// Change when properties in objects are quoted.
    pub quote_props: QuoteProps,

    // ── Commas ──────────────────────────────────────────────────────────
    /// Print trailing commas wherever possible in multi-line structures.
    pub trailing_comma: TrailingComma,

    // ── Prose ───────────────────────────────────────────────────────────
    /// How to wrap prose (markdown / YAML long text).
    pub prose_wrap: ProseWrap,

    // ── JS/TS (schema completeness) ─────────────────────────────────────
    /// Print semicolons at the ends of statements.
    pub semi: bool,

    /// Include parentheses around a sole arrow function parameter.
    pub arrow_parens: ArrowParens,

    /// Use single quotes in JSX.
    pub jsx_single_quote: bool,

    /// Put the `>` of a multi-line HTML element at the end of the last line
    /// instead of being alone on the next line.
    pub bracket_same_line: bool,

    // ── HTML ────────────────────────────────────────────────────────────
    /// How to handle whitespace in HTML.
    pub html_whitespace_sensitivity: HtmlWhitespaceSensitivity,

    /// Whether or not to indent the code inside `<script>` and `<style>`
    /// tags in Vue files.
    pub vue_indent_script_and_style: bool,

    /// Enforce single attribute per line in HTML, Vue and JSX.
    pub single_attribute_per_line: bool,

    // ── Embedded ────────────────────────────────────────────────────────
    /// Control whether Prettier formats quoted code embedded in the file.
    pub embedded_language_formatting: EmbeddedLanguageFormatting,

    // ── Pragma ──────────────────────────────────────────────────────────
    /// Insert `@format` pragma into file's first docblock comment.
    pub insert_pragma: bool,

    /// Require a special comment (`@prettier` or `@format`) to be present
    /// in the file's first docblock comment in order to format it.
    pub require_pragma: bool,

    // ── Range ───────────────────────────────────────────────────────────
    /// Format only a segment of a file — start offset.
    pub range_start: usize,

    /// Format only a segment of a file — end offset.
    pub range_end: usize,

    // ── Parser ──────────────────────────────────────────────────────────
    /// Specify which parser to use.
    pub parser: Option<String>,

    /// Specify the file name to use to infer which parser to use.
    pub filepath: Option<String>,

    // ── Editor ──────────────────────────────────────────────────────────
    /// Whether to take `.editorconfig` into account when parsing configuration.
    pub editorconfig: bool,
}

impl Default for PrettierConfig {
    fn default() -> Self {
        Self {
            print_width: 80,
            tab_width: 2,
            use_tabs: false,
            end_of_line: EndOfLine::Lf,
            single_quote: false,
            bracket_spacing: true,
            quote_props: QuoteProps::AsNeeded,
            trailing_comma: TrailingComma::All,
            prose_wrap: ProseWrap::Preserve,
            semi: true,
            arrow_parens: ArrowParens::Always,
            jsx_single_quote: false,
            bracket_same_line: false,
            html_whitespace_sensitivity: HtmlWhitespaceSensitivity::Css,
            vue_indent_script_and_style: false,
            single_attribute_per_line: false,
            embedded_language_formatting: EmbeddedLanguageFormatting::Auto,
            insert_pragma: false,
            require_pragma: false,
            range_start: 0,
            range_end: usize::MAX,
            parser: None,
            filepath: None,
            editorconfig: false,
        }
    }
}

// ── Enums ───────────────────────────────────────────────────────────────────

/// Print trailing commas wherever possible in multi-line structures.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrailingComma {
    /// Trailing commas everywhere possible.
    #[default]
    All,
    /// Trailing commas where valid in ES5 (objects, arrays, etc.).
    Es5,
    /// No trailing commas.
    None,
}

/// Which end of line characters to apply.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EndOfLine {
    /// Line Feed only (`\n`), common on Linux and macOS.
    #[default]
    Lf,
    /// Carriage Return + Line Feed (`\r\n`), common on Windows.
    Crlf,
    /// Carriage Return only (`\r`), used very rarely.
    Cr,
    /// Maintain existing line endings.
    Auto,
}

/// How to wrap prose.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProseWrap {
    /// Wrap prose if it exceeds the print width.
    Always,
    /// Do not wrap prose.
    Never,
    /// Preserve original wrapping.
    #[default]
    Preserve,
}

/// Change when properties in objects are quoted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuoteProps {
    /// Only add quotes around object properties where required.
    #[default]
    AsNeeded,
    /// If at least one property in an object requires quotes, quote all properties.
    Consistent,
    /// Respect the input use of quotes in object properties.
    Preserve,
}

/// Include parentheses around a sole arrow function parameter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArrowParens {
    /// Always include parens. Example: `(x) => x`
    #[default]
    Always,
    /// Omit parens when possible. Example: `x => x`
    Avoid,
}

/// How to handle whitespace in HTML.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HtmlWhitespaceSensitivity {
    /// Respect the default value of CSS `display` property.
    #[default]
    Css,
    /// Whitespace is considered sensitive.
    Strict,
    /// Whitespace is considered insensitive.
    Ignore,
}

/// Control whether Prettier formats quoted code embedded in the file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EmbeddedLanguageFormatting {
    /// Format embedded code if Prettier can automatically identify it.
    #[default]
    Auto,
    /// Never automatically format embedded code.
    Off,
}
