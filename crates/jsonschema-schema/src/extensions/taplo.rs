use alloc::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Taplo JSON Schema extension (`x-taplo`).
///
/// Controls editor behavior in the Taplo TOML language server: documentation
/// text, completion links, hidden fields, and plugins.
///
/// Compatible with taplo-common's `TaploSchemaExt`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaploSchemaExt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<ExtLinks>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<ExtDocs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init_keys: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Documentation text overrides for Taplo hover/completion.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ExtDocs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub const_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<Option<String>>>,
}

/// Link targets for Taplo navigation.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ExtLinks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<Option<String>>>,
}

/// Taplo schema catalog metadata (`x-taplo-info`).
///
/// Embeds file-association patterns, authorship, and version information
/// directly inside a schema file. Used by Taplo's built-in catalog to
/// match schemas to TOML files without a separate catalog entry.
///
/// Compatible with taplo-common's `TaploSchemaExtraInfo`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TaploInfo {
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
