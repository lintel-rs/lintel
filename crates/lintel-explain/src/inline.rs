//! Inline external `$ref` URIs into root `$defs`.
//!
//! When a schema uses relative or absolute `$ref` URIs pointing to external
//! schemas (e.g. `"meta/core"`), this module fetches those schemas and bundles
//! them into the root `$defs` object. The original `$ref` values are rewritten
//! to local `#/$defs/<name>` pointers so the existing rendering code can
//! resolve them.

use alloc::collections::BTreeMap;
use std::collections::HashSet;

use anyhow::Result;
use serde_json::Value;
use url::Url;

use lintel_schema_cache::SchemaCache;

/// Maximum number of resolution passes (to handle transitive external refs).
const MAX_PASSES: usize = 2;

/// Inline all external `$ref` URIs in `value` by fetching them and adding
/// them to root `$defs`. Rewrites the `$ref` values to `#/$defs/<name>`.
///
/// Performs up to [`MAX_PASSES`] passes to resolve transitive refs.
pub async fn inline_external_refs(
    value: &mut Value,
    schema_uri: &str,
    cache: &SchemaCache,
) -> Result<()> {
    let mut seen: HashSet<String> = HashSet::new();

    for _ in 0..MAX_PASSES {
        let Some(base) = extract_base_uri(value, schema_uri) else {
            break;
        };

        let external_uris = collect_all_external_ref_uris(value, &base);
        // Filter to only URIs we haven't processed yet.
        let new_uris: Vec<String> = external_uris
            .into_iter()
            .filter(|u| seen.insert(u.clone()))
            .collect();

        if new_uris.is_empty() {
            break;
        }

        // Fetch all external schemas.
        let mut fetched: BTreeMap<String, Value> = BTreeMap::new();
        for uri in &new_uris {
            match cache.fetch(uri).await {
                Ok((val, _)) => {
                    fetched.insert(uri.clone(), val);
                }
                Err(e) => {
                    tracing::warn!(uri, error = %e, "failed to fetch external $ref");
                }
            }
        }

        if fetched.is_empty() {
            break;
        }

        // Hoist sub-schema $defs into the root.
        for fetched_val in fetched.values() {
            hoist_defs(value, fetched_val);
        }

        // Bundle fetched schemas into root $defs and rewrite refs.
        bundle_into_defs(value, &fetched, &base);
    }

    Ok(())
}

/// Extract a base URI from the schema's `$id` field, falling back to
/// `schema_uri`.
fn extract_base_uri(value: &Value, schema_uri: &str) -> Option<Url> {
    let id = value
        .as_object()
        .and_then(|o| o.get("$id"))
        .and_then(Value::as_str);

    let base_str = id.unwrap_or(schema_uri);
    Url::parse(base_str).ok()
}

/// Resolve a `$ref` string against a base URL.
///
/// Returns `None` for local refs (starting with `#`).
fn resolve_ref_uri(ref_str: &str, base: &Url) -> Option<String> {
    if ref_str.starts_with('#') {
        return None;
    }

    // Try parsing as absolute URL first.
    if let Ok(abs) = Url::parse(ref_str) {
        return Some(abs.to_string());
    }

    // Resolve relative URI against base.
    base.join(ref_str).ok().map(|u| u.to_string())
}

/// Recursively collect all external `$ref` URI strings in the tree.
fn collect_all_external_ref_uris(value: &Value, base: &Url) -> Vec<String> {
    let mut uris = Vec::new();
    let mut seen = HashSet::new();
    collect_refs_recursive(value, base, &mut uris, &mut seen);
    uris
}

fn collect_refs_recursive(
    value: &Value,
    base: &Url,
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match value {
        Value::Object(map) => {
            if let Some(ref_val) = map.get("$ref").and_then(Value::as_str)
                && let Some(resolved) = resolve_ref_uri(ref_val, base)
                && seen.insert(resolved.clone())
            {
                out.push(resolved);
            }
            for v in map.values() {
                collect_refs_recursive(v, base, out, seen);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_refs_recursive(v, base, out, seen);
            }
        }
        _ => {}
    }
}

/// Merge `$defs` from a fetched schema into the root schema's `$defs`.
///
/// Uses first-writer-wins: if a def already exists in root, it is not
/// overwritten.
fn hoist_defs(root: &mut Value, fetched: &Value) {
    let fetched_defs = fetched
        .as_object()
        .and_then(|o| o.get("$defs"))
        .and_then(Value::as_object);

    let Some(fetched_defs) = fetched_defs else {
        return;
    };

    let Some(root_obj) = root.as_object_mut() else {
        return;
    };

    let root_defs = root_obj
        .entry("$defs")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));

    let Some(root_defs_map) = root_defs.as_object_mut() else {
        return;
    };

    for (key, val) in fetched_defs {
        // First-writer-wins.
        root_defs_map
            .entry(key.clone())
            .or_insert_with(|| val.clone());
    }
}

/// Add fetched schemas into root `$defs` with proper names, set
/// `x-lintel.source`, and rewrite `$ref` values to `#/$defs/<name>`.
fn bundle_into_defs(root: &mut Value, fetched: &BTreeMap<String, Value>, base: &Url) {
    // Build a mapping from original URI -> def name.
    let mut uri_to_def_name: BTreeMap<String, String> = BTreeMap::new();

    let Some(root_obj) = root.as_object_mut() else {
        return;
    };

    let root_defs = root_obj
        .entry("$defs")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));

    let Some(root_defs_map) = root_defs.as_object_mut() else {
        return;
    };

    for (uri, fetched_val) in fetched {
        let name = def_name_for_schema(fetched_val, uri);
        // Add x-lintel.source to the fetched schema.
        let mut schema_copy = fetched_val.clone();
        if let Some(obj) = schema_copy.as_object_mut() {
            let x_lintel = obj
                .entry("x-lintel")
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let Some(xl_obj) = x_lintel.as_object_mut() {
                xl_obj
                    .entry("source".to_string())
                    .or_insert_with(|| Value::String(uri.clone()));
            }
        }

        // First-writer-wins for the def name.
        root_defs_map.entry(name.clone()).or_insert(schema_copy);

        uri_to_def_name.insert(uri.clone(), name);
    }

    // Now rewrite all $ref values in the tree that point to these URIs.
    rewrite_refs(root, &uri_to_def_name, base);
}

/// Derive a definition name from a fetched schema.
///
/// Uses the schema's `title` if available, otherwise the last path segment
/// of the URL.
fn def_name_for_schema(fetched_value: &Value, url: &str) -> String {
    if let Some(title) = fetched_value
        .as_object()
        .and_then(|o| o.get("title"))
        .and_then(Value::as_str)
        && !title.is_empty()
    {
        return title.to_string();
    }

    // Fall back to last path segment of URL.
    Url::parse(url)
        .ok()
        .and_then(|u| {
            u.path_segments()
                .and_then(|mut segs| segs.next_back().map(String::from))
        })
        .unwrap_or_else(|| url.to_string())
}

/// Rewrite `$ref` values in the tree from external URIs to `#/$defs/<name>`.
fn rewrite_refs(value: &mut Value, uri_to_name: &BTreeMap<String, String>, base: &Url) {
    match value {
        Value::Object(map) => {
            if let Some(ref_val) = map.get("$ref").and_then(Value::as_str).map(String::from)
                && let Some(resolved) = resolve_ref_uri(&ref_val, base)
                && let Some(name) = uri_to_name.get(&resolved)
            {
                map.insert("$ref".to_string(), Value::String(format!("#/$defs/{name}")));
            }
            for v in map.values_mut() {
                rewrite_refs(v, uri_to_name, base);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                rewrite_refs(v, uri_to_name, base);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_base_uri_from_id() {
        let value = json!({
            "$id": "https://json-schema.org/draft/2020-12/schema"
        });
        let base = extract_base_uri(&value, "https://example.com/fallback").unwrap();
        assert_eq!(
            base.as_str(),
            "https://json-schema.org/draft/2020-12/schema"
        );
    }

    #[test]
    fn extract_base_uri_fallback() {
        let value = json!({ "type": "object" });
        let base = extract_base_uri(&value, "https://example.com/schema.json").unwrap();
        assert_eq!(base.as_str(), "https://example.com/schema.json");
    }

    #[test]
    fn resolve_ref_uri_relative() {
        let base = Url::parse("https://json-schema.org/draft/2020-12/schema").unwrap();
        let resolved = resolve_ref_uri("meta/core", &base).unwrap();
        assert_eq!(resolved, "https://json-schema.org/draft/2020-12/meta/core");
    }

    #[test]
    fn resolve_ref_uri_absolute() {
        let base = Url::parse("https://example.com/schema").unwrap();
        let resolved = resolve_ref_uri("https://other.com/schema.json", &base).unwrap();
        assert_eq!(resolved, "https://other.com/schema.json");
    }

    #[test]
    fn resolve_ref_uri_local_returns_none() {
        let base = Url::parse("https://example.com/schema").unwrap();
        assert!(resolve_ref_uri("#/$defs/foo", &base).is_none());
        assert!(resolve_ref_uri("#/properties/bar", &base).is_none());
    }

    #[test]
    fn collect_external_refs() {
        let base = Url::parse("https://json-schema.org/draft/2020-12/schema").unwrap();
        let value = json!({
            "allOf": [
                { "$ref": "meta/core" },
                { "$ref": "meta/applicator" },
                { "$ref": "#/$defs/local" }
            ],
            "$defs": {
                "local": { "type": "string" }
            }
        });
        let refs = collect_all_external_ref_uris(&value, &base);
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(&"https://json-schema.org/draft/2020-12/meta/core".to_string()));
        assert!(
            refs.contains(&"https://json-schema.org/draft/2020-12/meta/applicator".to_string())
        );
    }

    #[test]
    fn hoist_defs_merges() {
        let mut root = json!({
            "$defs": {
                "existing": { "type": "string" }
            }
        });
        let fetched = json!({
            "$defs": {
                "new_def": { "type": "integer" },
                "existing": { "type": "boolean" }
            }
        });
        hoist_defs(&mut root, &fetched);

        let defs = root["$defs"].as_object().unwrap();
        assert_eq!(defs.len(), 2);
        // existing should not be overwritten (first-writer-wins)
        assert_eq!(defs["existing"]["type"], "string");
        assert_eq!(defs["new_def"]["type"], "integer");
    }

    #[test]
    fn hoist_defs_creates_root_defs_if_missing() {
        let mut root = json!({ "type": "object" });
        let fetched = json!({
            "$defs": {
                "new_def": { "type": "integer" }
            }
        });
        hoist_defs(&mut root, &fetched);

        let defs = root["$defs"].as_object().unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs["new_def"]["type"], "integer");
    }

    #[test]
    fn def_name_uses_title() {
        let schema = json!({
            "title": "Core vocabulary meta-schema",
            "type": "object"
        });
        assert_eq!(
            def_name_for_schema(&schema, "https://example.com/meta/core"),
            "Core vocabulary meta-schema"
        );
    }

    #[test]
    fn def_name_falls_back_to_url_segment() {
        let schema = json!({ "type": "object" });
        assert_eq!(
            def_name_for_schema(&schema, "https://example.com/meta/core"),
            "core"
        );
    }

    #[test]
    fn bundle_and_rewrite() {
        let base = Url::parse("https://json-schema.org/draft/2020-12/schema").unwrap();
        let mut root = json!({
            "$id": "https://json-schema.org/draft/2020-12/schema",
            "allOf": [
                { "$ref": "meta/core" }
            ]
        });
        let mut fetched = BTreeMap::new();
        fetched.insert(
            "https://json-schema.org/draft/2020-12/meta/core".to_string(),
            json!({
                "title": "Core vocabulary meta-schema",
                "$defs": {
                    "anchorString": { "type": "string" }
                }
            }),
        );

        bundle_into_defs(&mut root, &fetched, &base);

        // $ref should be rewritten
        assert_eq!(
            root["allOf"][0]["$ref"],
            "#/$defs/Core vocabulary meta-schema"
        );

        // Definition should exist with x-lintel.source
        let defs = root["$defs"].as_object().unwrap();
        assert!(defs.contains_key("Core vocabulary meta-schema"));
        let core_def = &defs["Core vocabulary meta-schema"];
        assert_eq!(
            core_def["x-lintel"]["source"],
            "https://json-schema.org/draft/2020-12/meta/core"
        );
    }

    #[tokio::test]
    async fn inline_external_refs_with_memory_cache() {
        let cache = SchemaCache::memory();
        cache.insert(
            "https://example.com/meta/core",
            json!({
                "title": "Core Schema",
                "type": "object",
                "properties": {
                    "id": { "type": "string" }
                },
                "$defs": {
                    "idString": { "type": "string", "format": "uri" }
                }
            }),
        );

        let mut root = json!({
            "$id": "https://example.com/schema",
            "allOf": [
                { "$ref": "meta/core" }
            ]
        });

        inline_external_refs(&mut root, "https://example.com/schema", &cache)
            .await
            .unwrap();

        // $ref should be rewritten to local
        assert_eq!(root["allOf"][0]["$ref"], "#/$defs/Core Schema");

        // Definition should be present
        let defs = root["$defs"].as_object().unwrap();
        assert!(defs.contains_key("Core Schema"));
        assert!(defs.contains_key("idString"));

        // x-lintel.source should be set
        assert_eq!(
            defs["Core Schema"]["x-lintel"]["source"],
            "https://example.com/meta/core"
        );
    }

    #[tokio::test]
    async fn inline_no_external_refs_is_noop() {
        let cache = SchemaCache::memory();
        let mut root = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        });
        let original = root.clone();

        inline_external_refs(&mut root, "https://example.com/schema", &cache)
            .await
            .unwrap();

        assert_eq!(root, original);
    }
}
