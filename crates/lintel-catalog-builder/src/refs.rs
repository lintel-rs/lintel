use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use tracing::{debug, warn};

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
/// # Errors
///
/// Returns an error if the URL cannot be parsed or has no path segments.
pub fn filename_from_url(url: &str) -> Result<String> {
    let parsed = url::Url::parse(url).with_context(|| format!("invalid URL: {url}"))?;
    let segments: Vec<&str> = parsed
        .path_segments()
        .map(Iterator::collect)
        .unwrap_or_default();
    let last = segments
        .last()
        .filter(|s| !s.is_empty())
        .with_context(|| format!("URL has no path segments: {url}"))?;
    Ok((*last).to_string())
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

/// Download all `$ref` dependencies for a schema, rewrite URLs to local paths,
/// and write the updated schema. Handles transitive dependencies recursively.
///
/// - `schema_text`: the JSON text of the schema
/// - `schema_dest`: where to write the rewritten schema
/// - `shared_dir`: directory for dependency files (e.g. `schemas/<group>/_shared/`)
/// - `base_url_for_shared`: the URL prefix for the shared directory
/// - `already_downloaded`: set of URLs already downloaded (to avoid re-downloading)
///
/// Returns the set of URLs that were newly downloaded.
pub async fn resolve_and_rewrite(
    client: &reqwest::Client,
    schema_text: &str,
    schema_dest: &Path,
    shared_dir: &Path,
    base_url_for_shared: &str,
    already_downloaded: &mut HashSet<String>,
) -> Result<()> {
    let mut value: serde_json::Value =
        serde_json::from_str(schema_text).context("failed to parse schema JSON")?;

    let external_refs = find_external_refs(&value);
    if external_refs.is_empty() {
        // No external refs, just write as-is
        tokio::fs::write(schema_dest, schema_text).await?;
        return Ok(());
    }

    debug!(
        refs = external_refs.len(),
        "found external $ref dependencies"
    );

    // Build URL → local path mapping and download deps
    let mut url_map: HashMap<String, String> = HashMap::new();
    let mut to_process: Vec<(String, String)> = Vec::new(); // (url, local_text)

    for ref_url in &external_refs {
        if already_downloaded.contains(ref_url) {
            // Already downloaded, just build the mapping
            let filename = filename_from_url(ref_url)?;
            let local_url = format!("{}/{}", base_url_for_shared.trim_end_matches('/'), filename);
            url_map.insert(ref_url.clone(), local_url);
            continue;
        }

        let filename = filename_from_url(ref_url)?;
        let dest_path = shared_dir.join(&filename);

        // Check for filename collisions in shared dir
        if dest_path.exists() && !already_downloaded.contains(ref_url) {
            bail!("filename collision in _shared/: {filename} (from URL: {ref_url})");
        }

        tokio::fs::create_dir_all(shared_dir).await?;
        match crate::download::download_one(client, ref_url, &dest_path).await {
            Ok(dep_text) => {
                already_downloaded.insert(ref_url.clone());
                let local_url =
                    format!("{}/{}", base_url_for_shared.trim_end_matches('/'), filename);
                url_map.insert(ref_url.clone(), local_url);
                to_process.push((ref_url.clone(), dep_text));
            }
            Err(e) => {
                warn!(url = %ref_url, error = %e, "failed to download $ref dependency, keeping original URL");
            }
        }
    }

    // Rewrite refs in the main schema
    rewrite_refs(&mut value, &url_map);
    let rewritten = serde_json::to_string_pretty(&value)?;
    tokio::fs::write(schema_dest, format!("{rewritten}\n")).await?;

    // Recursively process transitive deps
    for (dep_url, dep_text) in to_process {
        let dep_filename = filename_from_url(&dep_url)?;
        let dep_dest = shared_dir.join(&dep_filename);
        Box::pin(resolve_and_rewrite(
            client,
            &dep_text,
            &dep_dest,
            shared_dir,
            base_url_for_shared,
            already_downloaded,
        ))
        .await?;
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
}
