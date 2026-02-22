use crate::retriever::SchemaCache;
use schemastore::Catalog;

/// The default Lintel catalog registry (always fetched unless `--no-catalog`).
pub const DEFAULT_REGISTRY: &str = "https://catalog.lintel.tools/catalog.json";

/// Resolve a registry URL, expanding shorthand notations.
///
/// Supported shorthands:
/// - `github:org/repo`        → tries `main` then `master` branch
/// - `github:org/repo/branch` → uses the specified branch
///
/// Plain `http://` and `https://` URLs are returned as-is.
///
/// Returns one or more URLs to try in order.
pub fn resolve_urls(url: &str) -> Vec<String> {
    if let Some(rest) = url.strip_prefix("github:") {
        // github:org/repo/branch — explicit branch
        let parts: Vec<&str> = rest.splitn(3, '/').collect();
        if parts.len() == 3 {
            vec![format!(
                "https://raw.githubusercontent.com/{}/{}/{}/catalog.json",
                parts[0], parts[1], parts[2]
            )]
        } else {
            // github:org/repo — try main first, then master
            vec![
                format!("https://raw.githubusercontent.com/{rest}/main/catalog.json"),
                format!("https://raw.githubusercontent.com/{rest}/master/catalog.json"),
            ]
        }
    } else {
        vec![url.to_string()]
    }
}

/// Fetch a schema registry catalog by URL.
///
/// The URL is first resolved via [`resolve_urls`] to expand shorthand
/// notations like `github:org/repo`. For GitHub shorthands without an
/// explicit branch, both `main` and `master` are tried.
///
/// # Errors
///
/// Returns an error if none of the resolved URLs can be fetched or parsed.
pub async fn fetch(
    cache: &SchemaCache,
    url: &str,
) -> Result<Catalog, Box<dyn core::error::Error + Send + Sync>> {
    let urls = resolve_urls(url);
    let mut last_err: Option<Box<dyn core::error::Error + Send + Sync>> = None;
    for resolved in &urls {
        match cache.fetch(resolved).await {
            Ok((value, _status)) => {
                let catalog = schemastore::parse_catalog(value)?;
                return Ok(catalog);
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| "no URLs to try".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_shorthand_tries_main_then_master() {
        let urls = resolve_urls("github:my-org/my-schemas");
        assert_eq!(urls.len(), 2);
        assert_eq!(
            urls[0],
            "https://raw.githubusercontent.com/my-org/my-schemas/main/catalog.json"
        );
        assert_eq!(
            urls[1],
            "https://raw.githubusercontent.com/my-org/my-schemas/master/catalog.json"
        );
    }

    #[test]
    fn github_shorthand_with_explicit_branch() {
        let urls = resolve_urls("github:lintel-rs/lintel/master");
        assert_eq!(urls.len(), 1);
        assert_eq!(
            urls[0],
            "https://raw.githubusercontent.com/lintel-rs/lintel/master/catalog.json"
        );
    }

    #[test]
    fn plain_url_unchanged() {
        let url = "https://example.com/catalog.json";
        assert_eq!(resolve_urls(url), vec![url]);
    }

    #[test]
    fn default_registry_is_plain_url() {
        let urls = resolve_urls(DEFAULT_REGISTRY);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://catalog.lintel.tools/catalog.json");
    }
}
