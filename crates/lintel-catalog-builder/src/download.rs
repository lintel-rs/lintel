use std::path::Path;

use anyhow::Result;
use lintel_schema_cache::{CacheStatus, SchemaCache};
use serde::{Deserialize, Serialize};
use tracing::warn;

/// Maximum schema file size we'll download (10 MiB). Schemas larger than this
/// are skipped and the catalog retains the original upstream URL.
const MAX_SCHEMA_SIZE: u64 = 10 * 1024 * 1024;

/// Metadata injected into downloaded schemas as `x-lintel`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintelExtra {
    /// Original URL the schema was fetched from.
    pub source: String,
    /// SHA-256 hex digest of the raw schema content before any transformations.
    #[serde(rename = "sourceSha256")]
    pub source_sha256: String,
    /// `true` when the schema fails validation after transformation.
    #[serde(default, skip_serializing_if = "is_false")]
    pub invalid: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde requires fn(&bool) -> bool
fn is_false(b: &bool) -> bool {
    !b
}

/// A retriever that returns a permissive schema for any URI.
///
/// Used during schema validation so that external `$ref` targets don't cause
/// compilation failures — we only want to catch structural issues in the
/// schema itself.
struct NoopRetriever;

impl jsonschema::Retrieve for NoopRetriever {
    fn retrieve(
        &self,
        _uri: &jsonschema::Uri<String>,
    ) -> Result<serde_json::Value, Box<dyn core::error::Error + Send + Sync>> {
        // `true` is the most permissive JSON Schema (accepts everything).
        Ok(serde_json::Value::Bool(true))
    }
}

/// Check whether a JSON Schema value is invalid by attempting to compile it.
///
/// Returns `true` if the schema fails compilation. Uses a no-op retriever
/// so external `$ref`s resolve to permissive schemas without network I/O.
fn is_schema_invalid(value: &serde_json::Value) -> bool {
    jsonschema::options()
        .with_retriever(NoopRetriever)
        .build(value)
        .is_err()
}

/// Inject `x-lintel` metadata into a schema's root object.
///
/// Looks up the content hash for `source_url` from the cache. If the hash is
/// not available (e.g. the schema was never fetched via HTTP), no metadata is
/// injected. Also validates the schema and sets `invalid: true` if it fails
/// compilation.
pub fn inject_lintel_extra(value: &mut serde_json::Value, source_url: &str, cache: &SchemaCache) {
    let Some(hash) = cache.content_hash(source_url) else {
        return;
    };
    let invalid = is_schema_invalid(value);
    if invalid {
        warn!(source = %source_url, "schema is invalid after transformation");
    }
    let extra = LintelExtra {
        source: source_url.to_string(),
        source_sha256: hash,
        invalid,
    };
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "x-lintel".to_string(),
            serde_json::to_value(extra).expect("LintelExtra is always serializable"),
        );
    }
}

/// Fetch a schema via the cache. Returns the parsed `Value` and cache status.
/// Does NOT write to disk.
pub async fn fetch_one(cache: &SchemaCache, url: &str) -> Result<(serde_json::Value, CacheStatus)> {
    let (value, status) = cache.fetch(url).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok((value, status))
}

/// Preferred key order for the root object of a JSON Schema.
const SCHEMA_KEY_ORDER: &[&str] = &[
    "$schema",
    "$id",
    "title",
    "description",
    "x-lintel",
    "type",
    "properties",
];

/// Reorder the top-level keys of a JSON Schema object so that well-known
/// fields appear first (in [`SCHEMA_KEY_ORDER`]), followed by the rest in
/// their original order.
fn reorder_schema_keys(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let mut ordered = serde_json::Map::with_capacity(obj.len());
    for &key in SCHEMA_KEY_ORDER {
        if let Some(v) = obj.remove(key) {
            ordered.insert(key.to_string(), v);
        }
    }
    // Append remaining keys in their original order
    ordered.extend(core::mem::take(obj));
    *obj = ordered;
}

/// Write a `serde_json::Value` to disk as pretty-printed JSON.
/// Enforces [`MAX_SCHEMA_SIZE`]. Creates parent directories.
/// Reorders top-level keys so well-known schema fields come first.
pub async fn write_schema_json(value: &serde_json::Value, path: &Path) -> Result<()> {
    let mut value = value.clone();
    reorder_schema_keys(&mut value);
    let text = serde_json::to_string_pretty(&value)?;

    if text.len() as u64 > MAX_SCHEMA_SIZE {
        anyhow::bail!(
            "schema too large ({} MiB, limit {} MiB)",
            text.len() / (1024 * 1024),
            MAX_SCHEMA_SIZE / (1024 * 1024),
        );
    }

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, format!("{text}\n")).await?;
    Ok(())
}
