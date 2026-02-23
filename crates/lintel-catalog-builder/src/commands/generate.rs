use alloc::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use lintel_schema_cache::SchemaCache;
use schema_catalog::SchemaEntry;
use tracing::{debug, info, warn};

use crate::catalog::build_output_catalog;
use crate::download::{DownloadItem, download_batch, download_one};
use crate::refs::{RefRewriteContext, resolve_and_rewrite};
use crate::targets::{AnyTarget, OutputContext, Target};
use lintel_catalog_builder::config::{CatalogConfig, OrganizeEntry, SourceConfig, load_config};

/// Cross-cutting state shared across the entire generation run.
struct GenerateContext<'a> {
    cache: &'a SchemaCache,
    config: &'a CatalogConfig,
    config_path: &'a Path,
    config_dir: &'a Path,
    concurrency: usize,
}

/// Per-target processing context passed to source-level functions.
struct SourceContext<'a> {
    cache: &'a SchemaCache,
    base_url: &'a str,
    schemas_dir: &'a Path,
    concurrency: usize,
}

/// Aggregated download results for a source, used by [`resolve_source_refs`].
struct SourceDownloadResult<'a> {
    items: &'a [DownloadItem],
    info: &'a [SourceSchemaInfo],
    downloaded: &'a HashSet<String>,
}

/// Run the `generate` subcommand.
pub async fn run(
    config_path: &Path,
    target_filter: Option<&str>,
    concurrency: usize,
    no_cache: bool,
) -> Result<()> {
    // Resolve config path
    let config_path = config_path
        .canonicalize()
        .with_context(|| format!("config file not found: {}", config_path.display()))?;
    let config_dir = config_path
        .parent()
        .context("config file has no parent directory")?;

    // 1. Load config
    let config = load_catalog_config(&config_path).await?;

    if config.target.is_empty() {
        bail!("no targets defined in config; add at least one [target.<name>] section");
    }

    // Validate target filter
    if let Some(name) = target_filter
        && !config.target.contains_key(name)
    {
        bail!(
            "target '{name}' not found in config; available targets: {}",
            config
                .target
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Create schema cache
    let cache = SchemaCache::builder().force_fetch(no_cache).build();

    let ctx = GenerateContext {
        cache: &cache,
        config: &config,
        config_path: &config_path,
        config_dir,
        concurrency,
    };

    // Process each target
    for (target_name, target_config) in &config.target {
        if let Some(filter) = target_filter
            && target_name != filter
        {
            continue;
        }

        info!(target = %target_name, "building target");
        let target = AnyTarget::from(target_config.clone());
        let output_dir = target.output_dir(target_name, config_dir);

        // Clean the output directory before generating to remove stale files.
        if output_dir.exists() {
            info!(path = %output_dir.display(), "cleaning output directory");
            tokio::fs::remove_dir_all(&output_dir)
                .await
                .with_context(|| {
                    format!("failed to clean output directory {}", output_dir.display())
                })?;
        }
        tokio::fs::create_dir_all(&output_dir).await?;

        generate_for_target(&ctx, &target, &output_dir)
            .await
            .with_context(|| format!("failed to build target '{target_name}'"))?;

        info!(target = %target_name, output = %output_dir.display(), "target complete");
    }

    info!("catalog generation complete");
    Ok(())
}

/// Generate output for a single target.
#[allow(clippy::too_many_lines)]
async fn generate_for_target(
    ctx: &GenerateContext<'_>,
    target: &AnyTarget,
    output_dir: &Path,
) -> Result<()> {
    let base_url = target.base_url();
    let schemas_dir = output_dir.join("schemas");
    let mut entries: Vec<SchemaEntry> = Vec::new();
    let mut output_paths: HashSet<PathBuf> = HashSet::new();
    let mut catalog_groups: BTreeMap<String, schema_catalog::CatalogGroup> = BTreeMap::new();

    // Collect group metadata for index.html
    let mut groups_meta: BTreeMap<String, (String, String)> = BTreeMap::new();

    // 2. Process groups
    for (group_key, group_config) in &ctx.config.groups {
        info!(group = %group_key, count = group_config.schemas.len(), "processing group");
        let group_dir = schemas_dir.join(group_key);
        tokio::fs::create_dir_all(&group_dir).await?;

        let trimmed_base = base_url.trim_end_matches('/');
        let mut group_schema_names: Vec<String> = Vec::new();

        for (key, schema_def) in &group_config.schemas {
            let entry = process_group_schema(
                ctx,
                &group_dir,
                group_key,
                key,
                schema_def,
                trimmed_base,
                &mut output_paths,
            )
            .await?;
            group_schema_names.push(entry.name.clone());
            entries.push(entry);
        }

        catalog_groups.insert(
            group_key.clone(),
            schema_catalog::CatalogGroup {
                name: group_config.name.clone(),
                description: group_config.description.clone(),
                schemas: group_schema_names,
            },
        );
        groups_meta.insert(
            group_key.clone(),
            (group_config.name.clone(), group_config.description.clone()),
        );
    }

    // 3. Process sources
    let source_ctx = SourceContext {
        cache: ctx.cache,
        base_url,
        schemas_dir: &schemas_dir,
        concurrency: ctx.concurrency,
    };
    for (source_name, source_config) in &ctx.config.sources {
        info!(source = %source_name, url = %source_config.url, "processing source");
        let (source_entries, source_groups) =
            process_source(&source_ctx, source_name, source_config, &mut output_paths)
                .await
                .with_context(|| format!("failed to process source: {source_name}"))?;

        // Merge organize schemas into existing groups (or auto-generate)
        for (group_key, org_schemas) in source_groups {
            if let Some(existing) = catalog_groups.get_mut(&group_key) {
                existing.schemas.extend(org_schemas);
            } else {
                // Auto-generate group from key
                let auto_name = title_case(&group_key);
                let auto_desc = format!("Auto-generated group for organize key '{group_key}'");
                warn!(
                    group = %group_key,
                    "no [groups.{group_key}] defined; auto-generating group \"{auto_name}\""
                );
                catalog_groups.insert(
                    group_key.clone(),
                    schema_catalog::CatalogGroup {
                        name: auto_name.clone(),
                        description: auto_desc.clone(),
                        schemas: org_schemas,
                    },
                );
                groups_meta.insert(group_key, (auto_name, auto_desc));
            }
        }

        entries.extend(source_entries);
    }

    // 4. Build catalog and finalize target output
    info!(entries = entries.len(), "writing output files");
    let catalog_groups_vec: Vec<schema_catalog::CatalogGroup> =
        catalog_groups.into_values().collect();
    let groups_meta_vec: Vec<(String, String)> = groups_meta.into_values().collect();
    let catalog = build_output_catalog(
        ctx.config.catalog.title.clone(),
        entries,
        catalog_groups_vec,
    );

    let output_ctx = OutputContext {
        output_dir,
        config_path: ctx.config_path,
        catalog: &catalog,
        groups_meta: &groups_meta_vec,
        source_count: ctx.config.sources.len(),
    };

    target.finalize(&output_ctx).await?;

    Ok(())
}

/// Process a single group schema entry: download, resolve refs, fetch versions.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
async fn process_group_schema(
    ctx: &GenerateContext<'_>,
    group_dir: &Path,
    group_key: &str,
    key: &str,
    schema_def: &lintel_catalog_builder::config::SchemaDefinition,
    trimmed_base: &str,
    output_paths: &mut HashSet<PathBuf>,
) -> Result<SchemaEntry> {
    let entry_dir = group_dir.join(key);
    tokio::fs::create_dir_all(&entry_dir).await?;

    let dest_path = entry_dir.join("latest.json");

    // Collision detection against the schema directory
    let canonical_dest = entry_dir
        .canonicalize()
        .unwrap_or_else(|_| entry_dir.clone());
    if !output_paths.insert(canonical_dest) {
        bail!(
            "output path collision: {} (group={group_key}, key={key})",
            entry_dir.display(),
        );
    }

    let schema_url = format!("{trimmed_base}/schemas/{group_key}/{key}/latest.json");

    // Per-schema shared dir
    let shared_dir = entry_dir.join("_shared");
    let shared_base_url = format!("{trimmed_base}/schemas/{group_key}/{key}/_shared");
    let mut already_downloaded: HashMap<String, String> = HashMap::new();

    let schema_text = if let Some(url) = &schema_def.url {
        // Download external schema
        let (text, status) = download_one(ctx.cache, url, &dest_path)
            .await
            .with_context(|| format!("failed to download schema for {group_key}/{key}"))?;
        info!(url = %url, status = %status, "downloaded group schema");

        // Resolve $ref dependencies and set $id
        resolve_and_rewrite(
            &mut RefRewriteContext {
                cache: ctx.cache,
                shared_dir: &shared_dir,
                base_url_for_shared: &shared_base_url,
                already_downloaded: &mut already_downloaded,
            },
            &text,
            &dest_path,
            &schema_url,
        )
        .await?;
        text
    } else {
        // Local schema — should exist at schemas/<group>/<key>.json relative to config dir
        let source_path = ctx
            .config_dir
            .join("schemas")
            .join(group_key)
            .join(format!("{key}.json"));
        if !source_path.exists() {
            bail!(
                "local schema not found: {} (expected for group={group_key}, key={key})",
                source_path.display(),
            );
        }

        let text = tokio::fs::read_to_string(&source_path)
            .await
            .with_context(|| format!("failed to read local schema {}", source_path.display()))?;

        // Resolve $ref deps, fix invalid URI references, and set $id
        resolve_and_rewrite(
            &mut RefRewriteContext {
                cache: ctx.cache,
                shared_dir: &shared_dir,
                base_url_for_shared: &shared_base_url,
                already_downloaded: &mut already_downloaded,
            },
            &text,
            &dest_path,
            &schema_url,
        )
        .await?;
        text
    };

    // Download versions
    let mut version_urls: BTreeMap<String, String> = BTreeMap::new();
    if !schema_def.versions.is_empty() {
        tokio::fs::create_dir_all(entry_dir.join("versions")).await?;
    }
    for (version_name, version_url) in &schema_def.versions {
        let version_dest = entry_dir
            .join("versions")
            .join(format!("{version_name}.json"));
        let version_local_url =
            format!("{trimmed_base}/schemas/{group_key}/{key}/versions/{version_name}.json");

        match download_one(ctx.cache, version_url, &version_dest).await {
            Ok((text, status)) => {
                info!(
                    url = %version_url,
                    version = %version_name,
                    status = %status,
                    "downloaded group schema version"
                );
                resolve_and_rewrite(
                    &mut RefRewriteContext {
                        cache: ctx.cache,
                        shared_dir: &shared_dir,
                        base_url_for_shared: &shared_base_url,
                        already_downloaded: &mut already_downloaded,
                    },
                    &text,
                    &version_dest,
                    &version_local_url,
                )
                .await?;
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

    // Auto-populate name and description from the JSON Schema if not
    // explicitly provided in the config.
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

/// Load and parse the catalog config from a TOML file.
async fn load_catalog_config(config_path: &Path) -> Result<CatalogConfig> {
    let config_text = tokio::fs::read_to_string(config_path)
        .await
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    load_config(&config_text).with_context(|| format!("failed to parse {}", config_path.display()))
}

/// Process a single external source (e.g. `SchemaStore`).
///
/// Returns `(entries, groups)` where groups are `(group_key, schema_names)`.
#[allow(clippy::too_many_lines)]
async fn process_source(
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

/// Extract the `title` and `description` from a JSON Schema string.
///
/// Returns `(title, description)` — either or both may be `None` if the schema
/// doesn't contain the corresponding top-level property or isn't valid JSON.
fn extract_schema_meta(text: &str) -> (Option<String>, Option<String>) {
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

/// Convert a key like `"github"` to title case (`"Github"`).
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Simple slugification for fallback filenames.
fn slugify(name: &str) -> String {
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

    #[test]
    fn slugify_simple() {
        assert_eq!(slugify("GitHub Workflow"), "github-workflow");
    }

    #[test]
    fn slugify_special_chars() {
        assert_eq!(slugify("foo/bar (baz)"), "foo-bar-baz");
    }
}
