use serde::{Deserialize, Serialize};

/// [Taplo] JSON Schema extension (`x-taplo`).
///
/// Controls editor behavior in the Taplo TOML language server: documentation
/// text, completion links, hidden fields, and plugins. Serialized as a
/// single nested JSON object under the `x-taplo` key.
///
/// Compatible with [`taplo-common`'s `SchemaExt`][taplo-ext].
///
/// # Example
///
/// ```json
/// {
///   "x-taplo": {
///     "hidden": true,
///     "docs": { "main": "Override description" },
///     "links": { "key": "https://example.com" }
///   }
/// }
/// ```
///
/// [Taplo]: https://taplo.tamasfe.dev
/// [taplo-ext]: https://github.com/tamasfe/taplo/blob/main/crates/taplo-common/src/schema/ext.rs
#[derive(
    Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct TaploSchemaExt {
    /// When `true`, the property is hidden from completion suggestions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,

    /// Navigation link targets shown as clickable references in the editor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<ExtLinks>,

    /// Documentation text overrides for hover popups and completion details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<ExtDocs>,

    /// Keys to insert when creating a new table from this schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init_keys: Option<Vec<String>>,

    /// Taplo plugin names to activate when this schema is in use.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<String>,
}

/// Documentation text overrides for [Taplo](TaploSchemaExt) hover and
/// completion.
///
/// Each field replaces the corresponding auto-generated documentation
/// that Taplo would otherwise derive from the schema.
#[derive(
    Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct ExtDocs {
    /// Primary documentation text, replacing the schema `description`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,

    /// Documentation for the `const` value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub const_value: Option<String>,

    /// Documentation for the `default` value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,

    /// Per-enum-value documentation strings (positional, matching the
    /// `enum` array). `None` entries leave the default text unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<Option<String>>>,
}

/// Link targets for [Taplo](TaploSchemaExt) navigation.
///
/// Provides clickable URLs in the editor for the property key and
/// individual enum values.
#[derive(
    Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct ExtLinks {
    /// URL to navigate to when clicking the property key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// Per-enum-value link URLs (positional, matching the `enum` array).
    /// `None` entries have no link.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<Option<String>>>,
}

/// [Taplo] schema catalog metadata (`x-taplo-info`).
///
/// Embeds file-association patterns, authorship, and version information
/// directly inside a schema file. Used by Taplo's built-in catalog to
/// match schemas to TOML files without a separate catalog entry.
///
/// Compatible with [`taplo-common`'s `SchemaExtraInfo`][taplo-info].
///
/// # Example
///
/// ```json
/// {
///   "x-taplo-info": {
///     "authors": ["Alice (https://example.com)"],
///     "version": "1.0.0",
///     "patterns": ["^pyproject\\.toml$"]
///   }
/// }
/// ```
///
/// [Taplo]: https://taplo.tamasfe.dev
/// [taplo-info]: https://github.com/tamasfe/taplo/blob/main/crates/taplo-common/src/schema/ext.rs
#[derive(Debug, Default, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaploInfoSchemaExt {
    /// Schema author credits, typically `"Name (url)"`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,

    /// Semver version of the schema or the tool it describes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Regex patterns matching file paths this schema applies to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<String>,
}
