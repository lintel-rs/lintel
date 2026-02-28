use alloc::collections::BTreeSet;
use alloc::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Result;
use lintel_schema_cache::{CacheStatus, SchemaCache};
use schema_catalog::FileFormat;
use serde::{Deserialize, Serialize};
use tracing::warn;

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
/// Validates the schema and sets `invalid: true` if it fails compilation.
/// The `extra` struct is completed with the `invalid` flag before insertion.
pub fn inject_lintel_extra(value: &mut serde_json::Value, mut extra: LintelExtra) {
    let invalid = is_schema_invalid(value);
    if invalid {
        warn!(source = %extra.source, "schema is invalid after transformation");
    }
    extra.invalid = invalid;
    // Preserve catalogDescription from existing x-lintel if not already set.
    if extra.catalog_description.is_none()
        && let Some(existing) = parse_lintel_extra(value)
    {
        extra.catalog_description = existing.catalog_description;
    }
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "x-lintel".to_string(),
            serde_json::to_value(extra).expect("LintelExtra is always serializable"),
        );
    }
}

/// Inject `x-lintel` metadata using the HTTP cache to look up the content hash.
///
/// If the hash is not available (e.g. the schema was never fetched via HTTP),
/// no metadata is injected. The provided `extra` is completed with the
/// source hash before injection.
pub fn inject_lintel_extra_from_cache(
    value: &mut serde_json::Value,
    cache: &SchemaCache,
    mut extra: LintelExtra,
) {
    let Some(hash) = cache.content_hash(&extra.source) else {
        return;
    };
    extra.source_sha256 = hash;
    inject_lintel_extra(value, extra);
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

/// Derive file formats from `fileMatch` glob patterns by inspecting extensions.
///
/// Returns a sorted, deduplicated list of formats.
pub fn parsers_from_file_match(patterns: &[String]) -> Vec<FileFormat> {
    let mut parsers: BTreeSet<FileFormat> = BTreeSet::new();
    for pattern in patterns {
        // Strip leading path components (e.g. "**/*.yml" → "*.yml")
        let base = pattern.rsplit('/').next().unwrap_or(pattern);
        if let Some(dot_pos) = base.rfind('.') {
            let format = match &base[dot_pos..] {
                ".json" => Some(FileFormat::Json),
                ".jsonc" => Some(FileFormat::Jsonc),
                ".json5" => Some(FileFormat::Json5),
                ".yaml" | ".yml" => Some(FileFormat::Yaml),
                ".toml" => Some(FileFormat::Toml),
                ".md" | ".mdx" => Some(FileFormat::Markdown),
                _ => None,
            };
            if let Some(f) = format {
                parsers.insert(f);
            }
        }
    }
    parsers.into_iter().collect()
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
/// Also inserts the final value into `processed` for in-memory lookups.
pub async fn write_schema_json(
    value: &serde_json::Value,
    path: &Path,
    processed: &ProcessedSchemas,
) -> Result<()> {
    let mut value = value.clone();
    reorder_schema_keys(&mut value);

    processed.insert(path, &value);

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
