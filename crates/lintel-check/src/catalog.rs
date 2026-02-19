pub use schemastore::{Catalog, CompiledCatalog, CATALOG_URL};

use crate::retriever::{HttpClient, SchemaCache};

/// Fetch the SchemaStore catalog via the schema cache.
pub fn fetch_catalog<C: HttpClient>(
    cache: &SchemaCache<C>,
) -> Result<Catalog, Box<dyn std::error::Error + Send + Sync>> {
    let (value, _status) = cache.fetch(CATALOG_URL)?;
    let catalog = schemastore::parse_catalog(value)?;
    Ok(catalog)
}
