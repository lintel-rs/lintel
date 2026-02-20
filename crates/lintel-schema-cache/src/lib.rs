use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Default TTL for cached schemas (12 hours).
pub const DEFAULT_SCHEMA_CACHE_TTL: Duration = Duration::from_secs(12 * 60 * 60);

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
#[async_trait::async_trait]
pub trait HttpClient: Clone + Send + Sync + 'static {
    /// # Errors
    ///
    /// Returns an error if the HTTP request fails or the response cannot be read.
    async fn get(&self, uri: &str) -> Result<String, Box<dyn Error + Send + Sync>>;
}

/// Default HTTP client using reqwest.
#[derive(Clone)]
pub struct ReqwestClient(pub reqwest::Client);

impl Default for ReqwestClient {
    fn default() -> Self {
        Self(reqwest::Client::new())
    }
}

#[async_trait::async_trait]
impl HttpClient for ReqwestClient {
    async fn get(&self, uri: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let resp = self.0.get(uri).send().await?.error_for_status()?;
        Ok(resp.text().await?)
    }
}

/// A disk-backed cache for JSON Schema files.
///
/// Schemas are fetched via HTTP and stored as `<cache_dir>/<hash>.json`
/// where `<hash>` is a hex-encoded hash of the URI. When a schema is
/// requested, the cache is checked first; on a miss the schema is fetched
/// and written to disk for future use.
#[derive(Clone)]
pub struct SchemaCache<C: HttpClient = ReqwestClient> {
    cache_dir: Option<PathBuf>,
    client: C,
    skip_read: bool,
    ttl: Option<Duration>,
    /// In-memory cache shared across all clones via `Arc`.
    memory_cache: Arc<Mutex<HashMap<String, Value>>>,
}

impl<C: HttpClient> SchemaCache<C> {
    pub fn new(
        cache_dir: Option<PathBuf>,
        client: C,
        skip_read: bool,
        ttl: Option<Duration>,
    ) -> Self {
        Self {
            cache_dir,
            client,
            skip_read,
            ttl,
            memory_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Fetch a schema by URI, using the disk cache when available.
    ///
    /// Returns the parsed schema and a [`CacheStatus`] indicating whether the
    /// result came from the disk cache, the network, or caching was disabled.
    ///
    /// When `skip_read` is set, the cache read is skipped but fetched schemas
    /// are still written to disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the schema cannot be fetched from the network,
    /// read from disk cache, or parsed as JSON.
    #[allow(clippy::missing_panics_doc)] // Mutex poisoning is unreachable
    #[tracing::instrument(skip(self), fields(status))]
    pub async fn fetch(
        &self,
        uri: &str,
    ) -> Result<(Value, CacheStatus), Box<dyn Error + Send + Sync>> {
        // Check in-memory cache first (unless skip_read is set)
        if !self.skip_read
            && let Some(value) = self
                .memory_cache
                .lock()
                .expect("memory cache poisoned")
                .get(uri)
                .cloned()
        {
            tracing::Span::current().record("status", "memory_hit");
            return Ok((value, CacheStatus::Hit));
        }

        // Check disk cache (unless skip_read is set)
        if !self.skip_read
            && let Some(ref cache_dir) = self.cache_dir
        {
            let hash = Self::hash_uri(uri);
            let cache_path = cache_dir.join(format!("{hash}.json"));
            if cache_path.exists() && !self.is_expired(&cache_path) {
                let content = fs::read_to_string(&cache_path)?;
                let value: Value = serde_json::from_str(&content)?;
                self.memory_cache
                    .lock()
                    .expect("memory cache poisoned")
                    .insert(uri.to_string(), value.clone());
                tracing::Span::current().record("status", "cache_hit");
                return Ok((value, CacheStatus::Hit));
            }
        }

        // Fetch from network
        tracing::Span::current().record("status", "network_fetch");
        let body = self.client.get(uri).await?;
        let value: Value = serde_json::from_str(&body)?;

        // Populate in-memory cache
        self.memory_cache
            .lock()
            .expect("memory cache poisoned")
            .insert(uri.to_string(), value.clone());

        let status = if let Some(ref cache_dir) = self.cache_dir {
            // Write to disk cache
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

    /// Check whether a cached file has exceeded the configured TTL.
    ///
    /// Returns `false` (not expired) when:
    /// - No TTL is configured (`self.ttl` is `None`)
    /// - The file metadata or mtime cannot be read (graceful degradation)
    fn is_expired(&self, path: &std::path::Path) -> bool {
        let Some(ttl) = self.ttl else {
            return false;
        };
        fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|mtime| mtime.elapsed().ok())
            .is_some_and(|age| age > ttl)
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

#[async_trait::async_trait]
impl<C: HttpClient> jsonschema::AsyncRetrieve for SchemaCache<C> {
    async fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn Error + Send + Sync>> {
        let (value, _status) = self.fetch(uri.as_str()).await?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct MockClient(HashMap<String, String>);

    #[async_trait::async_trait]
    impl HttpClient for MockClient {
        async fn get(&self, uri: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
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

    /// Convert a `Box<dyn Error + Send + Sync>` to `anyhow::Error`.
    #[allow(clippy::needless_pass_by_value)]
    fn boxerr(e: Box<dyn Error + Send + Sync>) -> anyhow::Error {
        anyhow::anyhow!("{e}")
    }

    #[tokio::test]
    async fn fetch_no_cache_dir() -> anyhow::Result<()> {
        let client = mock(&[("https://example.com/s.json", r#"{"type":"object"}"#)]);
        let cache = SchemaCache::new(None, client, false, None);
        let (val, status) = cache
            .fetch("https://example.com/s.json")
            .await
            .map_err(boxerr)?;
        assert_eq!(val, serde_json::json!({"type": "object"}));
        assert_eq!(status, CacheStatus::Disabled);
        Ok(())
    }

    #[tokio::test]
    async fn fetch_cold_cache() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let client = mock(&[("https://example.com/s.json", r#"{"type":"string"}"#)]);
        let cache = SchemaCache::new(Some(tmp.path().to_path_buf()), client, false, None);
        let (val, status) = cache
            .fetch("https://example.com/s.json")
            .await
            .map_err(boxerr)?;
        assert_eq!(val, serde_json::json!({"type": "string"}));
        assert_eq!(status, CacheStatus::Miss);

        // Verify file was written to disk
        let hash = SchemaCache::<MockClient>::hash_uri("https://example.com/s.json");
        let cache_path = tmp.path().join(format!("{hash}.json"));
        assert!(cache_path.exists());
        Ok(())
    }

    #[tokio::test]
    async fn fetch_warm_cache() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let hash = SchemaCache::<MockClient>::hash_uri("https://example.com/s.json");
        let cache_path = tmp.path().join(format!("{hash}.json"));
        fs::write(&cache_path, r#"{"type":"number"}"#)?;

        // Client has no entries — if it were called, it would error
        let client = mock(&[]);
        let cache = SchemaCache::new(Some(tmp.path().to_path_buf()), client, false, None);
        let (val, status) = cache
            .fetch("https://example.com/s.json")
            .await
            .map_err(boxerr)?;
        assert_eq!(val, serde_json::json!({"type": "number"}));
        assert_eq!(status, CacheStatus::Hit);
        Ok(())
    }

    #[tokio::test]
    async fn fetch_skip_read_bypasses_cache() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let hash = SchemaCache::<MockClient>::hash_uri("https://example.com/s.json");
        let cache_path = tmp.path().join(format!("{hash}.json"));
        fs::write(&cache_path, r#"{"type":"number"}"#)?;

        // With skip_read, the cached value is ignored and the client is called
        let client = mock(&[("https://example.com/s.json", r#"{"type":"string"}"#)]);
        let cache = SchemaCache::new(Some(tmp.path().to_path_buf()), client, true, None);
        let (val, status) = cache
            .fetch("https://example.com/s.json")
            .await
            .map_err(boxerr)?;
        assert_eq!(val, serde_json::json!({"type": "string"}));
        assert_eq!(status, CacheStatus::Miss);
        Ok(())
    }

    #[tokio::test]
    async fn fetch_client_error() {
        let client = mock(&[]);
        let cache = SchemaCache::new(None, client, false, None);
        assert!(
            cache
                .fetch("https://example.com/missing.json")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn fetch_invalid_json() {
        let client = mock(&[("https://example.com/bad.json", "not json")]);
        let cache = SchemaCache::new(None, client, false, None);
        assert!(cache.fetch("https://example.com/bad.json").await.is_err());
    }

    #[tokio::test]
    async fn async_retrieve_trait_delegates() -> anyhow::Result<()> {
        let client = mock(&[("https://example.com/s.json", r#"{"type":"object"}"#)]);
        let cache = SchemaCache::new(None, client, false, None);
        let uri: jsonschema::Uri<String> = "https://example.com/s.json".parse()?;
        let val = jsonschema::AsyncRetrieve::retrieve(&cache, &uri)
            .await
            .map_err(boxerr)?;
        assert_eq!(val, serde_json::json!({"type": "object"}));
        Ok(())
    }

    #[tokio::test]
    async fn fetch_expired_ttl_refetches() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let hash = SchemaCache::<MockClient>::hash_uri("https://example.com/s.json");
        let cache_path = tmp.path().join(format!("{hash}.json"));
        fs::write(&cache_path, r#"{"type":"number"}"#)?;

        // Set mtime to 2 seconds ago
        let two_secs_ago = filetime::FileTime::from_system_time(
            std::time::SystemTime::now() - std::time::Duration::from_secs(2),
        );
        filetime::set_file_mtime(&cache_path, two_secs_ago)?;

        // TTL of 1 second — the cached file is stale
        let client = mock(&[("https://example.com/s.json", r#"{"type":"string"}"#)]);
        let cache = SchemaCache::new(
            Some(tmp.path().to_path_buf()),
            client,
            false,
            Some(Duration::from_secs(1)),
        );
        let (val, status) = cache
            .fetch("https://example.com/s.json")
            .await
            .map_err(boxerr)?;
        assert_eq!(val, serde_json::json!({"type": "string"}));
        assert_eq!(status, CacheStatus::Miss);
        Ok(())
    }

    #[tokio::test]
    async fn fetch_unexpired_ttl_serves_cache() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let hash = SchemaCache::<MockClient>::hash_uri("https://example.com/s.json");
        let cache_path = tmp.path().join(format!("{hash}.json"));
        fs::write(&cache_path, r#"{"type":"number"}"#)?;

        // TTL of 1 hour — the file was just written, so it's fresh
        let client = mock(&[]);
        let cache = SchemaCache::new(
            Some(tmp.path().to_path_buf()),
            client,
            false,
            Some(Duration::from_secs(3600)),
        );
        let (val, status) = cache
            .fetch("https://example.com/s.json")
            .await
            .map_err(boxerr)?;
        assert_eq!(val, serde_json::json!({"type": "number"}));
        assert_eq!(status, CacheStatus::Hit);
        Ok(())
    }

    #[test]
    fn default_cache_dir_ends_with_schemas() {
        let dir = default_cache_dir();
        assert!(dir.ends_with("lintel/schemas"));
    }
}
