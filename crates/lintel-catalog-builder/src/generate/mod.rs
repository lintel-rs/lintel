mod groups;
mod sources;
mod util;

use alloc::collections::BTreeMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use lintel_schema_cache::SchemaCache;
use schema_catalog::SchemaEntry;
use tracing::{info, warn};

use crate::catalog::build_output_catalog;
use crate::download::ProcessedSchemas;
use crate::targets::{self, OutputContext};
use lintel_catalog_builder::config::SchemaDefinition;
use lintel_catalog_builder::config::{CatalogConfig, TargetConfig, load_config};

use groups::{GroupSchemaContext, process_group_schema};
use sources::{SourceContext, process_source};
use util::title_case;

/// Cross-cutting state shared across the entire generation run.
struct GenerateContext<'a> {
    cache: &'a SchemaCache,
    config: &'a CatalogConfig,
    config_path: &'a Path,
    config_dir: &'a Path,
    processed: &'a ProcessedSchemas,
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

    // Create schema cache with unified concurrency control
    let cache = SchemaCache::builder()
        .force_fetch(no_cache)
        .max_concurrent_requests(concurrency)
        .build();

    // Process each target
    for (target_name, target_config) in &config.target {
        if let Some(filter) = target_filter
            && target_name != filter
        {
            continue;
        }

        info!(target = %target_name, "building target");
        let output_dir = targets::output_dir(target_config, config_dir);

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

        let processed = ProcessedSchemas::new(&output_dir);
        let ctx = GenerateContext {
            cache: &cache,
            config: &config,
            config_path: &config_path,
            config_dir,
            processed: &processed,
        };

        generate_for_target(&ctx, target_config, &output_dir)
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
    target: &TargetConfig,
    output_dir: &Path,
) -> Result<()> {
    let base_url = &target.base_url;
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

        // Auto-discover local schema files in the group's source directory.
        // Files that aren't explicit entries are processed as implicit entries
        // so that sibling `$ref`s like `./hooks.json` resolve to canonical
        // catalog URLs instead of being downloaded into `_shared/`.
        let source_dir = ctx.config_dir.join("schemas").join(group_key);
        let mut implicit_schemas: Vec<(String, SchemaDefinition)> = Vec::new();
        if source_dir.is_dir()
            && let Ok(mut dir) = tokio::fs::read_dir(&source_dir).await
        {
            while let Ok(Some(entry)) = dir.next_entry().await {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "json")
                    && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                    && !group_config.schemas.contains_key(stem)
                {
                    implicit_schemas.push((
                        stem.to_string(),
                        SchemaDefinition {
                            url: None,
                            name: None,
                            description: None,
                            file_match: Vec::new(),
                            versions: BTreeMap::new(),
                        },
                    ));
                }
            }
        }

        // Build sibling URL map from both explicit and implicit entries.
        let mut sibling_urls = std::collections::HashMap::new();
        for sibling_key in group_config
            .schemas
            .keys()
            .chain(implicit_schemas.iter().map(|(k, _)| k))
        {
            let canonical = format!("{trimmed_base}/schemas/{group_key}/{sibling_key}/latest.json");
            sibling_urls.insert(format!("./{sibling_key}.json"), canonical.clone());
            sibling_urls.insert(format!("{sibling_key}.json"), canonical);
        }

        let group_ctx = GroupSchemaContext {
            generate: ctx,
            group_dir: &group_dir,
            group_key,
            trimmed_base,
            processed: ctx.processed,
            source_base_url: ctx.config.catalog.source_base_url.as_deref(),
            sibling_urls,
        };

        // Process implicit entries first so their files exist when
        // explicit entries reference them.
        for (key, schema_def) in &implicit_schemas {
            let entry =
                process_group_schema(&group_ctx, key, schema_def, &mut output_paths).await?;
            group_schema_names.push(entry.name.clone());
            entries.push(entry);
        }

        for (key, schema_def) in &group_config.schemas {
            let entry =
                process_group_schema(&group_ctx, key, schema_def, &mut output_paths).await?;
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
        processed: ctx.processed,
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
    let groups_meta_vec: Vec<(String, String, String)> = groups_meta
        .into_iter()
        .map(|(k, (n, d))| (k, n, d))
        .collect();
    let catalog = build_output_catalog(
        ctx.config.catalog.title.clone(),
        entries,
        catalog_groups_vec,
    );

    let site_description = target.site.as_ref().and_then(|s| s.description.as_deref());
    let ga_tracking_id = target
        .site
        .as_ref()
        .and_then(|s| s.ga_tracking_id.as_deref());
    let output_ctx = OutputContext {
        output_dir,
        config_path: ctx.config_path,
        config_dir: ctx.config_dir,
        catalog: &catalog,
        groups_meta: &groups_meta_vec,
        base_url,
        source_count: ctx.config.sources.len(),
        processed: ctx.processed,
        site_description,
        ga_tracking_id,
    };

    targets::finalize(target, &output_ctx).await?;

    Ok(())
}

/// Load and parse the catalog config from a TOML file.
async fn load_catalog_config(config_path: &Path) -> Result<CatalogConfig> {
    let config_text = tokio::fs::read_to_string(config_path)
        .await
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    load_config(&config_text).with_context(|| format!("failed to parse {}", config_path.display()))
}
