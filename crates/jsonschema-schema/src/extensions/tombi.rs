use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tombi JSON Schema extensions (`x-tombi-*`).
///
/// Unlike `x-taplo` (a single nested object), Tombi extensions are separate
/// top-level keys on the schema. This struct is flattened into `Schema` so
/// the individual `x-tombi-*` keys serialize/deserialize correctly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TombiExt {
    #[serde(
        rename = "x-tombi-toml-version",
        skip_serializing_if = "Option::is_none"
    )]
    pub toml_version: Option<String>,
    #[serde(
        rename = "x-tombi-table-keys-order",
        skip_serializing_if = "Option::is_none"
    )]
    pub table_keys_order: Option<Value>,
    #[serde(
        rename = "x-tombi-additional-key-label",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_key_label: Option<String>,
    #[serde(
        rename = "x-tombi-array-values-order",
        skip_serializing_if = "Option::is_none"
    )]
    pub array_values_order: Option<Value>,
}
