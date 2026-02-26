use std::path::Path;

use alloc::string::String;
use alloc::vec::Vec;

use anyhow::{Context, Result};
use schema_catalog::Catalog;

use super::context::{SearchEntry, schema_page_url};

/// Build and write `search-index.json` for client-side search.
pub async fn write_search_index(
    output_dir: &Path,
    catalog: &Catalog,
    base_url: &str,
    groups_meta: &[(String, String, String)],
) -> Result<()> {
    let index = build_search_index(catalog, base_url, groups_meta);
    let json = serde_json::to_string(&index).context("failed to serialize search index")?;
    let path = output_dir.join("search-index.json");
    tokio::fs::write(&path, json)
        .await
        .with_context(|| alloc::format!("failed to write {}", path.display()))?;
    Ok(())
}

fn build_search_index(
    catalog: &Catalog,
    base_url: &str,
    groups_meta: &[(String, String, String)],
) -> Vec<SearchEntry> {
    let group_map = super::context::schema_group_map(catalog, groups_meta);

    catalog
        .schemas
        .iter()
        .filter_map(|entry| {
            let url = schema_page_url(&entry.url, base_url)?;
            let group_name = group_map
                .get(entry.name.as_str())
                .map_or(String::new(), |(_, name)| String::from(*name));
            let file_match_str = entry.file_match.join(", ");
            Some(SearchEntry {
                n: entry.name.clone(),
                d: entry.description.clone(),
                f: file_match_str,
                u: url,
                g: group_name,
            })
        })
        .collect()
}
