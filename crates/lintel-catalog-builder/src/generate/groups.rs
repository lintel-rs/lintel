use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use lintel_schema_cache::SchemaCache;
use schema_catalog::SchemaEntry;
use tracing::info;

use crate::download::{ProcessedSchemas, fetch_one};
use crate::refs::{RefRewriteContext, resolve_and_rewrite, resolve_and_rewrite_value};
use lintel_catalog_builder::config::SchemaDefinition;

use super::GenerateContext;
use super::util::{
    extract_lintel_meta, extract_schema_meta, first_line, prefetch_versions,
    process_fetched_versions, resolve_latest_id,
};

/// Context for processing a single group schema entry.
pub(super) struct GroupSchemaContext<'a> {
    pub(super) generate: &'a GenerateContext<'a>,
    pub(super) group_dir: &'a Path,
    pub(super) group_key: &'a str,
    pub(super) trimmed_base: &'a str,
    pub(super) processed: &'a ProcessedSchemas,
    /// Base URL for local schema sources (e.g. raw GitHub URL).
    pub(super) source_base_url: Option<&'a str>,
}

/// Build the `x-lintel` source identifier for a local schema.
///
/// If `source_base_url` is configured, returns a full URL like
/// `https://raw.githubusercontent.com/.../schemas/group/key.json`.
/// Otherwise returns the relative path.
fn local_source_id(ctx: &GroupSchemaContext<'_>, relative_path: &str) -> String {
    if let Some(base) = ctx.source_base_url {
        format!("{}/{relative_path}", base.trim_end_matches('/'))
    } else {
        relative_path.to_string()
    }
}

/// Process a single group schema entry: fetch schema + versions concurrently,
/// then resolve refs and process versions.
#[allow(clippy::too_many_lines)]
pub(super) async fn process_group_schema(
    ctx: &GroupSchemaContext<'_>,
    key: &str,
    schema_def: &SchemaDefinition,
    output_paths: &mut HashSet<PathBuf>,
) -> Result<SchemaEntry> {
    let entry_dir = ctx.group_dir.join(key);
    tokio::fs::create_dir_all(&entry_dir).await?;

    let dest_path = entry_dir.join("latest.json");

    // Collision detection against the schema directory
    let canonical_dest = entry_dir
        .canonicalize()
        .unwrap_or_else(|_| entry_dir.clone());
    if !output_paths.insert(canonical_dest) {
        bail!(
            "output path collision: {} (group={}, key={key})",
            entry_dir.display(),
            ctx.group_key,
        );
    }

    let schema_url = format!(
        "{}/schemas/{}/{key}/latest.json",
        ctx.trimmed_base, ctx.group_key
    );

    // Fetch schema (if remote) and all versions concurrently
    let (schema_fetch_result, version_results) = tokio::join!(
        async {
            if let Some(url) = &schema_def.url {
                Some(fetch_one(ctx.generate.cache, url).await.with_context(|| {
                    format!("failed to download schema for {}/{key}", ctx.group_key)
                }))
            } else {
                None
            }
        },
        prefetch_versions(ctx.generate.cache, &schema_def.versions),
    );

    // Per-schema shared dir and ref-rewrite state
    let shared_dir = entry_dir.join("_shared");
    let shared_base_url = format!(
        "{}/schemas/{}/{key}/_shared",
        ctx.trimmed_base, ctx.group_key
    );
    let mut already_downloaded: HashMap<String, String> = HashMap::new();

    // For local schemas, pre-compute source identifier and content hash
    let lintel_source = if schema_def.url.is_none() {
        let relative_source = format!("schemas/{}/{key}.json", ctx.group_key);
        let source_path = ctx.generate.config_dir.join(&relative_source);
        if !source_path.exists() {
            bail!(
                "local schema not found: {} (expected for group={}, key={key})",
                source_path.display(),
                ctx.group_key,
            );
        }
        let text = tokio::fs::read_to_string(&source_path)
            .await
            .with_context(|| format!("failed to read local schema {}", source_path.display()))?;
        let source_id = local_source_id(ctx, &relative_source);
        let hash = SchemaCache::hash_content(&text);
        Some((source_id, hash, text))
    } else {
        None
    };

    // Resolve file_match + parsers: config takes priority, then fall back to schema metadata.
    // For local schemas we can extract from the text now; for remote schemas we
    // update ref_ctx after fetching.
    let mut file_match = schema_def.file_match.clone();
    let mut parsers = Vec::new();
    if file_match.is_empty()
        && let Some((_, _, text)) = &lintel_source
        && let Ok(val) = serde_json::from_str::<serde_json::Value>(text)
    {
        (file_match, parsers) = extract_lintel_meta(&val);
    }

    let mut ref_ctx = RefRewriteContext {
        cache: ctx.generate.cache,
        shared_dir: &shared_dir,
        base_url_for_shared: &shared_base_url,
        already_downloaded: &mut already_downloaded,
        source_url: schema_def.url.clone(),
        processed: ctx.processed,
        lintel_source: lintel_source
            .as_ref()
            .map(|(id, hash, _)| (id.clone(), hash.clone())),
        file_match: file_match.clone(),
        parsers,
    };

    // Process schema result
    let schema_text = if let Some(url) = &schema_def.url {
        let (mut value, status) =
            schema_fetch_result.expect("fetch result must exist when URL is present")?;
        info!(url = %url, status = %status, "downloaded group schema");

        // Extract fileMatch + parsers from fetched schema if config didn't provide any
        if ref_ctx.file_match.is_empty() {
            let (schema_file_match, schema_parsers) = extract_lintel_meta(&value);
            if !schema_file_match.is_empty() {
                file_match = schema_file_match;
                ref_ctx.file_match.clone_from(&file_match);
                ref_ctx.parsers = schema_parsers;
            }
        }

        // Use version URL as $id if latest content matches a version
        let schema_base_url = format!("{}/schemas/{}/{key}", ctx.trimmed_base, ctx.group_key,);
        let resolved_url = resolve_latest_id(
            ctx.generate.cache,
            url,
            &version_results,
            &schema_url,
            &schema_base_url,
        );

        resolve_and_rewrite_value(&mut ref_ctx, &mut value, &dest_path, &resolved_url).await?;
        serde_json::to_string_pretty(&value)?
    } else {
        let (_, _, text) = lintel_source
            .as_ref()
            .expect("computed above for local schemas");
        resolve_and_rewrite(&mut ref_ctx, text, &dest_path, &schema_url).await?;
        text.clone()
    };

    // Process pre-fetched versions
    let version_urls = process_fetched_versions(&mut ref_ctx, &entry_dir, version_results).await?;

    // Auto-populate name and description from the JSON Schema
    let (schema_title, schema_desc) = extract_schema_meta(&schema_text);
    let name = schema_def
        .name
        .clone()
        .or_else(|| schema_title.clone())
        .unwrap_or_else(|| key.to_string());
    let description = schema_def
        .description
        .clone()
        .or(schema_title)
        .or_else(|| schema_desc.as_deref().map(first_line))
        .unwrap_or_default();

    Ok(SchemaEntry {
        name,
        description,
        url: schema_url,
        source_url: schema_def.url.clone(),
        file_match,
        versions: version_urls,
    })
}
