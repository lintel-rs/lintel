pub use schemastore::{CATALOG_URL, Catalog, CompiledCatalog};

use crate::retriever::{HttpClient, SchemaCache};

/// Fetch the `SchemaStore` catalog via the schema cache.
///
/// # Errors
///
/// Returns an error if the catalog cannot be fetched or parsed.
pub fn fetch_catalog<C: HttpClient>(
    cache: &SchemaCache<C>,
) -> Result<Catalog, Box<dyn std::error::Error + Send + Sync>> {
    let (value, _status) = cache.fetch(CATALOG_URL)?;
    let catalog = schemastore::parse_catalog(value)?;
    Ok(catalog)
}
