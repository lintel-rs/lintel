use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use serde_json::Value;

/// Whether a schema was served from disk cache or fetched from the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheStatus {
    /// Schema was found in the disk cache.
    Hit,
    /// Schema was fetched from the network (and possibly written to cache).
    Miss,
    /// Caching is disabled (`cache_dir` is `None`).
    Disabled,
}

/// Trait for fetching content over HTTP.
pub trait HttpClient: Clone + Send + Sync + 'static {
    fn get(&self, uri: &str) -> Result<String, Box<dyn Error + Send + Sync>>;
}

/// Default HTTP client using ureq.
#[derive(Clone)]
pub struct UreqClient;

impl HttpClient for UreqClient {
    fn get(&self, uri: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let mut response = ureq::get(uri).call()?;
        Ok(response.body_mut().read_to_string()?)
    }
}

/// A disk-backed cache for JSON Schema files.
///
/// Schemas are fetched via HTTP and stored as `<cache_dir>/<hash>.json`
/// where `<hash>` is a hex-encoded hash of the URI. When a schema is
/// requested, the cache is checked first; on a miss the schema is fetched
/// and written to disk for future use.
#[derive(Clone)]
pub struct SchemaCache<C: HttpClient = UreqClient> {
    cache_dir: Option<PathBuf>,
    client: C,
}

impl<C: HttpClient> SchemaCache<C> {
    pub fn new(cache_dir: Option<PathBuf>, client: C) -> Self {
        Self { cache_dir, client }
    }

    /// Fetch a schema by URI, using the disk cache when available.
    ///
    /// Returns the parsed schema and a [`CacheStatus`] indicating whether the
    /// result came from the disk cache, the network, or caching was disabled.
    pub fn fetch(&self, uri: &str) -> Result<(Value, CacheStatus), Box<dyn Error + Send + Sync>> {
        // Check cache first
        if let Some(ref cache_dir) = self.cache_dir {
            let hash = Self::hash_uri(uri);
            let cache_path = cache_dir.join(format!("{hash}.json"));
            if cache_path.exists() {
                let content = fs::read_to_string(&cache_path)?;
                return Ok((serde_json::from_str(&content)?, CacheStatus::Hit));
            }
        }

        // Fetch from network
        let body = self.client.get(uri)?;
        let value: Value = serde_json::from_str(&body)?;

        let status = if let Some(ref cache_dir) = self.cache_dir {
            // Write to cache
            fs::create_dir_all(cache_dir)?;
            let hash = Self::hash_uri(uri);
            let cache_path = cache_dir.join(format!("{hash}.json"));
            fs::write(&cache_path, &body)?;
            CacheStatus::Miss
        } else {
            CacheStatus::Disabled
        };

        Ok((value, status))
    }

    fn hash_uri(uri: &str) -> String {
        let mut hasher = DefaultHasher::new();
        uri.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

/// Return the default cache directory for schemas: `<system_cache>/lintel/schemas`.
pub fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("lintel")
        .join("schemas")
}

// -- jsonschema trait impls --------------------------------------------------

impl<C: HttpClient> jsonschema::Retrieve for SchemaCache<C> {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn Error + Send + Sync>> {
        let (value, _status) = self.fetch(uri.as_str())?;
        Ok(value)
    }
}

#[async_trait::async_trait]
impl<C: HttpClient> jsonschema::AsyncRetrieve for SchemaCache<C> {
    async fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn Error + Send + Sync>> {
        let cache = self.clone();
        let uri_str = uri.as_str().to_string();
        let (value, _status) = tokio::task::spawn_blocking(move || cache.fetch(&uri_str)).await??;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Clone)]
    struct MockClient(HashMap<String, String>);

    impl HttpClient for MockClient {
        fn get(&self, uri: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
            self.0
                .get(uri)
                .cloned()
                .ok_or_else(|| format!("mock: no response for {uri}").into())
        }
    }

    fn mock(entries: &[(&str, &str)]) -> MockClient {
        MockClient(
            entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    #[test]
    fn hash_uri_deterministic() {
        let a = SchemaCache::<MockClient>::hash_uri("https://example.com/schema.json");
        let b = SchemaCache::<MockClient>::hash_uri("https://example.com/schema.json");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_uri_different_inputs() {
        let a = SchemaCache::<MockClient>::hash_uri("https://example.com/a.json");
        let b = SchemaCache::<MockClient>::hash_uri("https://example.com/b.json");
        assert_ne!(a, b);
    }

    #[test]
    fn fetch_no_cache_dir() {
        let client = mock(&[("https://example.com/s.json", r#"{"type":"object"}"#)]);
        let cache = SchemaCache::new(None, client);
        let (val, status) = cache.fetch("https://example.com/s.json").unwrap();
        assert_eq!(val, serde_json::json!({"type": "object"}));
        assert_eq!(status, CacheStatus::Disabled);
    }

    #[test]
    fn fetch_cold_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let client = mock(&[("https://example.com/s.json", r#"{"type":"string"}"#)]);
        let cache = SchemaCache::new(Some(tmp.path().to_path_buf()), client);
        let (val, status) = cache.fetch("https://example.com/s.json").unwrap();
        assert_eq!(val, serde_json::json!({"type": "string"}));
        assert_eq!(status, CacheStatus::Miss);

        // Verify file was written to disk
        let hash = SchemaCache::<MockClient>::hash_uri("https://example.com/s.json");
        let cache_path = tmp.path().join(format!("{hash}.json"));
        assert!(cache_path.exists());
    }

    #[test]
    fn fetch_warm_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let hash = SchemaCache::<MockClient>::hash_uri("https://example.com/s.json");
        let cache_path = tmp.path().join(format!("{hash}.json"));
        fs::write(&cache_path, r#"{"type":"number"}"#).unwrap();

        // Client has no entries — if it were called, it would error
        let client = mock(&[]);
        let cache = SchemaCache::new(Some(tmp.path().to_path_buf()), client);
        let (val, status) = cache.fetch("https://example.com/s.json").unwrap();
        assert_eq!(val, serde_json::json!({"type": "number"}));
        assert_eq!(status, CacheStatus::Hit);
    }

    #[test]
    fn fetch_client_error() {
        let client = mock(&[]);
        let cache = SchemaCache::new(None, client);
        assert!(cache.fetch("https://example.com/missing.json").is_err());
    }

    #[test]
    fn fetch_invalid_json() {
        let client = mock(&[("https://example.com/bad.json", "not json")]);
        let cache = SchemaCache::new(None, client);
        assert!(cache.fetch("https://example.com/bad.json").is_err());
    }

    #[test]
    fn retrieve_trait_delegates() {
        let client = mock(&[("https://example.com/s.json", r#"{"type":"object"}"#)]);
        let cache = SchemaCache::new(None, client);
        let uri: jsonschema::Uri<String> = "https://example.com/s.json".parse().unwrap();
        let val = jsonschema::Retrieve::retrieve(&cache, &uri).unwrap();
        assert_eq!(val, serde_json::json!({"type": "object"}));
    }

    #[tokio::test]
    async fn async_retrieve_trait_delegates() {
        let client = mock(&[("https://example.com/s.json", r#"{"type":"object"}"#)]);
        let cache = SchemaCache::new(None, client);
        let uri: jsonschema::Uri<String> = "https://example.com/s.json".parse().unwrap();
        let val = jsonschema::AsyncRetrieve::retrieve(&cache, &uri)
            .await
            .unwrap();
        assert_eq!(val, serde_json::json!({"type": "object"}));
    }

    #[test]
    fn default_cache_dir_ends_with_schemas() {
        let dir = default_cache_dir();
        assert!(dir.ends_with("lintel/schemas"));
    }
}
