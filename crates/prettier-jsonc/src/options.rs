use serde::Deserialize;

/// Prettier-compatible formatting options.
///
/// Only includes options relevant to JSON, JSONC, JSON5, and YAML.
/// All defaults match prettier's defaults.
#[derive(Debug, Clone)]
pub struct PrettierOptions {
    /// Specify the line length that the printer will wrap on.
    pub print_width: usize,
    /// Specify the number of spaces per indentation-level.
    pub tab_width: usize,
    /// Indent lines with tabs instead of spaces.
    pub use_tabs: bool,
    /// Use single quotes instead of double quotes.
    pub single_quote: bool,
    /// Print trailing commas wherever possible in multi-line structures.
    pub trailing_comma: TrailingComma,
    /// Print spaces between brackets in object literals.
    pub bracket_spacing: bool,
    /// Which end of line characters to apply.
    pub end_of_line: EndOfLine,
    /// How to wrap prose (long text).
    pub prose_wrap: ProseWrap,
    /// Change when properties in objects are quoted.
    pub quote_props: QuoteProps,
}

impl Default for PrettierOptions {
    fn default() -> Self {
        Self {
            print_width: 80,
            tab_width: 2,
            use_tabs: false,
            single_quote: false,
            trailing_comma: TrailingComma::All,
            bracket_spacing: true,
            end_of_line: EndOfLine::Lf,
            prose_wrap: ProseWrap::Preserve,
            quote_props: QuoteProps::AsNeeded,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TrailingComma {
    #[default]
    All,
    Es5,
    None,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EndOfLine {
    #[default]
    Lf,
    Crlf,
    Cr,
    Auto,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProseWrap {
    Always,
    Never,
    #[default]
    Preserve,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum QuoteProps {
    #[default]
    AsNeeded,
    Consistent,
    Preserve,
}

/// Raw config file representation for serde deserialization.
/// Uses camelCase to match prettier's config format.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPrettierConfig {
    #[serde(default)]
    pub print_width: Option<usize>,
    #[serde(default)]
    pub tab_width: Option<usize>,
    #[serde(default)]
    pub use_tabs: Option<bool>,
    #[serde(default)]
    pub single_quote: Option<bool>,
    #[serde(default)]
    pub trailing_comma: Option<String>,
    #[serde(default)]
    pub bracket_spacing: Option<bool>,
    #[serde(default)]
    pub end_of_line: Option<String>,
    #[serde(default)]
    pub prose_wrap: Option<String>,
    #[serde(default)]
    pub quote_props: Option<String>,
    #[serde(default)]
    pub overrides: Option<Vec<RawOverride>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawOverride {
    #[serde(default)]
    pub files: OverrideFiles,
    #[serde(default)]
    pub options: Option<RawPrettierConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(untagged)]
pub enum OverrideFiles {
    Single(String),
    Multiple(Vec<String>),
    #[default]
    None,
}

impl RawPrettierConfig {
    /// Apply this raw config on top of existing options.
    pub fn apply_to(&self, opts: &mut PrettierOptions) {
        if let Some(v) = self.print_width {
            opts.print_width = v;
        }
        if let Some(v) = self.tab_width {
            opts.tab_width = v;
        }
        if let Some(v) = self.use_tabs {
            opts.use_tabs = v;
        }
        if let Some(v) = self.single_quote {
            opts.single_quote = v;
        }
        if let Some(ref v) = self.trailing_comma {
            match v.as_str() {
                "all" => opts.trailing_comma = TrailingComma::All,
                "es5" => opts.trailing_comma = TrailingComma::Es5,
                "none" => opts.trailing_comma = TrailingComma::None,
                _ => {}
            }
        }
        if let Some(v) = self.bracket_spacing {
            opts.bracket_spacing = v;
        }
        if let Some(ref v) = self.end_of_line {
            match v.as_str() {
                "lf" => opts.end_of_line = EndOfLine::Lf,
                "crlf" => opts.end_of_line = EndOfLine::Crlf,
                "cr" => opts.end_of_line = EndOfLine::Cr,
                "auto" => opts.end_of_line = EndOfLine::Auto,
                _ => {}
            }
        }
        if let Some(ref v) = self.prose_wrap {
            match v.as_str() {
                "always" => opts.prose_wrap = ProseWrap::Always,
                "never" => opts.prose_wrap = ProseWrap::Never,
                "preserve" => opts.prose_wrap = ProseWrap::Preserve,
                _ => {}
            }
        }
        if let Some(ref v) = self.quote_props {
            match v.as_str() {
                "as-needed" => opts.quote_props = QuoteProps::AsNeeded,
                "consistent" => opts.quote_props = QuoteProps::Consistent,
                "preserve" => opts.quote_props = QuoteProps::Preserve,
                _ => {}
            }
        }
    }
}
