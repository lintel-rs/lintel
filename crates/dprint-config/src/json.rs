//! Configuration for the dprint JSON plugin.
//!
//! See: <https://dprint.dev/plugins/json/config/>

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// See: <https://dprint.dev/plugins/json/config/>
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

/// See: <https://dprint.dev/plugins/json/config/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum TrailingCommas {
    /// Always format with trailing commas. Beware: trailing commas can cause many JSON parsers to fail.
    #[serde(rename = "always")]
    Always,
    /// Use trailing commas in JSONC files and do not use trailing commas in JSON files.
    #[serde(rename = "jsonc")]
    Jsonc,
    /// Keep the trailing comma if it exists.
    #[serde(rename = "maintain")]
    Maintain,
    /// Never format with trailing commas.
    #[serde(rename = "never")]
    Never,
}

/// Configuration for the dprint [JSON](https://dprint.dev/plugins/json/config/) plugin.
///
/// See: <https://dprint.dev/plugins/json/config/>
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[schemars(title = "JSON Plugin Configuration")]
pub struct JsonConfig {
    /// Whether the configuration is not allowed to be overridden or extended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,

    /// File patterns to associate with this plugin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub associations: Option<Vec<String>>,

    /// If arrays and objects should collapse to a single line if it would be below the line width.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/json/config/#arraypreferSingleLine>
    #[serde(
        default,
        rename = "array.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub array_prefer_single_line: Option<bool>,

    /// Forces a space after slashes.  For example: `// comment` instead of `//comment`
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/json/config/#commentLineforceSpaceAfterSlashes>
    #[serde(
        default,
        rename = "commentLine.forceSpaceAfterSlashes",
        skip_serializing_if = "Option::is_none"
    )]
    pub comment_line_force_space_after_slashes: Option<bool>,

    /// Top level configuration that sets the configuration to what is used in Deno.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/json/config/#deno>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deno: Option<bool>,

    /// The text to use for an ignore comment (ex. `// dprint-ignore`).
    ///
    /// Default: `"dprint-ignore"`
    ///
    /// See: <https://dprint.dev/plugins/json/config/#ignoreNodeCommentText>
    #[serde(
        default,
        rename = "ignoreNodeCommentText",
        alias = "ignore-node-comment-text",
        skip_serializing_if = "Option::is_none"
    )]
    pub ignore_node_comment_text: Option<String>,

    /// The number of characters for an indent.
    ///
    /// Default: `2`
    ///
    /// See: <https://dprint.dev/plugins/json/config/#indentWidth>
    #[serde(
        default,
        rename = "indentWidth",
        alias = "indent-width",
        skip_serializing_if = "Option::is_none"
    )]
    pub indent_width: Option<u32>,

    /// When `trailingCommas` is `jsonc`, treat these files as JSONC and use trailing commas (ex. `["tsconfig.json", ".vscode/settings.json"]`).
    ///
    /// See: <https://dprint.dev/plugins/json/config/#jsonTrailingCommaFiles>
    #[serde(
        default,
        rename = "jsonTrailingCommaFiles",
        alias = "json-trailing-comma-files",
        skip_serializing_if = "Option::is_none"
    )]
    pub json_trailing_comma_files: Option<Vec<String>>,

    /// The width of a line the printer will try to stay under. Note that the printer may exceed this width in certain cases.
    ///
    /// Default: `120`
    ///
    /// See: <https://dprint.dev/plugins/json/config/#lineWidth>
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
    /// See: <https://dprint.dev/plugins/json/config/#newLineKind>
    #[serde(
        default,
        rename = "newLineKind",
        alias = "new-line-kind",
        skip_serializing_if = "Option::is_none"
    )]
    pub new_line_kind: Option<NewLineKind>,

    /// If arrays and objects should collapse to a single line if it would be below the line width.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/json/config/#objectpreferSingleLine>
    #[serde(
        default,
        rename = "object.preferSingleLine",
        skip_serializing_if = "Option::is_none"
    )]
    pub object_prefer_single_line: Option<bool>,

    /// If arrays and objects should collapse to a single line if it would be below the line width.
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/json/config/#preferSingleLine>
    #[serde(
        default,
        rename = "preferSingleLine",
        alias = "prefer-single-line",
        skip_serializing_if = "Option::is_none"
    )]
    pub prefer_single_line: Option<bool>,

    /// Whether to use trailing commas.
    ///
    /// Default: `"jsonc"`
    ///
    /// See: <https://dprint.dev/plugins/json/config/#trailingCommas>
    #[serde(
        default,
        rename = "trailingCommas",
        alias = "trailing-commas",
        skip_serializing_if = "Option::is_none"
    )]
    pub trailing_commas: Option<TrailingCommas>,

    /// Whether to use tabs (true) or spaces (false).
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/json/config/#useTabs>
    #[serde(
        default,
        rename = "useTabs",
        alias = "use-tabs",
        skip_serializing_if = "Option::is_none"
    )]
    pub use_tabs: Option<bool>,

    /// Additional plugin-specific settings not covered by the typed fields.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
