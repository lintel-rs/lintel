use alloc::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use schema_catalog::SchemaEntry;
use tracing::{debug, info, warn};

use crate::catalog::{build_output_catalog, write_catalog_json};
use crate::config::{CatalogConfig, SourceConfig, load_config};
use crate::download::{DownloadItem, download_batch, download_one};
use crate::refs::{filename_from_url, find_external_refs, resolve_and_rewrite};

/// Run the `generate` subcommand.
#[allow(clippy::too_many_lines)]
pub async fn run(config_path: &Path, output_dir: Option<&Path>, concurrency: usize) -> Result<()> {
    // Resolve config path
    let config_path = config_path
        .canonicalize()
        .with_context(|| format!("config file not found: {}", config_path.display()))?;
    let config_dir = config_path
        .parent()
        .context("config file has no parent directory")?;

    // Default output dir is the config file's parent directory
    let output_dir = output_dir.unwrap_or(config_dir);

    info!(config = %config_path.display(), output = %output_dir.display(), "starting catalog generation");

    // 1. Load config
    let config = load_catalog_config(&config_path).await?;

    let client = reqwest::Client::new();
    let schemas_dir = output_dir.join("schemas");
    let mut entries: Vec<SchemaEntry> = Vec::new();
    let mut output_paths: HashSet<PathBuf> = HashSet::new();

    // 2. Process groups
    for (group_name, group_schemas) in &config.groups {
        info!(group = %group_name, count = group_schemas.len(), "processing group");
        let group_dir = schemas_dir.join(group_name);
        tokio::fs::create_dir_all(&group_dir).await?;

        let shared_dir = group_dir.join("_shared");
        let base_url = config.catalog.base_url.trim_end_matches('/');
        let shared_base_url = format!("{base_url}/schemas/{group_name}/_shared");
        let mut already_downloaded: HashMap<String, String> = HashMap::new();

        for (key, schema_def) in group_schemas {
            let filename = format!("{key}.json");
            let dest_path = group_dir.join(&filename);

            // Collision detection
            let canonical_dest = dest_path
                .canonicalize()
                .unwrap_or_else(|_| dest_path.clone());
            if !output_paths.insert(canonical_dest) {
                bail!(
                    "output path collision: {} (group={group_name}, key={key})",
                    dest_path.display(),
                );
            }

            let schema_url = format!("{base_url}/schemas/{group_name}/{filename}");

            if let Some(url) = &schema_def.url {
                // Download external schema
                info!(url = %url, dest = %dest_path.display(), "downloading group schema");
                let text = download_one(&client, url, &dest_path)
                    .await
                    .with_context(|| format!("failed to download schema for {group_name}/{key}"))?;

                // Resolve $ref dependencies
                resolve_and_rewrite(
                    &client,
                    &text,
                    &dest_path,
                    &shared_dir,
                    &shared_base_url,
                    &mut already_downloaded,
                )
                .await?;
            } else {
                // Local schema — should exist at schemas/<group>/<key>.json relative to config dir
                let source_path = config_dir.join("schemas").join(group_name).join(&filename);
                if !source_path.exists() {
                    bail!(
                        "local schema not found: {} (expected for group={group_name}, key={key})",
                        source_path.display(),
                    );
                }

                let text = tokio::fs::read_to_string(&source_path)
                    .await
                    .with_context(|| {
                        format!("failed to read local schema {}", source_path.display())
                    })?;

                if output_dir == config_dir {
                    // Same directory — only resolve $ref deps if present
                    let value: serde_json::Value = serde_json::from_str(&text)?;
                    let ext_refs = find_external_refs(&value);
                    if !ext_refs.is_empty() {
                        resolve_and_rewrite(
                            &client,
                            &text,
                            &dest_path,
                            &shared_dir,
                            &shared_base_url,
                            &mut already_downloaded,
                        )
                        .await?;
                    }
                } else {
                    // Copy to output dir and resolve $ref deps
                    resolve_and_rewrite(
                        &client,
                        &text,
                        &dest_path,
                        &shared_dir,
                        &shared_base_url,
                        &mut already_downloaded,
                    )
                    .await?;
                }
            }

            entries.push(SchemaEntry {
                name: schema_def.name.clone(),
                description: schema_def.description.clone(),
                url: schema_url,
                file_match: schema_def.file_match.clone(),
                versions: BTreeMap::new(),
            });
        }
    }

    // 3. Process sources
    for (source_name, source_config) in &config.sources {
        info!(source = %source_name, url = %source_config.url, "processing source");
        let source_entries = process_source(
            &client,
            &config.catalog.base_url,
            source_name,
            source_config,
            &schemas_dir,
            concurrency,
            &mut output_paths,
        )
        .await
        .with_context(|| format!("failed to process source: {source_name}"))?;
        entries.extend(source_entries);
    }

    // 4. Write catalog.json
    info!(entries = entries.len(), "writing catalog.json");
    let catalog = build_output_catalog(entries);
    write_catalog_json(output_dir, &catalog).await?;

    info!("catalog generation complete");
    Ok(())
}

/// Load and parse the catalog config from a TOML file.
async fn load_catalog_config(config_path: &Path) -> Result<CatalogConfig> {
    let config_text = tokio::fs::read_to_string(config_path)
        .await
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    load_config(&config_text).with_context(|| format!("failed to parse {}", config_path.display()))
}

/// Process a single external source (e.g. `SchemaStore`).
#[allow(clippy::too_many_lines)]
async fn process_source(
    client: &reqwest::Client,
    base_url: &str,
    source_name: &str,
    source_config: &SourceConfig,
    schemas_dir: &Path,
    concurrency: usize,
    output_paths: &mut HashSet<PathBuf>,
) -> Result<Vec<SchemaEntry>> {
    // Fetch and parse the external catalog
    info!(url = %source_config.url, "fetching source catalog");
    let catalog_text = client
        .get(&source_config.url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let source_catalog: schema_catalog::Catalog = serde_json::from_str(&catalog_text)
        .with_context(|| format!("failed to parse source catalog from {}", source_config.url))?;

    info!(
        schemas = source_catalog.schemas.len(),
        "source catalog parsed"
    );

    let base_url = base_url.trim_end_matches('/');
    let source_dir = schemas_dir.join(source_name);
    tokio::fs::create_dir_all(&source_dir).await?;

    // Create organize directories
    for dir_name in source_config.organize.keys() {
        tokio::fs::create_dir_all(schemas_dir.join(dir_name)).await?;
    }

    // Classify each schema into an organize directory or the source default.
    // Track filenames per directory to handle collisions (e.g. many schemas
    // use "schema.json" as their URL's last segment). On collision, fall back
    // to a slugified schema name with numeric suffix deduplication.
    let mut download_items: Vec<DownloadItem> = Vec::new();
    let mut entry_info: Vec<SourceSchemaInfo> = Vec::new();
    let mut dir_filename_counts: HashMap<(String, String), usize> = HashMap::new();
    let mut seen_urls: HashSet<String> = HashSet::new();

    for schema in &source_catalog.schemas {
        // Skip duplicate URLs within the same source catalog
        if !seen_urls.insert(schema.url.clone()) {
            continue;
        }

        let target_dir = classify_schema(schema, &source_config.organize, source_name)?;
        let base_filename = filename_from_url(&schema.url)
            .unwrap_or_else(|_| format!("{}.json", slugify(&schema.name)));

        // Deduplicate filenames within the same directory
        let key = (target_dir.clone(), base_filename.clone());
        let count = dir_filename_counts.entry(key).or_insert(0);
        *count += 1;
        let filename = if *count == 1 {
            base_filename
        } else {
            // On collision, use slugified schema name instead
            let slug_name = format!("{}.json", slugify(&schema.name));
            let slug_key = (target_dir.clone(), slug_name.clone());
            let slug_count = dir_filename_counts.entry(slug_key).or_insert(0);
            *slug_count += 1;
            if *slug_count == 1 {
                slug_name
            } else {
                format!("{}-{}.json", slugify(&schema.name), slug_count)
            }
        };

        let dest_path = schemas_dir.join(&target_dir).join(&filename);

        // Final collision detection against all output paths (including groups)
        let canonical_dest = dest_path
            .canonicalize()
            .unwrap_or_else(|_| dest_path.clone());
        if !output_paths.insert(canonical_dest) {
            bail!(
                "output path collision: {} (source={source_name}, schema={})",
                dest_path.display(),
                schema.name,
            );
        }

        let local_url = format!("{base_url}/schemas/{target_dir}/{filename}");
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
        });
    }

    // Download all schemas concurrently
    info!(
        count = download_items.len(),
        concurrency, "downloading source schemas"
    );
    let downloaded = download_batch(client, &download_items, concurrency).await?;

    info!(
        downloaded = downloaded.len(),
        skipped = download_items.len() - downloaded.len(),
        "source download complete"
    );

    // Resolve $ref deps for downloaded schemas
    resolve_source_refs(
        client,
        &download_items,
        &entry_info,
        &downloaded,
        schemas_dir,
        base_url,
        source_name,
    )
    .await?;

    // Build catalog entries
    let entries = entry_info
        .iter()
        .map(|info| {
            let url = if downloaded.contains(&info.url) {
                info.local_url.clone()
            } else {
                warn!(schema = %info.name, "using upstream URL (download was skipped)");
                info.url.clone()
            };
            SchemaEntry {
                name: info.name.clone(),
                description: info.description.clone(),
                url,
                file_match: info.file_match.clone(),
                versions: info.versions.clone(),
            }
        })
        .collect();

    Ok(entries)
}

/// Resolve `$ref` dependencies for all downloaded source schemas.
async fn resolve_source_refs(
    client: &reqwest::Client,
    download_items: &[DownloadItem],
    entry_info: &[SourceSchemaInfo],
    downloaded: &HashSet<String>,
    schemas_dir: &Path,
    base_url: &str,
    source_name: &str,
) -> Result<()> {
    let shared_dir = schemas_dir.join(source_name).join("_shared");
    let shared_base_url = format!("{base_url}/schemas/{source_name}/_shared");
    let mut already_downloaded: HashMap<String, String> = HashMap::new();

    for (item, info) in download_items.iter().zip(entry_info.iter()) {
        if !downloaded.contains(&item.url) {
            continue;
        }
        let text = tokio::fs::read_to_string(&item.dest).await?;
        let value: serde_json::Value = serde_json::from_str(&text)?;
        let ext_refs = find_external_refs(&value);
        if !ext_refs.is_empty() {
            debug!(schema = %info.name, refs = ext_refs.len(), "resolving $ref deps for source schema");
            resolve_and_rewrite(
                client,
                &text,
                &item.dest,
                &shared_dir,
                &shared_base_url,
                &mut already_downloaded,
            )
            .await
            .with_context(|| format!("failed to resolve $ref deps for {}", info.name))?;
        }
    }

    Ok(())
}

/// Information about a source schema being processed.
struct SourceSchemaInfo {
    name: String,
    description: String,
    url: String,
    local_url: String,
    file_match: Vec<String>,
    versions: BTreeMap<String, String>,
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
    organize: &BTreeMap<String, Vec<String>>,
    source_name: &str,
) -> Result<String> {
    let mut matched_dir: Option<&str> = None;

    for (dir_name, matchers) in organize {
        for matcher in matchers {
            let matches = if matcher.starts_with("http://") || matcher.starts_with("https://") {
                // URL exact match
                schema.url == *matcher
            } else {
                // Glob match against fileMatch patterns (as literal strings).
                // We use a custom matcher because `glob_match` treats `**` in
                // the middle of a segment as two `*` characters (which don't
                // cross path separators), but we need `**` to match any
                // characters including `/` when matching against fileMatch
                // pattern strings.
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
            file_match: file_match.into_iter().map(String::from).collect(),
            versions: BTreeMap::new(),
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
        organize.insert("github".to_string(), vec!["**.github**".to_string()]);
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
            vec!["https://example.com/special.json".to_string()],
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
        organize.insert("dir1".to_string(), vec!["**.github**".to_string()]);
        organize.insert("dir2".to_string(), vec!["**.github**".to_string()]);
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
        organize.insert("github".to_string(), vec!["**.github**".to_string()]);
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
