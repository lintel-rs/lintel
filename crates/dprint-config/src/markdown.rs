//! Configuration for the dprint Markdown plugin.
//!
//! See: <https://dprint.dev/plugins/markdown/config/>

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// See: <https://dprint.dev/plugins/markdown/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum HeadingKind {
    /// Uses # or ## before the heading text (ATX headings).
    #[serde(rename = "atx")]
    Atx,
    /// Uses an underline of = or - beneath the heading text (setext headings). Only applies to level 1 and 2 headings.
    #[serde(rename = "setext")]
    Setext,
}

/// See: <https://dprint.dev/plugins/markdown/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum NewLineKind {
    /// For each file, uses the newline kind found at the end of the last line.
    #[serde(rename = "auto")]
    Auto,
    /// Uses carriage return, line feed.
    #[serde(rename = "crlf")]
    Crlf,
    /// Uses line feed.
    #[serde(rename = "lf")]
    Lf,
    /// Uses the system standard (ex. crlf on Windows).
    #[serde(rename = "system")]
    System,
}

/// See: <https://dprint.dev/plugins/markdown/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum StrongKind {
    /// Uses asterisks (*) for emphasis.
    #[serde(rename = "asterisks")]
    Asterisks,
    /// Uses underscores (_) for emphasis.
    #[serde(rename = "underscores")]
    Underscores,
}

/// See: <https://dprint.dev/plugins/markdown/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum TextWrap {
    /// Always wraps text.
    #[serde(rename = "always")]
    Always,
    /// Maintains line breaks.
    #[serde(rename = "maintain")]
    Maintain,
    /// Never wraps text.
    #[serde(rename = "never")]
    Never,
}

/// See: <https://dprint.dev/plugins/markdown/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum UnorderedListKind {
    /// Uses dashes (-) as primary character for lists.
    #[serde(rename = "dashes")]
    Dashes,
    /// Uses asterisks (*) as primary character for lists.
    #[serde(rename = "asterisks")]
    Asterisks,
}

/// Configuration for the dprint [Markdown](https://dprint.dev/plugins/markdown/config/) plugin.
///
/// See: <https://dprint.dev/plugins/markdown/config/>
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[schemars(title = "Markdown Plugin Configuration")]
pub struct MarkdownConfig {
    /// Whether the configuration is not allowed to be overridden or extended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,

    /// File patterns to associate with this plugin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub associations: Option<Vec<String>>,

    /// Top level configuration that sets the configuration to what is used in Deno.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#deno>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deno: Option<bool>,

    /// The character to use for emphasis/italics.
    ///
    /// Default: `"underscores"`
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#emphasisKind>
    #[serde(
        default,
        rename = "emphasisKind",
        alias = "emphasis-kind",
        skip_serializing_if = "Option::is_none"
    )]
    pub emphasis_kind: Option<StrongKind>,

    /// The style of heading to use for level 1 and level 2 headings. Level 3 and higher always use ATX headings.
    ///
    /// Default: `"atx"`
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#headingKind>
    #[serde(
        default,
        rename = "headingKind",
        alias = "heading-kind",
        skip_serializing_if = "Option::is_none"
    )]
    pub heading_kind: Option<HeadingKind>,

    /// The text to use for an ignore directive (ex. `<!-- dprint-ignore -->`).
    ///
    /// Default: `"dprint-ignore"`
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#ignoreDirective>
    #[serde(
        default,
        rename = "ignoreDirective",
        alias = "ignore-directive",
        skip_serializing_if = "Option::is_none"
    )]
    pub ignore_directive: Option<String>,

    /// The text to use for an ignore end directive (ex. `<!-- dprint-ignore-end -->`).
    ///
    /// Default: `"dprint-ignore-end"`
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#ignoreEndDirective>
    #[serde(
        default,
        rename = "ignoreEndDirective",
        alias = "ignore-end-directive",
        skip_serializing_if = "Option::is_none"
    )]
    pub ignore_end_directive: Option<String>,

    /// The text to use for an ignore file directive (ex. `<!-- dprint-ignore-file -->`).
    ///
    /// Default: `"dprint-ignore-file"`
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#ignoreFileDirective>
    #[serde(
        default,
        rename = "ignoreFileDirective",
        alias = "ignore-file-directive",
        skip_serializing_if = "Option::is_none"
    )]
    pub ignore_file_directive: Option<String>,

    /// The text to use for an ignore start directive (ex. `<!-- dprint-ignore-start -->`).
    ///
    /// Default: `"dprint-ignore-start"`
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#ignoreStartDirective>
    #[serde(
        default,
        rename = "ignoreStartDirective",
        alias = "ignore-start-directive",
        skip_serializing_if = "Option::is_none"
    )]
    pub ignore_start_directive: Option<String>,

    /// The width of a line the printer will try to stay under. Note that the printer may exceed this width in certain cases.
    ///
    /// Default: `80`
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#lineWidth>
    #[serde(
        default,
        rename = "lineWidth",
        alias = "line-width",
        skip_serializing_if = "Option::is_none"
    )]
    pub line_width: Option<u32>,

    /// The kind of newline to use.
    ///
    /// Default: `"lf"`
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#newLineKind>
    #[serde(
        default,
        rename = "newLineKind",
        alias = "new-line-kind",
        skip_serializing_if = "Option::is_none"
    )]
    pub new_line_kind: Option<NewLineKind>,

    /// The character to use for strong emphasis/bold.
    ///
    /// Default: `"asterisks"`
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#strongKind>
    #[serde(
        default,
        rename = "strongKind",
        alias = "strong-kind",
        skip_serializing_if = "Option::is_none"
    )]
    pub strong_kind: Option<StrongKind>,

    /// Custom tag to file extension mappings for formatting code blocks. For example: { "markdown": "md" }
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#tags>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<BTreeMap<String, String>>,

    /// Text wrapping possibilities.
    ///
    /// Default: `"maintain"`
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#textWrap>
    #[serde(
        default,
        rename = "textWrap",
        alias = "text-wrap",
        skip_serializing_if = "Option::is_none"
    )]
    pub text_wrap: Option<TextWrap>,

    /// The character to use for unordered lists.
    ///
    /// Default: `"dashes"`
    ///
    /// See: <https://dprint.dev/plugins/markdown/config/#unorderedListKind>
    #[serde(
        default,
        rename = "unorderedListKind",
        alias = "unordered-list-kind",
        skip_serializing_if = "Option::is_none"
    )]
    pub unordered_list_kind: Option<UnorderedListKind>,

    /// Additional plugin-specific settings not covered by the typed fields.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
