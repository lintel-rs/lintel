use alloc::collections::BTreeSet;

use lintel_schema_cache::SchemaCache;
use schema_catalog::FileFormat;
use tracing::warn;

use super::PostprocessContext;
use crate::download::{LintelExtra, parse_lintel_extra};

/// Inject `x-lintel` metadata into a schema value.
///
/// Uses `lintel_source` (pre-computed source + hash) if available, otherwise
/// falls back to cache-based injection from `source_url`.
pub(super) fn inject_lintel(ctx: &PostprocessContext<'_>, value: &mut serde_json::Value) {
    let parsers = if ctx.parsers.is_empty() {
        parsers_from_file_match(&ctx.file_match)
    } else {
        ctx.parsers.clone()
    };
    if let Some((source, hash)) = &ctx.lintel_source {
        inject_lintel_extra(
            value,
            LintelExtra {
                source: source.clone(),
                source_sha256: hash.clone(),
                invalid: false,
                file_match: ctx.file_match.clone(),
                parsers,
                catalog_description: None,
            },
        );
    } else if let Some(ref source_url) = ctx.source_url {
        inject_lintel_extra_from_cache(
            value,
            ctx.cache,
            LintelExtra {
                source: source_url.clone(),
                source_sha256: String::new(),
                invalid: false,
                file_match: ctx.file_match.clone(),
                parsers,
                catalog_description: None,
            },
        );
    }
}

/// Inject `x-lintel` metadata into a schema's root object.
///
/// Validates the schema and sets `invalid: true` if it fails compilation.
fn inject_lintel_extra(value: &mut serde_json::Value, mut extra: LintelExtra) {
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
/// no metadata is injected.
fn inject_lintel_extra_from_cache(
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

/// Derive file formats from `fileMatch` glob patterns by inspecting extensions.
///
/// Returns a sorted, deduplicated list of formats.
fn parsers_from_file_match(patterns: &[String]) -> Vec<FileFormat> {
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
