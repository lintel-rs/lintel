use alloc::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use lintel_schema_cache::SchemaCache;
use schema_catalog::SchemaEntry;
use tracing::{debug, info, warn};

use crate::download::{DownloadItem, download_batch};
use crate::refs::{RefRewriteContext, resolve_and_rewrite};
use lintel_catalog_builder::config::{OrganizeEntry, SourceConfig};

use super::util::slugify;

/// Per-target processing context passed to source-level functions.
pub(super) struct SourceContext<'a> {
    pub(super) cache: &'a SchemaCache,
    pub(super) base_url: &'a str,
    pub(super) schemas_dir: &'a Path,
    pub(super) concurrency: usize,
}

/// Aggregated download results for a source, used by [`resolve_source_refs`].
struct SourceDownloadResult<'a> {
    items: &'a [DownloadItem],
    info: &'a [SourceSchemaInfo],
    downloaded: &'a HashSet<String>,
}

/// Information about a source schema being processed.
struct SourceSchemaInfo {
    name: String,
    description: String,
    url: String,
    local_url: String,
    file_match: Vec<String>,
    versions: BTreeMap<String, String>,
    slug: String,
    target_dir: String,
}

/// Metadata for a version download, linking it back to its parent schema.
struct VersionDownloadMeta {
    entry_index: usize,
    version_name: String,
    local_url: String,
}

/// Process a single external source (e.g. `SchemaStore`).
///
/// Returns `(entries, groups)` where groups are `(group_key, schema_names)`.
#[allow(clippy::too_many_lines)]
pub(super) async fn process_source(
    ctx: &SourceContext<'_>,
    source_name: &str,
    source_config: &SourceConfig,
    output_paths: &mut HashSet<PathBuf>,
) -> Result<(Vec<SchemaEntry>, Vec<(String, Vec<String>)>)> {
    // Fetch and parse the external catalog
    info!(url = %source_config.url, "fetching source catalog");
    let (catalog_value, _status) = ctx
        .cache
        .fetch(&source_config.url)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let source_catalog: schema_catalog::Catalog = serde_json::from_value(catalog_value)
        .with_context(|| format!("failed to parse source catalog from {}", source_config.url))?;

    info!(
        schemas = source_catalog.schemas.len(),
        "source catalog parsed"
    );

    let base_url = ctx.base_url.trim_end_matches('/');
    let source_dir = ctx.schemas_dir.join(source_name);
    tokio::fs::create_dir_all(&source_dir).await?;

    // Create organize directories
    for dir_name in source_config.organize.keys() {
        tokio::fs::create_dir_all(ctx.schemas_dir.join(dir_name)).await?;
    }

    // Classify each schema into an organize directory or the source default.
    let mut download_items: Vec<DownloadItem> = Vec::new();
    let mut entry_info: Vec<SourceSchemaInfo> = Vec::new();
    let mut dir_slug_counts: HashMap<(String, String), usize> = HashMap::new();
    let mut seen_urls: HashSet<String> = HashSet::new();
    // Track which schemas belong to which organize group
    let mut organize_schemas: HashMap<String, Vec<String>> = HashMap::new();

    for schema in &source_catalog.schemas {
        // Skip duplicate URLs within the same source catalog
        if !seen_urls.insert(schema.url.clone()) {
            continue;
        }

        let target_dir = classify_schema(schema, &source_config.organize, source_name)?;
        let base_slug = slugify(&schema.name);

        // Deduplicate directory names within the same parent
        let key = (target_dir.clone(), base_slug.clone());
        let count = dir_slug_counts.entry(key).or_insert(0);
        *count += 1;
        let slug = if *count == 1 {
            base_slug
        } else {
            format!("{}-{}", slugify(&schema.name), count)
        };

        let schema_dir = ctx.schemas_dir.join(&target_dir).join(&slug);
        let dest_path = schema_dir.join("latest.json");

        // Final collision detection against schema directory
        let canonical_dest = schema_dir
            .canonicalize()
            .unwrap_or_else(|_| schema_dir.clone());
        if !output_paths.insert(canonical_dest) {
            bail!(
                "output path collision: {} (source={source_name}, schema={})",
                schema_dir.display(),
                schema.name,
            );
        }

        // Track organize group membership
        if target_dir != source_name {
            organize_schemas
                .entry(target_dir.clone())
                .or_default()
                .push(schema.name.clone());
        }

        let local_url = format!("{base_url}/schemas/{target_dir}/{slug}/latest.json");
        download_items.push(DownloadItem {
            url: schema.url.clone(),
            dest: dest_path,
        });
        entry_info.push(SourceSchemaInfo {
            name: schema.name.clone(),
            description: schema.description.clone(),
            url: schema.url.clone(),
            local_url,
            file_match: schema.file_match.clone(),
            versions: schema.versions.clone(),
            slug: slug.clone(),
            target_dir: target_dir.clone(),
        });
    }

    // Download all latest schemas concurrently
    info!(
        count = download_items.len(),
        concurrency = ctx.concurrency,
        "downloading source schemas"
    );
    let downloaded = download_batch(ctx.cache, &download_items, ctx.concurrency).await?;

    info!(
        downloaded = downloaded.len(),
        skipped = download_items.len() - downloaded.len(),
        "source download complete"
    );

    // Resolve $ref deps for downloaded schemas (per-schema shared dirs)
    let result = SourceDownloadResult {
        items: &download_items,
        info: &entry_info,
        downloaded: &downloaded,
    };
    resolve_source_refs(ctx, &result).await?;

    // Download, resolve refs, and build local URL maps for all schema versions
    let local_version_maps = download_source_versions(ctx, &entry_info, &downloaded).await?;

    // Build catalog entries
    let entries = entry_info
        .iter()
        .enumerate()
        .map(|(i, info)| {
            let url = if downloaded.contains(&info.url) {
                info.local_url.clone()
            } else {
                warn!(schema = %info.name, "using upstream URL (download was skipped)");
                info.url.clone()
            };
            let versions = if local_version_maps[i].is_empty() {
                info.versions.clone()
            } else {
                local_version_maps[i].clone()
            };
            SchemaEntry {
                name: info.name.clone(),
                description: info.description.clone(),
                url,
                source_url: Some(info.url.clone()),
                file_match: info.file_match.clone(),
                versions,
            }
        })
        .collect();

    // Build organize groups (key + matched schema names)
    let groups: Vec<(String, Vec<String>)> = source_config
        .organize
        .keys()
        .filter_map(|dir_name| {
            let schemas = organize_schemas.remove(dir_name)?;
            Some((dir_name.clone(), schemas))
        })
        .collect();

    Ok((entries, groups))
}

/// Resolve `$ref` dependencies for all downloaded source schemas.
///
/// Uses per-schema `_shared` directories.
async fn resolve_source_refs(
    ctx: &SourceContext<'_>,
    result: &SourceDownloadResult<'_>,
) -> Result<()> {
    let base_url = ctx.base_url.trim_end_matches('/');

    for (item, info) in result.items.iter().zip(result.info.iter()) {
        if !result.downloaded.contains(&item.url) {
            continue;
        }

        let schema_dir = ctx.schemas_dir.join(&info.target_dir).join(&info.slug);
        let shared_dir = schema_dir.join("_shared");
        let shared_base_url = format!(
            "{base_url}/schemas/{}/{}/_shared",
            info.target_dir, info.slug,
        );
        let mut already_downloaded: HashMap<String, String> = HashMap::new();

        let text = tokio::fs::read_to_string(&item.dest).await?;
        // Always run resolve_and_rewrite: it handles both external $ref
        // resolution and fixing invalid URI references (spaces, brackets, etc.)
        debug!(schema = %info.name, "processing schema refs");
        resolve_and_rewrite(
            &mut RefRewriteContext {
                cache: ctx.cache,
                shared_dir: &shared_dir,
                base_url_for_shared: &shared_base_url,
                already_downloaded: &mut already_downloaded,
                source_url: Some(item.url.clone()),
            },
            &text,
            &item.dest,
            &info.local_url,
        )
        .await
        .with_context(|| format!("failed to process refs for {}", info.name))?;
    }

    Ok(())
}

/// Download all schema versions, resolve their refs, and return per-entry
/// local version URL maps.
async fn download_source_versions(
    ctx: &SourceContext<'_>,
    entry_info: &[SourceSchemaInfo],
    downloaded: &HashSet<String>,
) -> Result<Vec<BTreeMap<String, String>>> {
    let base_url = ctx.base_url.trim_end_matches('/');

    // Collect version download items
    let mut version_download_items: Vec<DownloadItem> = Vec::new();
    let mut version_meta: Vec<VersionDownloadMeta> = Vec::new();
    for (i, info) in entry_info.iter().enumerate() {
        if !downloaded.contains(&info.url) {
            continue;
        }
        for (version_name, version_url) in &info.versions {
            let versions_dir = ctx
                .schemas_dir
                .join(&info.target_dir)
                .join(&info.slug)
                .join("versions");
            let version_dest = versions_dir.join(format!("{version_name}.json"));
            let version_local_url = format!(
                "{base_url}/schemas/{}/{}/versions/{version_name}.json",
                info.target_dir, info.slug,
            );
            version_download_items.push(DownloadItem {
                url: version_url.clone(),
                dest: version_dest,
            });
            version_meta.push(VersionDownloadMeta {
                entry_index: i,
                version_name: version_name.clone(),
                local_url: version_local_url,
            });
        }
    }

    // Batch download versions
    let version_downloaded = if version_download_items.is_empty() {
        HashSet::new()
    } else {
        info!(
            count = version_download_items.len(),
            "downloading source schema versions"
        );
        let dl = download_batch(ctx.cache, &version_download_items, ctx.concurrency).await?;
        info!(
            downloaded = dl.len(),
            skipped = version_download_items.len() - dl.len(),
            "version download complete"
        );
        dl
    };

    // Resolve refs for downloaded versions
    for (vi, vitem) in version_download_items.iter().enumerate() {
        if !version_downloaded.contains(&vitem.url) {
            continue;
        }
        let meta = &version_meta[vi];
        let info = &entry_info[meta.entry_index];
        let entry_dir = ctx.schemas_dir.join(&info.target_dir).join(&info.slug);
        let shared_dir = entry_dir.join("_shared");
        let shared_base_url = format!(
            "{base_url}/schemas/{}/{}/_shared",
            info.target_dir, info.slug,
        );
        let mut already_downloaded: HashMap<String, String> = HashMap::new();
        let text = tokio::fs::read_to_string(&vitem.dest).await?;
        resolve_and_rewrite(
            &mut RefRewriteContext {
                cache: ctx.cache,
                shared_dir: &shared_dir,
                base_url_for_shared: &shared_base_url,
                already_downloaded: &mut already_downloaded,
                source_url: Some(vitem.url.clone()),
            },
            &text,
            &vitem.dest,
            &meta.local_url,
        )
        .await
        .with_context(|| {
            format!(
                "failed to process refs for {} version {}",
                info.name, meta.version_name,
            )
        })?;
    }

    // Build local version URL maps per entry
    let mut local_version_maps: Vec<BTreeMap<String, String>> =
        vec![BTreeMap::new(); entry_info.len()];
    for (vi, vitem) in version_download_items.iter().enumerate() {
        let meta = &version_meta[vi];
        let local_url = if version_downloaded.contains(&vitem.url) {
            meta.local_url.clone()
        } else {
            vitem.url.clone()
        };
        local_version_maps[meta.entry_index].insert(meta.version_name.clone(), local_url);
    }

    Ok(local_version_maps)
}

/// Classify a schema into an organize directory or the source default.
///
/// Returns the directory name relative to `schemas/`.
///
/// # Errors
///
/// Returns an error if a schema matches multiple organize entries (ambiguous).
fn classify_schema(
    schema: &schema_catalog::SchemaEntry,
    organize: &BTreeMap<String, OrganizeEntry>,
    source_name: &str,
) -> Result<String> {
    let mut matched_dir: Option<&str> = None;

    for (dir_name, entry) in organize {
        for matcher in &entry.match_patterns {
            let matches = if matcher.starts_with("http://") || matcher.starts_with("https://") {
                // URL exact match
                schema.url == *matcher
            } else {
                // Glob match against fileMatch patterns (as literal strings).
                schema
                    .file_match
                    .iter()
                    .any(|fm| organize_glob_matches(matcher, fm))
            };

            if matches {
                if let Some(existing) = matched_dir {
                    if existing != dir_name.as_str() {
                        bail!(
                            "schema '{}' matches multiple organize entries: '{existing}' and '{dir_name}'",
                            schema.name,
                        );
                    }
                } else {
                    matched_dir = Some(dir_name);
                }
            }
        }
    }

    Ok(matched_dir.map_or_else(|| source_name.to_string(), String::from))
}

/// Match an organize glob pattern against a fileMatch string (treated as literal text).
///
/// Unlike `glob_match::glob_match`, this treats `**` as matching any characters
/// including path separators. The literal parts between `**` segments must
/// appear in order in the text.
///
/// Examples:
/// - `**.github**` matches `**/.github/workflows/*.yml` (contains `.github`)
/// - `**.github**` matches `.github/dependabot.yml` (contains `.github`)
fn organize_glob_matches(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split("**").collect();

    let mut remaining = text;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 && !pattern.starts_with("**") {
            // First literal part must match at the start of the text
            if let Some(rest) = remaining.strip_prefix(part) {
                remaining = rest;
            } else {
                return false;
            }
        } else if let Some(pos) = remaining.find(part) {
            remaining = &remaining[pos + part.len()..];
        } else {
            return false;
        }
    }

    // If pattern doesn't end with **, remaining text must be empty
    if !pattern.ends_with("**") && !remaining.is_empty() {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_schema(name: &str, url: &str, file_match: Vec<&str>) -> schema_catalog::SchemaEntry {
        schema_catalog::SchemaEntry {
            name: name.into(),
            description: String::new(),
            url: url.into(),
            source_url: None,
            file_match: file_match.into_iter().map(String::from).collect(),
            versions: BTreeMap::new(),
        }
    }

    fn make_organize(matchers: Vec<&str>) -> OrganizeEntry {
        OrganizeEntry {
            match_patterns: matchers.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn classify_no_organize_uses_source_name() {
        let schema = make_schema("Test", "https://example.com/test.json", vec!["test.json"]);
        let organize = BTreeMap::new();
        assert_eq!(
            classify_schema(&schema, &organize, "mysource").expect("ok"),
            "mysource"
        );
    }

    #[test]
    fn classify_glob_match() {
        let schema = make_schema(
            "GitHub Workflow",
            "https://example.com/github-workflow.json",
            vec!["**/.github/workflows/*.yml"],
        );
        let mut organize = BTreeMap::new();
        organize.insert("github".to_string(), make_organize(vec!["**.github**"]));
        assert_eq!(
            classify_schema(&schema, &organize, "schemastore").expect("ok"),
            "github"
        );
    }

    #[test]
    fn classify_url_exact_match() {
        let schema = make_schema(
            "Special",
            "https://example.com/special.json",
            vec!["special.json"],
        );
        let mut organize = BTreeMap::new();
        organize.insert(
            "special-dir".to_string(),
            make_organize(vec!["https://example.com/special.json"]),
        );
        assert_eq!(
            classify_schema(&schema, &organize, "source").expect("ok"),
            "special-dir"
        );
    }

    #[test]
    fn classify_ambiguous_is_error() {
        let schema = make_schema(
            "Ambiguous",
            "https://example.com/ambig.json",
            vec!["**/.github/workflows/*.yml"],
        );
        let mut organize = BTreeMap::new();
        organize.insert("dir1".to_string(), make_organize(vec!["**.github**"]));
        organize.insert("dir2".to_string(), make_organize(vec!["**.github**"]));
        assert!(classify_schema(&schema, &organize, "source").is_err());
    }

    #[test]
    fn classify_no_match_uses_default() {
        let schema = make_schema(
            "Unmatched",
            "https://example.com/unmatched.json",
            vec!["unmatched.yaml"],
        );
        let mut organize = BTreeMap::new();
        organize.insert("github".to_string(), make_organize(vec!["**.github**"]));
        assert_eq!(
            classify_schema(&schema, &organize, "schemastore").expect("ok"),
            "schemastore"
        );
    }
}
