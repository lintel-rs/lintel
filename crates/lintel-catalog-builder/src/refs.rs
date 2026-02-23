use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use lintel_schema_cache::SchemaCache;
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use tracing::{debug, info, warn};

/// A downloaded `$ref` dependency pending recursive processing.
struct DownloadedDep {
    text: String,
    filename: String,
    url: String,
}

/// Shared context for [`resolve_and_rewrite`], grouping cross-cutting state
/// that would otherwise require many individual arguments.
pub struct RefRewriteContext<'a> {
    pub cache: &'a SchemaCache,
    pub shared_dir: &'a Path,
    pub base_url_for_shared: &'a str,
    pub already_downloaded: &'a mut HashMap<String, String>,
}

/// Characters that must be percent-encoded in URI fragment components.
///
/// Per RFC 3986, fragments may contain: `pchar / "/" / "?"` where
/// `pchar = unreserved / pct-encoded / sub-delims / ":" / "@"`.
///
/// This set encodes everything that is NOT allowed in a fragment.
const FRAGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'<')
    .add(b'>')
    .add(b'[')
    .add(b']')
    .add(b'{')
    .add(b'}')
    .add(b'|')
    .add(b'\\')
    .add(b'^')
    .add(b'"')
    .add(b'`');

/// Recursively scan a JSON value for `$ref` strings that are absolute HTTP(S) URLs.
/// Returns the set of base URLs (fragments stripped).
pub fn find_external_refs(value: &serde_json::Value) -> HashSet<String> {
    let mut refs = HashSet::new();
    collect_refs(value, &mut refs);
    refs
}

fn collect_refs(value: &serde_json::Value, refs: &mut HashSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(ref_str)) = map.get("$ref")
                && (ref_str.starts_with("http://") || ref_str.starts_with("https://"))
            {
                // Strip fragment
                let base = ref_str.split('#').next().unwrap_or(ref_str);
                if !base.is_empty() {
                    refs.insert(base.to_string());
                }
            }
            for v in map.values() {
                collect_refs(v, refs);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_refs(v, refs);
            }
        }
        _ => {}
    }
}

/// Extract a filename from a URL's last path segment.
///
/// Falls back to the domain name with `.json` extension if the URL has no
/// meaningful path segments (e.g. `https://meta.json-schema.tools`).
///
/// # Errors
///
/// Returns an error if the URL cannot be parsed.
pub fn filename_from_url(url: &str) -> Result<String> {
    let parsed = url::Url::parse(url).with_context(|| format!("invalid URL: {url}"))?;
    let segments: Vec<&str> = parsed
        .path_segments()
        .map(Iterator::collect)
        .unwrap_or_default();
    if let Some(last) = segments.last().filter(|s| !s.is_empty()) {
        return Ok((*last).to_string());
    }
    // No path segments — use domain as filename
    let host = parsed
        .host_str()
        .with_context(|| format!("URL has no host: {url}"))?;
    Ok(format!("{host}.json"))
}

/// Generate a unique filename in a directory, adding numeric suffixes on collision.
///
/// Given `base.json`, tries `base.json`, `base-2.json`, `base-3.json`, etc.
fn unique_filename_in(dir: &Path, base: &str) -> String {
    if !dir.join(base).exists() {
        return base.to_string();
    }
    let (stem, ext) = match base.rfind('.') {
        Some(pos) => (&base[..pos], &base[pos..]),
        None => (base, ""),
    };
    let mut n = 2u32;
    loop {
        let candidate = format!("{stem}-{n}{ext}");
        if !dir.join(&candidate).exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Rewrite `$ref` URLs in a JSON value using the provided mapping.
/// Preserves fragments (e.g. `#/definitions/Foo`).
pub fn rewrite_refs(value: &mut serde_json::Value, url_map: &HashMap<String, String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(ref_str)) = map.get("$ref") {
                let (base, fragment) = match ref_str.split_once('#') {
                    Some((b, f)) => (b, Some(f)),
                    None => (ref_str.as_str(), None),
                };
                if let Some(new_base) = url_map.get(base) {
                    let new_ref = match fragment {
                        Some(f) => format!("{new_base}#{f}"),
                        None => new_base.clone(),
                    };
                    map.insert("$ref".to_string(), serde_json::Value::String(new_ref));
                }
            }
            for v in map.values_mut() {
                rewrite_refs(v, url_map);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                rewrite_refs(v, url_map);
            }
        }
        _ => {}
    }
}

/// Percent-encode invalid characters in `$ref` URI references.
///
/// Many schemas in the wild use definition names with spaces, brackets, angle
/// brackets, etc. that are not valid in URI references per RFC 3986. This
/// function fixes them by percent-encoding the offending characters in the
/// fragment portion of `$ref` values.
pub fn fix_ref_uris(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(ref_str)) = map.get("$ref")
                && let Some(new_ref) = encode_ref_fragment(ref_str)
            {
                map.insert("$ref".to_string(), serde_json::Value::String(new_ref));
            }
            for v in map.values_mut() {
                fix_ref_uris(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                fix_ref_uris(v);
            }
        }
        _ => {}
    }
}

/// Encode invalid characters in a `$ref` fragment. Returns `None` if no
/// encoding is needed.
fn encode_ref_fragment(ref_str: &str) -> Option<String> {
    let (base, fragment) = ref_str.split_once('#')?;

    // Encode each JSON Pointer segment individually, preserving `/` separators
    let encoded_fragment: String = fragment
        .split('/')
        .map(|segment| utf8_percent_encode(segment, FRAGMENT_ENCODE_SET).to_string())
        .collect::<Vec<_>>()
        .join("/");

    if encoded_fragment == fragment {
        return None;
    }

    Some(format!("{base}#{encoded_fragment}"))
}

/// Download all `$ref` dependencies for a schema, rewrite URLs to local paths,
/// and write the updated schema. Handles transitive dependencies recursively.
///
/// - `ctx`: shared context containing the schema cache, shared directory,
///   base URL for the shared directory, and already-downloaded map
/// - `schema_text`: the JSON text of the schema
/// - `schema_dest`: where to write the rewritten schema
///
/// Filenames in `_shared/` are prefixed with the parent schema stem
/// (e.g. `github-workflow--schema.json`) and disambiguated with numeric
/// suffixes when collisions remain.
pub async fn resolve_and_rewrite(
    ctx: &mut RefRewriteContext<'_>,
    schema_text: &str,
    schema_dest: &Path,
    schema_url: &str,
) -> Result<()> {
    let mut value: serde_json::Value =
        serde_json::from_str(schema_text).context("failed to parse schema JSON")?;

    // Set $id to the canonical URL where this schema will be hosted
    value
        .as_object_mut()
        .context("schema root must be an object")?
        .insert(
            "$id".to_string(),
            serde_json::Value::String(schema_url.to_string()),
        );

    jsonschema_migrate::migrate_to_2020_12(&mut value);

    let external_refs = find_external_refs(&value);
    if external_refs.is_empty() {
        // No external refs — still fix invalid URI references
        fix_ref_uris(&mut value);
        let fixed = serde_json::to_string_pretty(&value)?;
        tokio::fs::write(schema_dest, format!("{fixed}\n")).await?;
        return Ok(());
    }

    debug!(
        refs = external_refs.len(),
        "found external $ref dependencies"
    );

    // Build URL → local path mapping and download deps
    let mut url_map: HashMap<String, String> = HashMap::new();
    let mut to_process: Vec<DownloadedDep> = Vec::new();

    for ref_url in &external_refs {
        if let Some(existing_filename) = ctx.already_downloaded.get(ref_url) {
            // Already downloaded, just build the mapping using the stored filename
            let local_url = format!(
                "{}/{}",
                ctx.base_url_for_shared.trim_end_matches('/'),
                existing_filename,
            );
            url_map.insert(ref_url.clone(), local_url);
            continue;
        }

        let dep_basename = filename_from_url(ref_url)?;
        let parent_stem = schema_dest
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        let base_filename = format!("{parent_stem}--{dep_basename}");
        tokio::fs::create_dir_all(ctx.shared_dir).await?;
        // Disambiguate filename if another URL already produced the same name
        let filename = unique_filename_in(ctx.shared_dir, &base_filename);
        let dest_path = ctx.shared_dir.join(&filename);

        match crate::download::download_one(ctx.cache, ref_url, &dest_path).await {
            Ok((dep_text, status)) => {
                info!(url = %ref_url, status = %status, "downloaded $ref dependency");
                ctx.already_downloaded
                    .insert(ref_url.clone(), filename.clone());
                let local_url = format!(
                    "{}/{filename}",
                    ctx.base_url_for_shared.trim_end_matches('/')
                );
                url_map.insert(ref_url.clone(), local_url.clone());
                to_process.push(DownloadedDep {
                    text: dep_text,
                    filename,
                    url: local_url,
                });
            }
            Err(e) => {
                warn!(url = %ref_url, error = %e, "failed to download $ref dependency, keeping original URL");
            }
        }
    }

    // Rewrite refs in the main schema and fix invalid URI references
    rewrite_refs(&mut value, &url_map);
    fix_ref_uris(&mut value);
    let rewritten = serde_json::to_string_pretty(&value)?;
    tokio::fs::write(schema_dest, format!("{rewritten}\n")).await?;

    // Recursively process transitive deps
    for dep in to_process {
        let dep_dest = ctx.shared_dir.join(&dep.filename);
        Box::pin(resolve_and_rewrite(ctx, &dep.text, &dep_dest, &dep.url)).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_refs_in_simple_schema() {
        let schema = serde_json::json!({
            "$ref": "https://example.com/base.json#/definitions/Foo",
            "properties": {
                "bar": { "$ref": "https://example.com/other.json" },
                "local": { "$ref": "#/definitions/Local" }
            }
        });
        let refs = find_external_refs(&schema);
        assert_eq!(refs.len(), 2);
        assert!(refs.contains("https://example.com/base.json"));
        assert!(refs.contains("https://example.com/other.json"));
    }

    #[test]
    fn find_refs_ignores_relative() {
        let schema = serde_json::json!({
            "$ref": "#/definitions/Local",
            "items": { "$ref": "./local.json" }
        });
        let refs = find_external_refs(&schema);
        assert!(refs.is_empty());
    }

    #[test]
    fn find_refs_in_arrays() {
        let schema = serde_json::json!({
            "oneOf": [
                { "$ref": "https://a.com/one.json" },
                { "$ref": "https://b.com/two.json#/defs/X" }
            ]
        });
        let refs = find_external_refs(&schema);
        assert_eq!(refs.len(), 2);
        assert!(refs.contains("https://a.com/one.json"));
        assert!(refs.contains("https://b.com/two.json"));
    }

    #[test]
    fn filename_from_url_extracts_last_segment() {
        assert_eq!(
            filename_from_url("https://example.com/schemas/foo.json").expect("ok"),
            "foo.json"
        );
    }

    #[test]
    fn filename_from_url_with_path() {
        assert_eq!(
            filename_from_url("https://example.com/a/b/c/my-schema.json").expect("ok"),
            "my-schema.json"
        );
    }

    #[test]
    fn rewrite_refs_replaces_mapped_urls() {
        let mut schema = serde_json::json!({
            "$ref": "https://example.com/base.json#/definitions/Foo",
            "properties": {
                "bar": { "$ref": "https://example.com/other.json" },
                "local": { "$ref": "#/definitions/Local" }
            }
        });
        let url_map: HashMap<String, String> = [
            (
                "https://example.com/base.json".to_string(),
                "_shared/base.json".to_string(),
            ),
            (
                "https://example.com/other.json".to_string(),
                "_shared/other.json".to_string(),
            ),
        ]
        .into_iter()
        .collect();

        rewrite_refs(&mut schema, &url_map);

        assert_eq!(schema["$ref"], "_shared/base.json#/definitions/Foo");
        assert_eq!(schema["properties"]["bar"]["$ref"], "_shared/other.json");
        // Local refs are untouched
        assert_eq!(schema["properties"]["local"]["$ref"], "#/definitions/Local");
    }

    #[test]
    fn encode_ref_fragment_with_spaces() {
        assert_eq!(
            encode_ref_fragment("#/$defs/Parameter Node"),
            Some("#/$defs/Parameter%20Node".to_string())
        );
    }

    #[test]
    fn encode_ref_fragment_with_brackets() {
        assert_eq!(
            encode_ref_fragment("#/$defs/ConfigTranslated[string]"),
            Some("#/$defs/ConfigTranslated%5Bstring%5D".to_string())
        );
    }

    #[test]
    fn encode_ref_fragment_with_angle_brackets() {
        assert_eq!(
            encode_ref_fragment("#/definitions/Dictionary<any>"),
            Some("#/definitions/Dictionary%3Cany%3E".to_string())
        );
    }

    #[test]
    fn encode_ref_fragment_with_pipe() {
        assert_eq!(
            encode_ref_fragment("#/definitions/k8s.io|api|core|v1.TaintEffect"),
            Some("#/definitions/k8s.io%7Capi%7Ccore%7Cv1.TaintEffect".to_string())
        );
    }

    #[test]
    fn encode_ref_fragment_valid_unchanged() {
        // Already-valid refs should return None (no change needed)
        assert_eq!(encode_ref_fragment("#/definitions/Foo"), None);
        assert_eq!(encode_ref_fragment("#/$defs/bar-baz"), None);
    }

    #[test]
    fn encode_ref_no_fragment() {
        assert_eq!(encode_ref_fragment("https://example.com/foo.json"), None);
    }

    #[test]
    fn fix_ref_uris_encodes_spaces_in_schema() {
        let mut schema = serde_json::json!({
            "oneOf": [
                { "$ref": "#/$defs/Parameter Node" },
                { "$ref": "#/$defs/Event Node" }
            ],
            "properties": {
                "ok": { "$ref": "#/definitions/Valid" }
            }
        });
        fix_ref_uris(&mut schema);
        assert_eq!(schema["oneOf"][0]["$ref"], "#/$defs/Parameter%20Node");
        assert_eq!(schema["oneOf"][1]["$ref"], "#/$defs/Event%20Node");
        assert_eq!(schema["properties"]["ok"]["$ref"], "#/definitions/Valid");
    }

    #[test]
    fn fix_ref_uris_encodes_complex_rust_types() {
        let mut schema = serde_json::json!({
            "$ref": "#/definitions/core::option::Option<vector::template::Template>"
        });
        fix_ref_uris(&mut schema);
        assert_eq!(
            schema["$ref"],
            "#/definitions/core::option::Option%3Cvector::template::Template%3E"
        );
    }
}
