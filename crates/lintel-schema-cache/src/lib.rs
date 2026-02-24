#![doc = include_str!("../README.md")]

extern crate alloc;

use alloc::sync::Arc;
use core::error::Error;
use core::time::Duration;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Default TTL for cached schemas (12 hours).
pub const DEFAULT_SCHEMA_CACHE_TTL: Duration = Duration::from_secs(12 * 60 * 60);

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

impl core::fmt::Display for CacheStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Hit => f.write_str("cached"),
            Self::Miss => f.write_str("fetched"),
            Self::Disabled => f.write_str("fetched (no cache)"),
        }
    }
}

/// Response from a conditional HTTP request.
struct ConditionalResponse {
    /// Response body. `None` indicates a 304 Not Modified response.
    body: Option<String>,
    /// `ETag` header from the response, if present.
    etag: Option<String>,
}

/// Internal HTTP backend.
enum HttpMode {
    /// Production mode — uses reqwest for HTTP requests.
    Reqwest(reqwest::Client),
    /// Test mode — no HTTP, no disk. Only serves from memory cache.
    Memory,
}

/// A disk-backed schema cache with HTTP fetching and JSON parsing.
///
/// Schemas are fetched via HTTP and stored as `<cache_dir>/<hash>.json`
/// where `<hash>` is a SHA-256 hex digest of the URI. When a schema is
/// requested, the cache is checked first; on a miss the schema is fetched
/// and written to disk for future use.
#[derive(Clone)]
pub struct SchemaCache {
    cache_dir: Option<PathBuf>,
    http: Arc<HttpMode>,
    skip_read: bool,
    ttl: Option<Duration>,
    /// In-memory cache shared across all clones via `Arc`.
    memory_cache: Arc<Mutex<HashMap<String, Value>>>,
    /// SHA-256 hex digests of the raw content fetched for each URI.
    content_hashes: Arc<Mutex<HashMap<String, String>>>,
    /// Semaphore that limits concurrent HTTP requests across all callers.
    http_semaphore: Arc<tokio::sync::Semaphore>,
}

/// Builder for constructing a [`SchemaCache`] with sensible defaults.
///
/// Defaults:
/// - `cache_dir`: [`ensure_cache_dir()`]
/// - `force_fetch`: `false`
/// - `ttl`: [`DEFAULT_SCHEMA_CACHE_TTL`] (12 hours)
///
/// # Examples
///
/// ```rust,ignore
/// let cache = SchemaCache::builder().build();
/// let cache = SchemaCache::builder().force_fetch(true).ttl(Duration::from_secs(3600)).build();
/// ```
/// Default maximum number of concurrent HTTP requests.
const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 20;

#[must_use]
pub struct SchemaCacheBuilder {
    cache_dir: Option<PathBuf>,
    skip_read: bool,
    ttl: Option<Duration>,
    max_concurrent_requests: usize,
}

impl SchemaCacheBuilder {
    /// Override the default cache directory.
    pub fn cache_dir(mut self, dir: PathBuf) -> Self {
        self.cache_dir = Some(dir);
        self
    }

    /// When `true`, bypass cache reads and always fetch from the network.
    /// Fetched schemas are still written to the cache.
    pub fn force_fetch(mut self, force: bool) -> Self {
        self.skip_read = force;
        self
    }

    /// Override the default TTL for cached schemas.
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the maximum number of concurrent HTTP requests.
    pub fn max_concurrent_requests(mut self, n: usize) -> Self {
        self.max_concurrent_requests = n;
        self
    }

    /// Returns the cache directory that will be used, or [`ensure_cache_dir()`]
    /// if none was explicitly set.
    ///
    /// Useful when callers need the resolved path before calling [`build`](Self::build).
    pub fn cache_dir_or_default(&self) -> PathBuf {
        self.cache_dir.clone().unwrap_or_else(ensure_cache_dir)
    }

    /// Build the [`SchemaCache`].
    pub fn build(self) -> SchemaCache {
        SchemaCache {
            cache_dir: self.cache_dir,
            http: Arc::new(HttpMode::Reqwest(reqwest::Client::new())),
            skip_read: self.skip_read,
            ttl: self.ttl,
            memory_cache: Arc::new(Mutex::new(HashMap::new())),
            content_hashes: Arc::new(Mutex::new(HashMap::new())),
            http_semaphore: Arc::new(tokio::sync::Semaphore::new(self.max_concurrent_requests)),
        }
    }
}

impl SchemaCache {
    /// Returns a builder pre-configured with sensible defaults.
    ///
    /// - `cache_dir` = [`ensure_cache_dir()`]
    /// - `ttl` = [`DEFAULT_SCHEMA_CACHE_TTL`]
    /// - `force_fetch` = `false`
    pub fn builder() -> SchemaCacheBuilder {
        SchemaCacheBuilder {
            cache_dir: Some(ensure_cache_dir()),
            skip_read: false,
            ttl: Some(DEFAULT_SCHEMA_CACHE_TTL),
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
        }
    }

    /// Test constructor — memory-only, no HTTP, no disk.
    ///
    /// Pre-populate with [`insert`](Self::insert). Calls to [`fetch`](Self::fetch)
    /// for unknown URIs will error.
    pub fn memory() -> Self {
        Self {
            cache_dir: None,
            http: Arc::new(HttpMode::Memory),
            skip_read: false,
            ttl: None,
            memory_cache: Arc::new(Mutex::new(HashMap::new())),
            content_hashes: Arc::new(Mutex::new(HashMap::new())),
            http_semaphore: Arc::new(tokio::sync::Semaphore::new(DEFAULT_MAX_CONCURRENT_REQUESTS)),
        }
    }

    /// Insert a value into the in-memory cache (useful for tests).
    #[allow(clippy::missing_panics_doc)] // Mutex poisoning is unreachable
    pub fn insert(&self, uri: &str, value: Value) {
        self.memory_cache
            .lock()
            .expect("memory cache poisoned")
            .insert(uri.to_string(), value);
    }

    /// Look up a schema by URI from the in-memory cache only.
    ///
    /// Returns `None` if the URI is not in memory. Does not check disk cache
    /// or fetch from the network.
    #[allow(clippy::missing_panics_doc)] // Mutex poisoning is unreachable
    pub fn get(&self, uri: &str) -> Option<Value> {
        self.memory_cache
            .lock()
            .expect("memory cache poisoned")
            .get(uri)
            .cloned()
    }

    /// Return the SHA-256 hex digest of the raw content last fetched for `uri`.
    ///
    /// Returns `None` if the URI has not been fetched or was inserted via
    /// [`insert`](Self::insert) (which has no raw content to hash).
    #[allow(clippy::missing_panics_doc)] // Mutex poisoning is unreachable
    pub fn content_hash(&self, uri: &str) -> Option<String> {
        self.content_hashes
            .lock()
            .expect("content hashes poisoned")
            .get(uri)
            .cloned()
    }

    /// Compute SHA-256 of raw content and store it keyed by URI.
    fn store_content_hash(&self, uri: &str, content: &str) {
        let hash = Self::hash_content(content);
        self.content_hashes
            .lock()
            .expect("content hashes poisoned")
            .insert(uri.to_string(), hash);
    }

    /// Compute the SHA-256 hash of arbitrary content, returned as a 64-char hex string.
    pub fn hash_content(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
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
    #[tracing::instrument(level = "debug", skip(self), fields(status))]
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

        // Memory-only mode: if not in cache, error out.
        if matches!(*self.http, HttpMode::Memory) {
            return Err(format!("memory-only cache: no entry for {uri}").into());
        }

        // Check disk cache (unless skip_read is set)
        let mut stored_etag: Option<String> = None;
        let mut cached_content: Option<String> = None;

        if let Some(ref cache_dir) = self.cache_dir {
            let hash = Self::hash_uri(uri);
            let cache_path = cache_dir.join(format!("{hash}.json"));
            let etag_path = cache_dir.join(format!("{hash}.etag"));

            if cache_path.exists() {
                if !self.skip_read && !self.is_expired(&cache_path) {
                    // Fresh cache — return immediately
                    if let Ok(content) = tokio::fs::read_to_string(&cache_path).await
                        && let Ok(value) = serde_json::from_str::<Value>(&content)
                    {
                        self.store_content_hash(uri, &content);
                        self.memory_cache
                            .lock()
                            .expect("memory cache poisoned")
                            .insert(uri.to_string(), value.clone());
                        tracing::Span::current().record("status", "cache_hit");
                        return Ok((value, CacheStatus::Hit));
                    }
                }

                // Stale or skip_read — read ETag for conditional fetch
                if let Ok(etag) = tokio::fs::read_to_string(&etag_path).await {
                    stored_etag = Some(etag);
                }
                // Keep cached content for 304 fallback
                if let Ok(content) = tokio::fs::read_to_string(&cache_path).await {
                    cached_content = Some(content);
                }
            }
        }

        // Acquire a permit before making the HTTP request
        let _permit = self
            .http_semaphore
            .acquire()
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Conditional network fetch
        tracing::Span::current().record("status", "network_fetch");
        let conditional = self.get_conditional(uri, stored_etag.as_deref()).await?;

        if conditional.body.is_none() {
            // 304 Not Modified — use cached content
            if let Some(content) = cached_content {
                let value: Value = serde_json::from_str(&content)?;
                self.store_content_hash(uri, &content);
                self.memory_cache
                    .lock()
                    .expect("memory cache poisoned")
                    .insert(uri.to_string(), value.clone());

                // Touch the cache file to reset TTL
                if let Some(ref cache_dir) = self.cache_dir {
                    let hash = Self::hash_uri(uri);
                    let cache_path = cache_dir.join(format!("{hash}.json"));
                    let now = filetime::FileTime::now();
                    let _ = filetime::set_file_mtime(&cache_path, now);
                }

                tracing::Span::current().record("status", "etag_hit");
                return Ok((value, CacheStatus::Hit));
            }
        }

        let body = conditional.body.expect("non-304 response must have a body");
        let value: Value = serde_json::from_str(&body)?;
        self.store_content_hash(uri, &body);

        // Populate in-memory cache
        self.memory_cache
            .lock()
            .expect("memory cache poisoned")
            .insert(uri.to_string(), value.clone());

        let status = if let Some(ref cache_dir) = self.cache_dir {
            let hash = Self::hash_uri(uri);
            let cache_path = cache_dir.join(format!("{hash}.json"));
            let etag_path = cache_dir.join(format!("{hash}.etag"));
            if let Err(e) = tokio::fs::write(&cache_path, &body).await {
                tracing::warn!(
                    path = %cache_path.display(),
                    error = %e,
                    "failed to write schema to disk cache"
                );
            }
            // Write ETag if present
            if let Some(etag) = conditional.etag {
                let _ = tokio::fs::write(&etag_path, &etag).await;
            }
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

    /// Compute the SHA-256 hash of a URI, returned as a 64-char hex string.
    pub fn hash_uri(uri: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(uri.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Internal: perform a conditional GET using reqwest.
    async fn get_conditional(
        &self,
        uri: &str,
        etag: Option<&str>,
    ) -> Result<ConditionalResponse, Box<dyn Error + Send + Sync>> {
        let HttpMode::Reqwest(ref client) = *self.http else {
            return Err("HTTP not available in memory-only mode".into());
        };

        let mut req = client.get(uri);
        if let Some(etag) = etag {
            req = req.header("If-None-Match", etag);
        }
        let resp = req.send().await?;
        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(ConditionalResponse {
                body: None,
                etag: None,
            });
        }
        let resp = resp.error_for_status()?;
        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let body = resp.text().await?;
        Ok(ConditionalResponse {
            body: Some(body),
            etag,
        })
    }
}

/// Return a usable cache directory for schemas, creating it if necessary.
///
/// Tries `<system_cache>/lintel/schemas` first, falling back to
/// `<temp_dir>/lintel/schemas` when the preferred path is unwritable.
pub fn ensure_cache_dir() -> PathBuf {
    let candidates = [
        dirs::cache_dir().map(|d| d.join("lintel").join("schemas")),
        Some(std::env::temp_dir().join("lintel").join("schemas")),
    ];
    for candidate in candidates.into_iter().flatten() {
        if fs::create_dir_all(&candidate).is_ok() {
            return candidate;
        }
    }
    std::env::temp_dir().join("lintel").join("schemas")
}

// -- jsonschema trait impls --------------------------------------------------

#[async_trait::async_trait]
impl jsonschema::AsyncRetrieve for SchemaCache {
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

    #[test]
    fn hash_uri_deterministic() {
        let a = SchemaCache::hash_uri("https://example.com/schema.json");
        let b = SchemaCache::hash_uri("https://example.com/schema.json");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_uri_different_inputs() {
        let a = SchemaCache::hash_uri("https://example.com/a.json");
        let b = SchemaCache::hash_uri("https://example.com/b.json");
        assert_ne!(a, b);
    }

    #[test]
    fn hash_uri_is_64_hex_chars() {
        let h = SchemaCache::hash_uri("https://example.com/schema.json");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// Convert a `Box<dyn Error + Send + Sync>` to `anyhow::Error`.
    #[allow(clippy::needless_pass_by_value)]
    fn boxerr(e: Box<dyn Error + Send + Sync>) -> anyhow::Error {
        anyhow::anyhow!("{e}")
    }

    #[tokio::test]
    async fn memory_cache_insert_and_fetch() -> anyhow::Result<()> {
        let cache = SchemaCache::memory();
        cache.insert(
            "https://example.com/s.json",
            serde_json::json!({"type": "object"}),
        );
        let (val, status) = cache
            .fetch("https://example.com/s.json")
            .await
            .map_err(boxerr)?;
        assert_eq!(val, serde_json::json!({"type": "object"}));
        assert_eq!(status, CacheStatus::Hit);
        Ok(())
    }

    #[tokio::test]
    async fn memory_cache_missing_uri_errors() {
        let cache = SchemaCache::memory();
        assert!(
            cache
                .fetch("https://example.com/missing.json")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn async_retrieve_trait_delegates() -> anyhow::Result<()> {
        let cache = SchemaCache::memory();
        cache.insert(
            "https://example.com/s.json",
            serde_json::json!({"type": "object"}),
        );
        let uri: jsonschema::Uri<String> = "https://example.com/s.json".parse()?;
        let val = jsonschema::AsyncRetrieve::retrieve(&cache, &uri)
            .await
            .map_err(boxerr)?;
        assert_eq!(val, serde_json::json!({"type": "object"}));
        Ok(())
    }

    #[test]
    fn ensure_cache_dir_ends_with_schemas() {
        let dir = ensure_cache_dir();
        assert!(dir.ends_with("lintel/schemas"));
    }
}
