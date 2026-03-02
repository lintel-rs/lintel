use serde::{Deserialize, Serialize};
use serde_json::Value;

/// [Tombi] JSON Schema extensions (`x-tombi-*`).
///
/// Tombi is a TOML language server and formatter. Unlike
/// [`TaploSchemaExt`](super::TaploSchemaExt) (a single nested object
/// under `x-taplo`), Tombi extensions are separate top-level keys on the
/// schema object. This struct is
/// [`#[serde(flatten)]`](https://serde.rs/attr-flatten.html)-ed into
/// [`Schema`](crate::Schema).
///
/// # Keys
///
/// | JSON key | Rust field | Purpose |
/// |---|---|---|
/// | `x-tombi-toml-version` | [`toml_version`](Self::toml_version) | Required TOML spec version |
/// | `x-tombi-table-keys-order` | [`table_keys_order`](Self::table_keys_order) | Preferred key ordering |
/// | `x-tombi-additional-key-label` | [`additional_key_label`](Self::additional_key_label) | Label for `additionalProperties` keys |
/// | `x-tombi-array-values-order` | [`array_values_order`](Self::array_values_order) | Preferred array element ordering |
///
/// [Tombi]: https://github.com/tombi-toml/tombi
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TombiSchemaExt {
    /// TOML specification version required by this schema (e.g. `"1.0.0"`).
    #[serde(
        rename = "x-tombi-toml-version",
        skip_serializing_if = "Option::is_none"
    )]
    pub toml_version: Option<String>,

    /// Preferred ordering of table keys for formatting.
    ///
    /// The value is tool-defined and typically an array of key names or an
    /// ordering strategy object.
    #[serde(
        rename = "x-tombi-table-keys-order",
        skip_serializing_if = "Option::is_none"
    )]
    pub table_keys_order: Option<Value>,

    /// Display label for keys matched by `additionalProperties`.
    #[serde(
        rename = "x-tombi-additional-key-label",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_key_label: Option<String>,

    /// Preferred ordering of array element values for formatting.
    ///
    /// The value is tool-defined and typically an array of values or an
    /// ordering strategy object.
    #[serde(
        rename = "x-tombi-array-values-order",
        skip_serializing_if = "Option::is_none"
    )]
    pub array_values_order: Option<Value>,
}
