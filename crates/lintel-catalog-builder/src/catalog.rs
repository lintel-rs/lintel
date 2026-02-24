use std::path::Path;

use anyhow::{Context, Result};
use schema_catalog::{Catalog, SchemaEntry};

/// Build an output catalog from a list of schema entries and optional groups.
pub fn build_output_catalog(
    title: Option<String>,
    entries: Vec<SchemaEntry>,
    groups: Vec<schema_catalog::CatalogGroup>,
) -> Catalog {
    Catalog {
        version: 1,
        title,
        schemas: entries,
        groups,
        ..Catalog::default()
    }
}

/// Write the catalog to `catalog.json` in the given directory.
///
/// Pretty-prints the JSON. The custom `Serialize` impl on [`Catalog`]
/// handles `$schema` injection and key ordering automatically.
pub async fn write_catalog_json(output_dir: &Path, catalog: &Catalog) -> Result<()> {
    let json = serde_json::to_string_pretty(catalog).context("failed to serialize catalog")?;
    let catalog_path = output_dir.join("catalog.json");
    tokio::fs::write(&catalog_path, format!("{json}\n"))
        .await
        .with_context(|| format!("failed to write {}", catalog_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use super::*;

    #[test]
    fn build_output_catalog_sets_version() {
        let catalog = build_output_catalog(None, vec![], vec![]);
        assert_eq!(catalog.version, 1);
        assert!(catalog.title.is_none());
        assert!(catalog.schemas.is_empty());
        assert!(catalog.groups.is_empty());
    }

    #[tokio::test]
    async fn write_catalog_json_includes_schema_field() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let catalog = build_output_catalog(
            None,
            vec![SchemaEntry {
                name: "Test".into(),
                description: "A test".into(),
                url: "https://example.com/test.json".into(),
                source_url: None,
                file_match: vec!["*.test".into()],
                versions: BTreeMap::new(),
            }],
            vec![],
        );
        write_catalog_json(dir.path(), &catalog).await?;

        let content = tokio::fs::read_to_string(dir.path().join("catalog.json")).await?;
        let value: serde_json::Value = serde_json::from_str(&content)?;
        assert_eq!(value["$schema"], schema_catalog::DEFAULT_SCHEMA_URL,);
        assert_eq!(value["version"], 1);
        assert_eq!(value["schemas"][0]["name"], "Test");
        assert_eq!(value["schemas"][0]["fileMatch"][0], "*.test");
        Ok(())
    }
}
