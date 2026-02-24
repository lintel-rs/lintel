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
use crate::targets::{AnyTarget, OutputContext, Target};
use lintel_catalog_builder::config::{CatalogConfig, load_config};

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

        let processed = ProcessedSchemas::new(&output_dir);
        let ctx = GenerateContext {
            cache: &cache,
            config: &config,
            config_path: &config_path,
            config_dir,
            processed: &processed,
        };

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

        let group_ctx = GroupSchemaContext {
            generate: ctx,
            group_dir: &group_dir,
            group_key,
            trimmed_base,
            processed: ctx.processed,
        };
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

    let output_ctx = OutputContext {
        output_dir,
        config_path: ctx.config_path,
        catalog: &catalog,
        groups_meta: &groups_meta_vec,
        base_url,
        source_count: ctx.config.sources.len(),
        processed: ctx.processed,
    };

    target.finalize(&output_ctx).await?;

    Ok(())
}

/// Load and parse the catalog config from a TOML file.
async fn load_catalog_config(config_path: &Path) -> Result<CatalogConfig> {
    let config_text = tokio::fs::read_to_string(config_path)
        .await
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    load_config(&config_text).with_context(|| format!("failed to parse {}", config_path.display()))
}
