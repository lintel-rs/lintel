use alloc::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Result;
use lintel_schema_cache::{CacheStatus, SchemaCache};
use schema_catalog::FileFormat;
use serde::{Deserialize, Serialize};

/// Thread-safe collection of all processed schema values, keyed by their
/// path relative to the output directory (e.g. `schemas/github/workflow/latest.json`).
#[derive(Clone)]
pub struct ProcessedSchemas {
    inner: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    output_dir: PathBuf,
}

impl ProcessedSchemas {
    pub fn new(output_dir: &Path) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            output_dir: output_dir.to_path_buf(),
        }
    }

    /// Insert a processed schema value, keyed by its path relative to `output_dir`.
    pub fn insert(&self, path: &Path, value: &serde_json::Value) {
        let relative = path
            .strip_prefix(&self.output_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        self.inner
            .lock()
            .expect("ProcessedSchemas lock poisoned")
            .insert(relative, value.clone());
    }

    /// Look up a schema by its relative path (e.g. `schemas/github/workflow/latest.json`).
    pub fn get(&self, relative_path: &str) -> Option<serde_json::Value> {
        self.inner
            .lock()
            .expect("ProcessedSchemas lock poisoned")
            .get(relative_path)
            .cloned()
    }

    /// Look up a schema by its absolute path, stripping the output directory
    /// prefix to derive the key.
    pub fn get_by_path(&self, path: &Path) -> Option<serde_json::Value> {
        let relative = path
            .strip_prefix(&self.output_dir)
            .unwrap_or(path)
            .to_string_lossy();
        self.get(&relative)
    }

    /// Total count of all stored schemas.
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("ProcessedSchemas lock poisoned")
            .len()
    }

    /// Return all relative paths stored.
    pub fn keys(&self) -> Vec<String> {
        self.inner
            .lock()
            .expect("ProcessedSchemas lock poisoned")
            .keys()
            .cloned()
            .collect()
    }
}

/// Maximum schema file size we'll download (10 MiB). Schemas larger than this
/// are skipped and the catalog retains the original upstream URL.
const MAX_SCHEMA_SIZE: u64 = 10 * 1024 * 1024;

#[allow(clippy::trivially_copy_pass_by_ref)] // serde requires fn(&bool) -> bool
fn is_false(b: &bool) -> bool {
    !b
}

/// Metadata injected into downloaded schemas as `x-lintel`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LintelExtra {
    /// Original URL the schema was fetched from.
    #[serde(default)]
    pub source: String,
    /// SHA-256 hex digest of the raw schema content before any transformations.
    #[serde(default)]
    pub source_sha256: String,
    /// `true` when the schema fails validation after transformation.
    #[serde(default, skip_serializing_if = "is_false")]
    pub invalid: bool,
    /// Glob patterns for files this schema should be associated with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_match: Vec<String>,
    /// Parsers that can handle files matched by this schema.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parsers: Vec<FileFormat>,
    /// Brief description used to populate the catalog entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_description: Option<String>,
}

/// Parse the `x-lintel` extension from a schema's root object.
///
/// Returns `None` if the key is missing or cannot be deserialized.
pub fn parse_lintel_extra(value: &serde_json::Value) -> Option<LintelExtra> {
    value
        .get("x-lintel")
        .and_then(|v| serde_json::from_value::<LintelExtra>(v.clone()).ok())
}

/// Fetch a schema via the cache. Returns the parsed `Value` and cache status.
/// Does NOT write to disk.
pub async fn fetch_one(cache: &SchemaCache, url: &str) -> Result<(serde_json::Value, CacheStatus)> {
    let (value, status) = cache.fetch(url).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok((value, status))
}

/// Write a `serde_json::Value` to disk as pretty-printed JSON.
/// Enforces [`MAX_SCHEMA_SIZE`]. Creates parent directories.
/// Inserts the value into `processed` for in-memory lookups.
///
/// Callers are expected to run [`crate::postprocess::postprocess_schema`]
/// before calling this function.
pub async fn write_schema_json(
    value: &serde_json::Value,
    path: &Path,
    processed: &ProcessedSchemas,
) -> Result<()> {
    processed.insert(path, value);

    let text = serde_json::to_string_pretty(value)?;

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
