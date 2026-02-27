use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use futures_util::stream::StreamExt;
use lintel_schema_cache::SchemaCache;
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use tracing::{debug, info, warn};

use crate::download::ProcessedSchemas;

/// Shared context for [`resolve_and_rewrite`], grouping cross-cutting state
/// that would otherwise require many individual arguments.
pub struct RefRewriteContext<'a> {
    pub cache: &'a SchemaCache,
    pub shared_dir: &'a Path,
    pub base_url_for_shared: &'a str,
    pub already_downloaded: &'a mut HashMap<String, String>,
    /// Original source URL of the schema being processed. Used to resolve
    /// relative `$ref` values (e.g. `./rule.json`) against the schema's origin.
    pub source_url: Option<String>,
    pub processed: &'a ProcessedSchemas,
    /// Pre-computed source identifier and SHA-256 hash for `x-lintel` injection.
    ///
    /// Used for local schemas that aren't in the HTTP cache. When set, this
    /// takes priority over cache-based injection from `source_url`.
    /// Format: `(source_identifier, sha256_hex)`.
    pub lintel_source: Option<(String, String)>,
    /// Glob patterns from the catalog entry, injected into `x-lintel.fileMatch`.
    pub file_match: Vec<String>,
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

/// Recursively scan a JSON value for `$ref` strings that are relative file
/// references (e.g. `./rule.json`, `../other.json`).  Returns the set of
/// relative paths (fragments stripped).  Internal `#/…` refs are excluded.
pub fn find_relative_refs(value: &serde_json::Value) -> HashSet<String> {
    let mut refs = HashSet::new();
    collect_relative_refs(value, &mut refs);
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

fn collect_relative_refs(value: &serde_json::Value, refs: &mut HashSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(ref_str)) = map.get("$ref") {
                let base = ref_str.split('#').next().unwrap_or(ref_str);
                // Relative ref: not empty, not a fragment-only ref, not an absolute URL
                if !base.is_empty() && !base.starts_with("http://") && !base.starts_with("https://")
                {
                    refs.insert(base.to_string());
                }
            }
            for v in map.values() {
                collect_relative_refs(v, refs);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_relative_refs(v, refs);
            }
        }
        _ => {}
    }
}

/// Resolve a relative path against a base URL.
///
/// For example, resolving `./rule.json` against
/// `https://example.com/schemas/project.json` yields
/// `https://example.com/schemas/rule.json`.
fn resolve_relative_url(relative: &str, base_url: &str) -> Result<String> {
    let base =
        url::Url::parse(base_url).with_context(|| format!("invalid base URL: {base_url}"))?;
    let resolved = base
        .join(relative)
        .with_context(|| format!("failed to resolve '{relative}' against '{base_url}'"))?;
    Ok(resolved.to_string())
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
        let name = (*last).to_string();
        // Ensure .json extension so the HTML generator can safely strip it
        // to create a directory path that won't collide with the file.
        if std::path::Path::new(&name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            return Ok(name);
        }
        return Ok(format!("{name}.json"));
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

/// Resolve all relative `$ref` paths in a schema against a source URL.
///
/// Returns a map from relative path → absolute URL for each successfully resolved
/// ref.  When `source_url` is `None` or resolution fails for a particular ref,
/// that ref is skipped with a debug/warning log.
fn resolve_all_relative_refs(
    value: &serde_json::Value,
    source_url: Option<&str>,
) -> HashMap<String, String> {
    let relative_refs = find_relative_refs(value);
    let mut resolved: HashMap<String, String> = HashMap::new();
    if let Some(source_url) = source_url {
        for rel_ref in &relative_refs {
            match resolve_relative_url(rel_ref, source_url) {
                Ok(abs_url) => {
                    debug!(relative = %rel_ref, resolved = %abs_url, "resolved relative $ref");
                    resolved.insert(rel_ref.clone(), abs_url);
                }
                Err(e) => {
                    warn!(relative = %rel_ref, error = %e, "failed to resolve relative $ref");
                }
            }
        }
    } else if !relative_refs.is_empty() {
        debug!(
            count = relative_refs.len(),
            "skipping relative $ref resolution (no source URL)"
        );
    }
    resolved
}

/// Inject `x-lintel` metadata into a schema value.
///
/// Uses `lintel_source` (pre-computed source + hash) if available, otherwise
/// falls back to cache-based injection from `source_url`.
fn inject_lintel(value: &mut serde_json::Value, ctx: &RefRewriteContext<'_>) {
    let parsers = crate::download::parsers_from_file_match(&ctx.file_match);
    if let Some((source, hash)) = &ctx.lintel_source {
        crate::download::inject_lintel_extra(
            value,
            crate::download::LintelExtra {
                source: source.clone(),
                source_sha256: hash.clone(),
                invalid: false,
                file_match: ctx.file_match.clone(),
                parsers,
            },
        );
    } else if let Some(ref source_url) = ctx.source_url {
        crate::download::inject_lintel_extra_from_cache(
            value,
            ctx.cache,
            crate::download::LintelExtra {
                source: source_url.clone(),
                source_sha256: String::new(),
                invalid: false,
                file_match: ctx.file_match.clone(),
                parsers,
            },
        );
    }
}

/// Download all `$ref` dependencies for a schema, rewrite URLs to local paths,
/// and write the updated schema. Handles transitive dependencies via BFS
/// concurrent resolution.
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

    resolve_and_rewrite_value(ctx, &mut value, schema_dest, schema_url).await
}

/// Like [`resolve_and_rewrite`] but takes an already-parsed `Value`, avoiding
/// a redundant parse when the caller already has the JSON in memory.
pub async fn resolve_and_rewrite_value(
    ctx: &mut RefRewriteContext<'_>,
    value: &mut serde_json::Value,
    schema_dest: &Path,
    schema_url: &str,
) -> Result<()> {
    // Set $id to the canonical URL where this schema will be hosted
    value
        .as_object_mut()
        .context("schema root must be an object")?
        .insert(
            "$id".to_string(),
            serde_json::Value::String(schema_url.to_string()),
        );

    jsonschema_migrate::migrate_to_2020_12(value);

    let external_refs = find_external_refs(value);
    let resolved_relative = resolve_all_relative_refs(value, ctx.source_url.as_deref());

    if external_refs.is_empty() && resolved_relative.is_empty() {
        // No external refs — still fix invalid URI references
        fix_ref_uris(value);
        inject_lintel(value, ctx);
        crate::download::write_schema_json(value, schema_dest, ctx.processed).await?;
        return Ok(());
    }

    debug!(
        external = external_refs.len(),
        relative = resolved_relative.len(),
        "found $ref dependencies"
    );

    // Seed the queue with all refs from the root schema.
    let pending: Vec<(String, String, Option<String>)> = external_refs
        .iter()
        .map(|url| (url.clone(), url.clone(), Some(url.clone())))
        .chain(
            resolved_relative
                .iter()
                .map(|(rel, abs)| (rel.clone(), abs.clone(), Some(abs.clone()))),
        )
        .collect();

    let parent_stem = schema_dest
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Fetch all transitive deps concurrently via a bounded queue
    let (url_map, dep_values) = fetch_refs_queued(ctx, pending, &parent_stem).await?;

    // Rewrite refs in the root schema and write it
    rewrite_refs(value, &url_map);
    fix_ref_uris(value);
    inject_lintel(value, ctx);
    crate::download::write_schema_json(value, schema_dest, ctx.processed).await?;

    // Process and write each dependency
    write_dep_schemas(ctx, dep_values, &url_map).await?;

    Ok(())
}

/// Queue-based concurrent fetcher for `$ref` dependencies. Unlike BFS waves,
/// this starts fetching transitive deps as soon as each parent completes,
/// maintaining up to `concurrency` in-flight fetches at all times.
///
/// Returns `(url_map, dep_values)` where `url_map` maps original `$ref` keys
/// to local URLs and `dep_values` contains the fetched JSON values.
async fn fetch_refs_queued(
    ctx: &mut RefRewriteContext<'_>,
    initial: Vec<(String, String, Option<String>)>,
    parent_stem: &str,
) -> Result<(
    HashMap<String, String>,
    Vec<(String, serde_json::Value, Option<String>)>,
)> {
    let mut url_map: HashMap<String, String> = HashMap::new();
    let mut dep_values: Vec<(String, serde_json::Value, Option<String>)> = Vec::new();
    let mut pending: Vec<(String, String, Option<String>)> = initial;
    let mut in_flight = futures_util::stream::FuturesUnordered::new();
    let mut shared_dir_created = false;

    loop {
        // Drain all pending items into in_flight — the semaphore in
        // SchemaCache handles HTTP concurrency.
        while let Some((ref_key, download_url, source_url)) = pending.pop() {
            if let Some(existing_filename) = ctx.already_downloaded.get(&download_url) {
                let local_url = format!(
                    "{}/{}",
                    ctx.base_url_for_shared.trim_end_matches('/'),
                    existing_filename,
                );
                url_map.insert(ref_key, local_url);
                continue;
            }

            if !shared_dir_created {
                tokio::fs::create_dir_all(ctx.shared_dir).await?;
                shared_dir_created = true;
            }

            let dep_basename = filename_from_url(&download_url)?;
            let base_filename = format!("{parent_stem}--{dep_basename}");
            let filename = unique_filename_in(ctx.shared_dir, &base_filename);
            ctx.already_downloaded
                .insert(download_url.clone(), filename.clone());
            let local_url = format!(
                "{}/{}",
                ctx.base_url_for_shared.trim_end_matches('/'),
                filename,
            );
            url_map.insert(ref_key, local_url.clone());

            let cache = ctx.cache.clone();
            in_flight.push(async move {
                let result = crate::download::fetch_one(&cache, &download_url).await;
                (download_url, filename, local_url, source_url, result)
            });
        }

        // Nothing in flight and nothing pending → done
        if in_flight.is_empty() {
            break;
        }

        // Wait for the next completed fetch (safe: we checked !is_empty above)
        let Some((download_url, filename, local_url, source_url, result)) = in_flight.next().await
        else {
            break;
        };

        match result {
            Ok((dep_value, status)) => {
                info!(url = %download_url, status = %status, "downloaded $ref dependency");

                // Immediately enqueue transitive refs (will be submitted next iteration)
                for url in find_external_refs(&dep_value) {
                    if !ctx.already_downloaded.contains_key(&url) {
                        pending.push((url.clone(), url.clone(), Some(url.clone())));
                    }
                }
                for (rel, abs) in resolve_all_relative_refs(&dep_value, source_url.as_deref()) {
                    if !ctx.already_downloaded.contains_key(&abs) {
                        pending.push((rel, abs.clone(), Some(abs)));
                    }
                }

                dep_values.push((filename, dep_value, source_url));
            }
            Err(e) => {
                warn!(url = %download_url, error = %e, "failed to download $ref dependency, keeping original URL");
                ctx.already_downloaded.remove(&download_url);
                url_map.retain(|_, v| v != &local_url);
            }
        }
    }

    Ok((url_map, dep_values))
}

/// Set `$id`, migrate, rewrite `$ref`s, fix URIs, and write each dependency to
/// disk.
async fn write_dep_schemas(
    ctx: &RefRewriteContext<'_>,
    dep_values: Vec<(String, serde_json::Value, Option<String>)>,
    url_map: &HashMap<String, String>,
) -> Result<()> {
    for (filename, mut dep_value, source_url) in dep_values {
        let dep_dest = ctx.shared_dir.join(&filename);
        let dep_local_url = format!(
            "{}/{}",
            ctx.base_url_for_shared.trim_end_matches('/'),
            filename,
        );

        // Resolve any relative refs in the dep against its source URL
        let dep_relative = resolve_all_relative_refs(&dep_value, source_url.as_deref());
        // Build a combined url_map for this dep: inherited + its own relative refs
        let mut dep_url_map = url_map.clone();
        for (rel, abs) in &dep_relative {
            if let Some(existing_filename) = ctx.already_downloaded.get(abs) {
                let local_url = format!(
                    "{}/{}",
                    ctx.base_url_for_shared.trim_end_matches('/'),
                    existing_filename,
                );
                dep_url_map.insert(rel.clone(), local_url);
            }
        }

        if let Some(obj) = dep_value.as_object_mut() {
            obj.insert("$id".to_string(), serde_json::Value::String(dep_local_url));
        }
        jsonschema_migrate::migrate_to_2020_12(&mut dep_value);
        rewrite_refs(&mut dep_value, &dep_url_map);
        fix_ref_uris(&mut dep_value);
        if let Some(ref source_url) = source_url {
            crate::download::inject_lintel_extra_from_cache(
                &mut dep_value,
                ctx.cache,
                crate::download::LintelExtra {
                    source: source_url.clone(),
                    source_sha256: String::new(),
                    invalid: false,
                    file_match: Vec::new(),
                    parsers: Vec::new(),
                },
            );
        }
        crate::download::write_schema_json(&dep_value, &dep_dest, ctx.processed).await?;
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
    fn filename_from_url_appends_json_extension() {
        assert_eq!(
            filename_from_url("https://example.com/version/1").expect("ok"),
            "1.json"
        );
        assert_eq!(
            filename_from_url("https://example.com/schemas/feed-1").expect("ok"),
            "feed-1.json"
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

    // --- find_relative_refs ---

    #[test]
    fn find_relative_refs_dot_slash() {
        let schema = serde_json::json!({
            "properties": {
                "rule": { "$ref": "./rule.json#/$defs/SerializableRule" }
            }
        });
        let refs = find_relative_refs(&schema);
        assert_eq!(refs.len(), 1);
        assert!(refs.contains("./rule.json"));
    }

    #[test]
    fn find_relative_refs_ignores_fragment_only() {
        let schema = serde_json::json!({
            "$ref": "#/definitions/Foo",
            "items": { "$ref": "#/$defs/Bar" }
        });
        let refs = find_relative_refs(&schema);
        assert!(refs.is_empty());
    }

    #[test]
    fn find_relative_refs_ignores_http() {
        let schema = serde_json::json!({
            "$ref": "https://example.com/schema.json"
        });
        let refs = find_relative_refs(&schema);
        assert!(refs.is_empty());
    }

    #[test]
    fn find_relative_refs_various_patterns() {
        let schema = serde_json::json!({
            "oneOf": [
                { "$ref": "./a.json" },
                { "$ref": "../b.json#/defs/X" },
                { "$ref": "subdir/c.json" }
            ]
        });
        let refs = find_relative_refs(&schema);
        assert_eq!(refs.len(), 3);
        assert!(refs.contains("./a.json"));
        assert!(refs.contains("../b.json"));
        assert!(refs.contains("subdir/c.json"));
    }

    // --- resolve_relative_url ---

    #[test]
    fn resolve_relative_dot_slash() {
        let result = resolve_relative_url(
            "./rule.json",
            "https://raw.githubusercontent.com/ast-grep/ast-grep/main/schemas/project.json",
        )
        .expect("ok");
        assert_eq!(
            result,
            "https://raw.githubusercontent.com/ast-grep/ast-grep/main/schemas/rule.json"
        );
    }

    #[test]
    fn resolve_relative_parent_dir() {
        let result = resolve_relative_url(
            "../other/schema.json",
            "https://example.com/schemas/sub/main.json",
        )
        .expect("ok");
        assert_eq!(result, "https://example.com/schemas/other/schema.json");
    }

    #[test]
    fn resolve_relative_bare_filename() {
        let result = resolve_relative_url("types.json", "https://example.com/schemas/main.json")
            .expect("ok");
        assert_eq!(result, "https://example.com/schemas/types.json");
    }

    // --- rewrite_refs with relative refs ---

    #[test]
    fn rewrite_refs_replaces_relative_refs() {
        let mut schema = serde_json::json!({
            "properties": {
                "rule": { "$ref": "./rule.json#/$defs/SerializableRule" },
                "local": { "$ref": "#/definitions/Local" }
            }
        });
        let url_map: HashMap<String, String> = [(
            "./rule.json".to_string(),
            "_shared/project--rule.json".to_string(),
        )]
        .into_iter()
        .collect();

        rewrite_refs(&mut schema, &url_map);

        assert_eq!(
            schema["properties"]["rule"]["$ref"],
            "_shared/project--rule.json#/$defs/SerializableRule"
        );
        assert_eq!(schema["properties"]["local"]["$ref"], "#/definitions/Local");
    }
}
