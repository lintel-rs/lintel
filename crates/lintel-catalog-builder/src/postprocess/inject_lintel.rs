use alloc::collections::BTreeSet;

use lintel_schema_cache::SchemaCache;
use schema_catalog::FileFormat;
use tracing::error;

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
    let compile_err = try_compile_schema(value);
    if let Some(ref e) = compile_err {
        error!(source = %extra.source, error = %e, "schema is invalid after transformation");
    }
    extra.invalid = compile_err.is_some();
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

/// Try to compile a JSON Schema value, returning the error if it fails.
///
/// Uses a no-op retriever so external `$ref`s resolve to permissive schemas
/// without network I/O. Errors from external ref resolution (pointer/anchor
/// "does not exist") are ignored since the `NoopRetriever` cannot provide
/// the actual document structure for fragment navigation.
fn try_compile_schema(value: &serde_json::Value) -> Option<jsonschema::ValidationError<'static>> {
    let err = jsonschema::options()
        .with_retriever(NoopRetriever)
        .build(value)
        .err()?;

    // Filter out false-positive errors caused by limitations of schema
    // compilation without full external reference resolution:
    //
    // - "does not exist": JSON Pointer or $anchor lookups against the
    //   NoopRetriever's boolean `true` schema (has no navigable structure).
    //
    // - "is not of type": Meta-schema self-validation issues (e.g. draft
    //   2019-09 `$vocabulary` has boolean values that fail type checks when
    //   compiled under 2020-12).
    //
    // - "is not valid under any of the schemas listed in the 'anyOf'":
    //   Non-standard type values in source schemas (e.g. `"float"`,
    //   `"ConnectorMetric"`) that the jsonschema crate rejects.
    //
    // - "does not match": Pattern validation failures on $ref URI values
    //   (e.g. nested definition paths that don't match `^[^#]*#?$`).
    let msg = err.to_string();
    if msg.contains("does not exist")
        || msg.contains("is not of type")
        || msg.contains("is not valid under any of the schemas listed in the")
        || msg.contains("does not match")
    {
        return None;
    }

    Some(err)
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
