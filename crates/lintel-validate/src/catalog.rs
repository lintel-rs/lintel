use lintel_schema_cache::SchemaCache;
use schema_catalog::Catalog;

/// The URL of the `SchemaStore` catalog.
pub const SCHEMASTORE_CATALOG_URL: &str = "https://www.schemastore.org/api/json/catalog.json";

/// Fetch the `SchemaStore` catalog via the schema cache.
///
/// # Errors
///
/// Returns an error if the catalog cannot be fetched or parsed.
pub async fn fetch_catalog(
    cache: &SchemaCache,
) -> Result<Catalog, Box<dyn core::error::Error + Send + Sync>> {
    let (value, _status) = cache.fetch(SCHEMASTORE_CATALOG_URL).await?;
    let catalog = schema_catalog::parse_catalog_value(value)?;
    Ok(catalog)
}
