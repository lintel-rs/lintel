use alloc::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use lintel_schema_cache::SchemaCache;
use schema_catalog::SchemaEntry;
use tracing::{info, warn};

use crate::download::download_one;
use crate::refs::{RefRewriteContext, resolve_and_rewrite};
use lintel_catalog_builder::config::SchemaDefinition;

use super::GenerateContext;
use super::util::extract_schema_meta;

/// Context for processing a single group schema entry.
pub(super) struct GroupSchemaContext<'a> {
    pub(super) generate: &'a GenerateContext<'a>,
    pub(super) group_dir: &'a Path,
    pub(super) group_key: &'a str,
    pub(super) trimmed_base: &'a str,
}

/// Process a single group schema entry: download, resolve refs, fetch versions.
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

    // Per-schema shared dir and ref-rewrite state
    let shared_dir = entry_dir.join("_shared");
    let shared_base_url = format!(
        "{}/schemas/{}/{key}/_shared",
        ctx.trimmed_base, ctx.group_key
    );
    let mut already_downloaded: HashMap<String, String> = HashMap::new();
    let mut ref_ctx = RefRewriteContext {
        cache: ctx.generate.cache,
        shared_dir: &shared_dir,
        base_url_for_shared: &shared_base_url,
        already_downloaded: &mut already_downloaded,
    };

    // Fetch schema (remote or local)
    let schema_text = if let Some(url) = &schema_def.url {
        let (text, status) = download_one(ctx.generate.cache, url, &dest_path)
            .await
            .with_context(|| format!("failed to download schema for {}/{key}", ctx.group_key))?;
        info!(url = %url, status = %status, "downloaded group schema");
        resolve_and_rewrite(&mut ref_ctx, &text, &dest_path, &schema_url).await?;
        text
    } else {
        let source_path = ctx
            .generate
            .config_dir
            .join("schemas")
            .join(ctx.group_key)
            .join(format!("{key}.json"));
        if !source_path.exists() {
            bail!(
                "local schema not found: {} (expected for group={}, key={key})",
                source_path.display(),
                ctx.group_key,
            );
        }
        let text: String = tokio::fs::read_to_string(&source_path)
            .await
            .with_context(|| format!("failed to read local schema {}", source_path.display()))?;
        resolve_and_rewrite(&mut ref_ctx, &text, &dest_path, &schema_url).await?;
        text
    };

    // Download versions
    let version_urls = download_versions(
        ctx.generate.cache,
        &mut ref_ctx,
        &entry_dir,
        &schema_def.versions,
    )
    .await?;

    // Auto-populate name and description from the JSON Schema
    let (schema_title, schema_desc) = extract_schema_meta(&schema_text);
    let name = schema_def
        .name
        .clone()
        .or(schema_title)
        .unwrap_or_else(|| key.to_string());
    let description = schema_def
        .description
        .clone()
        .or(schema_desc)
        .unwrap_or_default();

    Ok(SchemaEntry {
        name,
        description,
        url: schema_url,
        source_url: schema_def.url.clone(),
        file_match: schema_def.file_match.clone(),
        versions: version_urls,
    })
}

/// Download each declared version of a schema into `<entry_dir>/versions/` and
/// resolve `$ref` dependencies. Returns a map of version name -> local URL.
///
/// The local URLs are derived from `ref_ctx.base_url_for_shared` by replacing
/// `_shared` with `versions/<name>.json`.
async fn download_versions(
    cache: &SchemaCache,
    ref_ctx: &mut RefRewriteContext<'_>,
    entry_dir: &Path,
    versions: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let mut version_urls: BTreeMap<String, String> = BTreeMap::new();
    if versions.is_empty() {
        return Ok(version_urls);
    }
    tokio::fs::create_dir_all(entry_dir.join("versions")).await?;

    // Derive the versions base URL from the shared base URL (sibling directory)
    let versions_base_url = ref_ctx
        .base_url_for_shared
        .trim_end_matches("/_shared")
        .to_string()
        + "/versions";

    for (version_name, version_url) in versions {
        let version_dest = entry_dir
            .join("versions")
            .join(format!("{version_name}.json"));
        let version_local_url = format!("{versions_base_url}/{version_name}.json");

        match download_one(cache, version_url, &version_dest).await {
            Ok((text, status)) => {
                info!(
                    url = %version_url,
                    version = %version_name,
                    status = %status,
                    "downloaded schema version"
                );
                resolve_and_rewrite(ref_ctx, &text, &version_dest, &version_local_url).await?;
                version_urls.insert(version_name.clone(), version_local_url);
            }
            Err(e) => {
                warn!(
                    url = %version_url,
                    version = %version_name,
                    error = %e,
                    "failed to download version, keeping upstream URL"
                );
                version_urls.insert(version_name.clone(), version_url.clone());
            }
        }
    }

    Ok(version_urls)
}
