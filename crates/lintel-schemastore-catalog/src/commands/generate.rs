use std::path::Path;

use anyhow::{Context, Result};
use tracing::{debug, info};

use crate::catalog::{build_filename_map, rewrite_catalog_urls};
use crate::download::download_schemas;

const DEFAULT_CONCURRENCY: usize = 20;
const DEFAULT_BASE_URL: &str =
    "https://raw.githubusercontent.com/lintel-rs/schemastore-catalog/main/schemas";

/// Run the `generate` subcommand: fetch catalog, download schemas, rewrite URLs, write output.
pub async fn run(
    output_dir: &Path,
    concurrency: Option<usize>,
    base_url: Option<&str>,
) -> Result<()> {
    let concurrency = concurrency.unwrap_or(DEFAULT_CONCURRENCY);
    let base_url = base_url.unwrap_or(DEFAULT_BASE_URL);
    let client = reqwest::Client::new();

    // 1. Fetch the SchemaStore catalog
    info!(url = schemastore::CATALOG_URL, "fetching catalog");
    let catalog_text = client
        .get(schemastore::CATALOG_URL)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    debug!(bytes = catalog_text.len(), "catalog fetched");

    // 2. Parse as both typed Catalog (for URL extraction) and Value (for round-trip rewriting)
    let catalog: schema_catalog::Catalog =
        serde_json::from_str(&catalog_text).context("failed to parse SchemaStore catalog")?;
    let mut catalog_value: serde_json::Value = serde_json::from_str(&catalog_text)
        .context("failed to parse SchemaStore catalog as JSON value")?;
    debug!(schemas = catalog.schemas.len(), "catalog parsed");

    // 3. Build URL → filename mapping from schema names
    let url_to_filename = build_filename_map(&catalog.schemas);
    let urls_and_filenames: Vec<(String, String)> = url_to_filename
        .iter()
        .map(|(url, filename)| (url.clone(), filename.clone()))
        .collect();

    info!(
        count = urls_and_filenames.len(),
        concurrency, "downloading schemas"
    );

    // 4. Create output directories
    let schemas_dir = output_dir.join("schemas");
    tokio::fs::create_dir_all(&schemas_dir)
        .await
        .context("failed to create schemas directory")?;
    debug!(path = %schemas_dir.display(), "created schemas directory");

    // 5. Download all schemas concurrently
    let downloaded =
        download_schemas(&client, &urls_and_filenames, &schemas_dir, concurrency).await?;

    info!(
        downloaded = downloaded.len(),
        failed = urls_and_filenames.len() - downloaded.len(),
        "download complete"
    );

    // 6. Rewrite catalog URLs to point to local schemas
    info!(base_url, "rewriting catalog URLs");
    rewrite_catalog_urls(&mut catalog_value, base_url, &url_to_filename, &downloaded);

    // 7. Write the rewritten catalog
    let catalog_path = output_dir.join("catalog.json");
    let catalog_json =
        serde_json::to_string_pretty(&catalog_value).context("failed to serialize catalog")?;
    tokio::fs::write(&catalog_path, catalog_json)
        .await
        .context("failed to write catalog.json")?;

    info!(path = %catalog_path.display(), "catalog written");
    Ok(())
}
