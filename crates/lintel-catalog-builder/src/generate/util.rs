use alloc::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use futures_util::stream::{self, StreamExt};
use lintel_schema_cache::{CacheStatus, SchemaCache};
use tracing::{info, warn};

use crate::download::fetch_one;
use crate::refs::{RefRewriteContext, resolve_and_rewrite_value};

/// Result of pre-fetching a single schema version: `(name, source_url, fetch_result)`.
pub(super) type VersionFetchResult = (String, String, Result<(serde_json::Value, CacheStatus)>);

/// Fetch all versions of a schema concurrently.
pub(super) async fn prefetch_versions(
    cache: &SchemaCache,
    versions: &BTreeMap<String, String>,
) -> Vec<VersionFetchResult> {
    stream::iter(versions.iter())
        .map(|(version_name, version_url)| {
            let cache = cache.clone();
            let version_name = version_name.clone();
            let version_url = version_url.clone();
            async move {
                let result = fetch_one(&cache, &version_url).await;
                (version_name, version_url, result)
            }
        })
        .buffer_unordered(64)
        .collect()
        .await
}

/// Resolve refs for pre-fetched version results.
///
/// Refs are resolved sequentially since they share the same `_shared` dir
/// and `already_downloaded` state.
pub(super) async fn process_fetched_versions(
    ref_ctx: &mut RefRewriteContext<'_>,
    schema_dir: &Path,
    fetch_results: Vec<VersionFetchResult>,
) -> Result<BTreeMap<String, String>> {
    let mut version_urls: BTreeMap<String, String> = BTreeMap::new();
    if fetch_results.is_empty() {
        return Ok(version_urls);
    }
    tokio::fs::create_dir_all(schema_dir.join("versions")).await?;

    let versions_base_url = ref_ctx
        .base_url_for_shared
        .trim_end_matches("/_shared")
        .to_string()
        + "/versions";

    for (version_name, version_url, result) in fetch_results {
        let version_dest = schema_dir
            .join("versions")
            .join(format!("{version_name}.json"));
        let version_local_url = format!("{versions_base_url}/{version_name}.json");

        match result {
            Ok((mut value, status)) => {
                info!(
                    url = %version_url,
                    version = %version_name,
                    status = %status,
                    "downloaded schema version"
                );
                // Temporarily set source_url to the version's URL so that
                // resolve_and_rewrite_value injects the correct x-lintel metadata.
                let prev_source_url = ref_ctx.source_url.replace(version_url.clone());
                resolve_and_rewrite_value(ref_ctx, &mut value, &version_dest, &version_local_url)
                    .await?;
                ref_ctx.source_url = prev_source_url;
                version_urls.insert(version_name, version_local_url);
            }
            Err(e) => {
                warn!(
                    url = %version_url,
                    version = %version_name,
                    error = %e,
                    "failed to download version, keeping upstream URL"
                );
                version_urls.insert(version_name, version_url);
            }
        }
    }

    Ok(version_urls)
}

/// If the latest schema's content hash matches any successfully-fetched
/// version's content hash, return that version's local URL as the canonical
/// `$id`. Otherwise return `default_url` (the `.../latest.json` URL).
///
/// When multiple versions match (same content), the highest version is chosen
/// using semver ordering. Non-semver version names fall back to lexicographic.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_latest_id(
    cache: &SchemaCache,
    latest_source_url: &str,
    version_results: &[VersionFetchResult],
    default_url: &str,
    schema_base_url: &str,
) -> String {
    let Some(latest_hash) = cache.content_hash(latest_source_url) else {
        return default_url.to_string();
    };
    let mut best_match: Option<&str> = None;
    for (version_name, version_url, result) in version_results {
        if result.is_ok()
            && let Some(vh) = cache.content_hash(version_url)
            && vh == latest_hash
            && best_match.is_none_or(|prev| version_gt(version_name, prev))
        {
            best_match = Some(version_name.as_str());
        }
    }
    match best_match {
        Some(version_name) => format!("{schema_base_url}/versions/{version_name}.json"),
        None => default_url.to_string(),
    }
}

/// Return `true` if `a` is a higher version than `b`.
///
/// Attempts semver parsing first; falls back to lexicographic comparison for
/// non-semver version strings.
fn version_gt(a: &str, b: &str) -> bool {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(va), Ok(vb)) => va > vb,
        _ => a > b,
    }
}

/// Extract the `title` and `description` from a JSON Schema string.
///
/// Returns `(title, description)` — either or both may be `None` if the schema
/// doesn't contain the corresponding top-level property or isn't valid JSON.
pub(super) fn extract_schema_meta(text: &str) -> (Option<String>, Option<String>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return (None, None);
    };
    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .map(String::from);
    let description = value
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from);
    (title, description)
}

/// Extract the `fileMatch` array from a JSON Schema value.
///
/// Checks `x-lintel.fileMatch` first (our own metadata), then falls back
/// to a root-level `fileMatch` array (used by some upstream schemas).
/// Returns an empty `Vec` if neither is present.
pub(super) fn extract_file_match(value: &serde_json::Value) -> Vec<String> {
    // Prefer x-lintel.fileMatch (our structured metadata)
    if let Some(extra) = crate::download::parse_lintel_extra(value)
        && !extra.file_match.is_empty()
    {
        return extra.file_match;
    }

    // Fall back to root-level fileMatch (upstream schemas)
    value
        .get("fileMatch")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Convert a key like `"github"` to title case (`"Github"`).
pub(super) fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Simple slugification for fallback filenames.
pub(super) fn slugify(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let mut result = String::new();
    let mut prev_hyphen = true;
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    result.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_simple() {
        assert_eq!(slugify("GitHub Workflow"), "github-workflow");
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!(slugify("foo/bar (baz)"), "foo-bar-baz");
    }

    #[test]
    fn version_gt_semver() {
        assert!(version_gt("2.3.15", "2.3.9"));
        assert!(version_gt("2.4.4", "2.4.3"));
        assert!(!version_gt("2.4.3", "2.4.4"));
        assert!(!version_gt("2.4.4", "2.4.4"));
        assert!(version_gt("10.0.0", "9.9.9"));
    }
}
