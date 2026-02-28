use alloc::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use futures_util::stream::{self, StreamExt};
use lintel_schema_cache::SchemaCache;
use schema_catalog::SchemaEntry;
use tracing::{debug, info, warn};

use crate::download::{ProcessedSchemas, fetch_one};
use crate::refs::{RefRewriteContext, resolve_and_rewrite_value};
use lintel_catalog_builder::config::{OrganizeEntry, SourceConfig};

use super::util::{
    extract_lintel_meta, prefetch_versions, process_fetched_versions, resolve_latest_id, slugify,
};

/// Per-target processing context passed to source-level functions.
pub(super) struct SourceContext<'a> {
    pub(super) cache: &'a SchemaCache,
    pub(super) base_url: &'a str,
    pub(super) schemas_dir: &'a Path,
    pub(super) processed: &'a ProcessedSchemas,
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

        // Skip schemas whose fileMatch entries match any exclude-matches pattern
        if !source_config.exclude_matches.is_empty()
            && schema.file_match.iter().any(|fm| {
                source_config
                    .exclude_matches
                    .iter()
                    .any(|pat| organize_glob_matches(pat, fm))
            })
        {
            debug!(schema = %schema.name, "excluded by exclude-matches");
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

        // Track group membership (including the source's own default group)
        organize_schemas
            .entry(target_dir.clone())
            .or_default()
            .push(schema.name.clone());

        let local_url = format!("{base_url}/schemas/{target_dir}/{slug}/latest.json");
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

    // Process all schemas concurrently: fetch → resolve refs → versions
    info!(count = entry_info.len(), "processing source schemas");
    let cache = ctx.cache.clone();
    let schemas_dir = ctx.schemas_dir.to_path_buf();
    let base_url_owned = base_url.to_string();
    let processed = ctx.processed.clone();

    let mut indexed_entries: Vec<(usize, SchemaEntry)> =
        stream::iter(entry_info.into_iter().enumerate())
            .map(|(i, info)| {
                let cache = cache.clone();
                let schemas_dir = schemas_dir.clone();
                let base_url = base_url_owned.clone();
                let processed = processed.clone();
                async move {
                    let ctx = SourceSchemaProcessContext {
                        cache: &cache,
                        schemas_dir: &schemas_dir,
                        base_url: &base_url,
                        processed: &processed,
                    };
                    let entry = process_one_source_schema(&ctx, info).await?;
                    Ok::<_, anyhow::Error>((i, entry))
                }
            })
            .buffer_unordered(64)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>>>()?;

    // Restore original ordering
    indexed_entries.sort_by_key(|(i, _)| *i);
    let entries: Vec<SchemaEntry> = indexed_entries.into_iter().map(|(_, e)| e).collect();

    let downloaded = entries
        .iter()
        .filter(|e| e.source_url.as_ref() != Some(&e.url))
        .count();
    let skipped = entries.len() - downloaded;
    info!(downloaded, skipped, "source processing complete");

    // Build groups: explicit organize keys + the source's own default group
    let mut groups: Vec<(String, Vec<String>)> = source_config
        .organize
        .keys()
        .filter_map(|dir_name| {
            let schemas = organize_schemas.remove(dir_name)?;
            Some((dir_name.clone(), schemas))
        })
        .collect();
    // Remaining entries are schemas that fell through to the source default
    for (key, schemas) in organize_schemas {
        groups.push((key, schemas));
    }

    Ok((entries, groups))
}

/// Shared context for processing a single source schema concurrently.
struct SourceSchemaProcessContext<'a> {
    cache: &'a SchemaCache,
    schemas_dir: &'a Path,
    base_url: &'a str,
    processed: &'a ProcessedSchemas,
}

/// Process a single source schema end-to-end: fetch latest + versions
/// concurrently, then resolve `$ref` dependencies and version refs.
async fn process_one_source_schema(
    ctx: &SourceSchemaProcessContext<'_>,
    info: SourceSchemaInfo,
) -> Result<SchemaEntry> {
    let entry_dir = ctx.schemas_dir.join(&info.target_dir).join(&info.slug);
    let dest_path = entry_dir.join("latest.json");
    let source_url = info.url.clone();

    // Fetch latest schema and all versions concurrently
    let (latest_result, version_results) = tokio::join!(
        fetch_one(ctx.cache, &source_url),
        prefetch_versions(ctx.cache, &info.versions),
    );

    let (entry_url, versions, file_match) = match latest_result {
        Ok((mut value, status)) => {
            info!(
                url = %source_url,
                status = %status,
                schema = %info.name,
                "downloaded source schema"
            );

            // Use version URL as $id if latest content matches a version
            let schema_base_url =
                format!("{}/schemas/{}/{}", ctx.base_url, info.target_dir, info.slug,);
            let schema_url = resolve_latest_id(
                ctx.cache,
                &source_url,
                &version_results,
                &info.local_url,
                &schema_base_url,
            );

            // Extract fileMatch + parsers from the schema if the catalog didn't provide any
            let (file_match, parsers) = if info.file_match.is_empty() {
                extract_lintel_meta(&value)
            } else {
                (info.file_match.clone(), Vec::new())
            };

            let shared_dir = entry_dir.join("_shared");
            let shared_base_url = format!("{schema_base_url}/_shared");
            let mut already_downloaded: HashMap<String, String> = HashMap::new();
            let mut ref_ctx = RefRewriteContext {
                cache: ctx.cache,
                shared_dir: &shared_dir,
                base_url_for_shared: &shared_base_url,
                already_downloaded: &mut already_downloaded,
                source_url: Some(source_url.clone()),
                processed: ctx.processed,
                lintel_source: None,
                file_match: file_match.clone(),
                parsers,
            };

            debug!(schema = %info.name, "processing schema refs");
            if let Err(e) =
                resolve_and_rewrite_value(&mut ref_ctx, &mut value, &dest_path, &schema_url).await
            {
                warn!(
                    url = %source_url,
                    schema = %info.name,
                    error = %e,
                    "failed to process refs, using upstream URL"
                );
                return Ok(SchemaEntry {
                    name: info.name,
                    description: info.description,
                    url: source_url.clone(),
                    source_url: Some(source_url),
                    file_match,
                    versions: info.versions,
                });
            }

            // Process pre-fetched versions
            let version_urls =
                process_fetched_versions(&mut ref_ctx, &entry_dir, version_results).await?;

            let versions = if version_urls.is_empty() {
                info.versions
            } else {
                version_urls
            };
            (info.local_url, versions, file_match)
        }
        Err(e) => {
            warn!(
                url = %source_url,
                schema = %info.name,
                error = %e,
                "failed to download source schema, skipping"
            );
            (source_url.clone(), info.versions, info.file_match)
        }
    };

    Ok(SchemaEntry {
        name: info.name,
        description: info.description,
        url: entry_url,
        source_url: Some(source_url),
        file_match,
        versions,
    })
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
