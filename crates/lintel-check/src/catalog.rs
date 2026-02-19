pub use schemastore::{Catalog, CompiledCatalog, CATALOG_URL};

use crate::retriever::{HttpClient, SchemaCache};

/// The default Lintel catalog registry (always fetched unless `--no-catalog`).
pub const DEFAULT_REGISTRY: &str = "github:lintel-rs/catalog";

/// Resolve a registry URL, expanding shorthand notations.
///
/// Supported shorthands:
/// - `github:org/repo` → `https://raw.githubusercontent.com/org/repo/main/catalog.json`
pub fn resolve_registry_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("github:") {
        format!("https://raw.githubusercontent.com/{rest}/main/catalog.json")
    } else {
        url.to_string()
    }
}

/// Fetch the SchemaStore catalog via the schema cache.
pub fn fetch_catalog<C: HttpClient>(
    cache: &SchemaCache<C>,
) -> Result<Catalog, Box<dyn std::error::Error + Send + Sync>> {
    let (value, _status) = cache.fetch(CATALOG_URL)?;
    let catalog = schemastore::parse_catalog(value)?;
    Ok(catalog)
}

/// Fetch an additional schema registry catalog by URL.
///
/// The URL is first resolved via [`resolve_registry_url`] to expand
/// shorthand notations like `github:org/repo`.
pub fn fetch_registry<C: HttpClient>(
    cache: &SchemaCache<C>,
    url: &str,
) -> Result<Catalog, Box<dyn std::error::Error + Send + Sync>> {
    let resolved = resolve_registry_url(url);
    let (value, _status) = cache.fetch(&resolved)?;
    let catalog = schemastore::parse_catalog(value)?;
    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_github_shorthand() {
        assert_eq!(
            resolve_registry_url("github:lintel-rs/catalog"),
            "https://raw.githubusercontent.com/lintel-rs/catalog/main/catalog.json"
        );
    }

    #[test]
    fn resolve_github_shorthand_with_org() {
        assert_eq!(
            resolve_registry_url("github:my-org/my-schemas"),
            "https://raw.githubusercontent.com/my-org/my-schemas/main/catalog.json"
        );
    }

    #[test]
    fn resolve_plain_url_unchanged() {
        let url = "https://example.com/catalog.json";
        assert_eq!(resolve_registry_url(url), url);
    }
}
