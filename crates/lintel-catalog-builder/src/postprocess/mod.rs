/// Post-processing transformations applied to JSON Schema values before they
/// are written to disk: ref URI fixups, metadata injection, key reordering,
/// description link conversion, etc.
mod fix_ref_uris;
mod inject_lintel;
mod linkify_descriptions;
mod reorder_schema_keys;

use fix_ref_uris::fix_ref_uris;
use inject_lintel::inject_lintel;
use linkify_descriptions::linkify_descriptions;
use reorder_schema_keys::reorder_schema_keys;

use lintel_schema_cache::SchemaCache;
use schema_catalog::FileFormat;

/// Context for post-processing transformations that require build-time state.
///
/// When provided to [`postprocess_schema`], enables `x-lintel` metadata
/// injection into the schema.
pub struct PostprocessContext<'a> {
    /// Schema cache, used for content-hash lookups.
    pub cache: &'a SchemaCache,
    /// Original source URL the schema was fetched from.
    pub source_url: Option<String>,
    /// Pre-computed source identifier and SHA-256 hash for local schemas.
    /// Format: `(source_identifier, sha256_hex)`.
    pub lintel_source: Option<(String, String)>,
    /// Glob patterns from the catalog entry.
    pub file_match: Vec<String>,
    /// Explicit parsers; when empty, derived from `file_match` extensions.
    pub parsers: Vec<FileFormat>,
}

/// Apply all post-processing transformations to a JSON Schema value.
///
/// Steps (in order):
/// 1. Percent-encode invalid characters in `$ref` fragment URIs.
/// 2. Inject `x-lintel` metadata.
/// 3. Reorder top-level keys so well-known fields come first.
/// 4. Wrap bare URLs in `description` fields as Markdown autolinks.
pub fn postprocess_schema(ctx: &PostprocessContext<'_>, value: &mut serde_json::Value) {
    fix_ref_uris(value);
    inject_lintel(ctx, value);
    reorder_schema_keys(value);
    linkify_descriptions(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorders_and_linkifies() {
        let cache = lintel_schema_cache::SchemaCache::memory();
        let ctx = PostprocessContext {
            cache: &cache,
            source_url: None,
            lintel_source: None,
            file_match: Vec::new(),
            parsers: Vec::new(),
        };
        let mut schema = serde_json::json!({
            "type": "object",
            "description": "See https://example.com",
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Test"
        });
        postprocess_schema(&ctx, &mut schema);

        let keys: Vec<&String> = schema
            .as_object()
            .expect("test value is an object")
            .keys()
            .collect();
        assert_eq!(keys, &["$schema", "title", "description", "type"]);

        assert_eq!(schema["description"], "See <https://example.com>");
    }
}
