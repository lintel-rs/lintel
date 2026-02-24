use std::path::Path;

use anyhow::Result;
use lintel_schema_cache::{CacheStatus, SchemaCache};
use serde::{Deserialize, Serialize};

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
}

/// Inject `x-lintel` metadata into a schema's root object.
///
/// Looks up the content hash for `source_url` from the cache. If the hash is
/// not available (e.g. the schema was never fetched via HTTP), no metadata is
/// injected.
pub fn inject_lintel_extra(value: &mut serde_json::Value, source_url: &str, cache: &SchemaCache) {
    let Some(hash) = cache.content_hash(source_url) else {
        return;
    };
    let extra = LintelExtra {
        source: source_url.to_string(),
        source_sha256: hash,
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
