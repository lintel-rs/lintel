//! Configuration for the dprint TOML plugin.
//!
//! See: <https://dprint.dev/plugins/toml/config/>

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// See: <https://dprint.dev/plugins/toml/config/>
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

/// Configuration for the dprint [TOML](https://dprint.dev/plugins/toml/config/) plugin.
///
/// See: <https://dprint.dev/plugins/toml/config/>
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[schemars(title = "TOML Plugin Configuration")]
pub struct TomlConfig {
    /// Whether the configuration is not allowed to be overridden or extended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,

    /// File patterns to associate with this plugin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub associations: Option<Vec<String>>,

    /// Whether to apply sorting to a Cargo.toml file.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/toml/config/#cargoapplyConventions>
    #[serde(
        default,
        rename = "cargo.applyConventions",
        skip_serializing_if = "Option::is_none"
    )]
    pub cargo_apply_conventions: Option<bool>,

    /// Whether to force a leading space in a comment.
    ///
    /// Default: `true`
    ///
    /// See: <https://dprint.dev/plugins/toml/config/#commentforceLeadingSpace>
    #[serde(
        default,
        rename = "comment.forceLeadingSpace",
        skip_serializing_if = "Option::is_none"
    )]
    pub comment_force_leading_space: Option<bool>,

    /// The number of characters for an indent.
    ///
    /// Default: `2`
    ///
    /// See: <https://dprint.dev/plugins/toml/config/#indentWidth>
    #[serde(
        default,
        rename = "indentWidth",
        alias = "indent-width",
        skip_serializing_if = "Option::is_none"
    )]
    pub indent_width: Option<u32>,

    /// The width of a line the printer will try to stay under. Note that the printer may exceed this width in certain cases.
    ///
    /// Default: `120`
    ///
    /// See: <https://dprint.dev/plugins/toml/config/#lineWidth>
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
    /// See: <https://dprint.dev/plugins/toml/config/#newLineKind>
    #[serde(
        default,
        rename = "newLineKind",
        alias = "new-line-kind",
        skip_serializing_if = "Option::is_none"
    )]
    pub new_line_kind: Option<NewLineKind>,

    /// Whether to use tabs (true) or spaces (false).
    ///
    /// Default: `false`
    ///
    /// See: <https://dprint.dev/plugins/toml/config/#useTabs>
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
