pub use schemastore::{CATALOG_URL, Catalog, CompiledCatalog};

use crate::retriever::HttpCache;

/// Fetch the `SchemaStore` catalog via the schema cache.
///
/// # Errors
///
/// Returns an error if the catalog cannot be fetched or parsed.
pub async fn fetch_catalog(
    cache: &HttpCache,
) -> Result<Catalog, Box<dyn core::error::Error + Send + Sync>> {
    let (value, _status) = cache.fetch(CATALOG_URL).await?;
    let catalog = schemastore::parse_catalog(value)?;
    Ok(catalog)
}
