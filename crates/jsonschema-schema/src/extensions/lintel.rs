use alloc::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// [Lintel] provenance metadata (`x-lintel`).
///
/// Embedded by the Lintel catalog builder to record where a schema was
/// fetched from and its content hash, enabling cache validation and
/// source attribution.
///
/// Serialized as a single nested JSON object under the `x-lintel` key:
///
/// ```json
/// {
///   "x-lintel": {
///     "source": "https://json.schemastore.org/tsconfig.json",
///     "sourceSha256": "a1b2c3..."
///   }
/// }
/// ```
///
/// [Lintel]: https://github.com/lintel-rs/lintel
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LintelSchemaExt {
    /// URL the schema was originally fetched from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// SHA-256 hex digest of the original schema content before any
    /// transformations (migration, injection, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_sha256: Option<String>,
    /// Catch-all for any additional Lintel-specific properties added in
    /// the future.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
