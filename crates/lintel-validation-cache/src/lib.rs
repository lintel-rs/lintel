use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Whether a validation result was served from the disk cache or freshly computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationCacheStatus {
    /// Validation result was found in the disk cache.
    Hit,
    /// Validation result was computed (cache miss or skip-read mode).
    Miss,
}

#[derive(Serialize, Deserialize)]
struct CachedError {
    instance_path: String,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct CachedResult {
    errors: Vec<CachedError>,
}

/// A disk-backed cache for JSON Schema validation results.
///
/// Results are keyed by `SHA-256(file_content + schema_json + validate_formats_byte)`.
/// Cache files are stored as `<cache_dir>/<sha256-hex>.json`.
#[derive(Clone)]
pub struct ValidationCache {
    cache_dir: PathBuf,
    skip_read: bool,
}

impl ValidationCache {
    pub fn new(cache_dir: PathBuf, skip_read: bool) -> Self {
        Self {
            cache_dir,
            skip_read,
        }
    }

    /// Look up a cached validation result.
    ///
    /// Returns `(Some(errors), Hit)` on cache hit, where each error is
    /// `(instance_path, message)`. Returns `(None, Miss)` on cache miss or
    /// when `skip_read` is set.
    ///
    /// `schema_hash` should be obtained from [`schema_hash`] — pass the same
    /// value for all files in a schema group to avoid redundant serialization.
    pub async fn lookup(
        &self,
        file_content: &str,
        schema_hash: &str,
        validate_formats: bool,
    ) -> (Option<Vec<(String, String)>>, ValidationCacheStatus) {
        if self.skip_read {
            return (None, ValidationCacheStatus::Miss);
        }

        let key = Self::cache_key(file_content, schema_hash, validate_formats);
        let cache_path = self.cache_dir.join(format!("{key}.json"));

        let Ok(data) = tokio::fs::read_to_string(&cache_path).await else {
            return (None, ValidationCacheStatus::Miss);
        };

        let Ok(cached) = serde_json::from_str::<CachedResult>(&data) else {
            return (None, ValidationCacheStatus::Miss);
        };

        let errors: Vec<(String, String)> = cached
            .errors
            .into_iter()
            .map(|e| (e.instance_path, e.message))
            .collect();

        (Some(errors), ValidationCacheStatus::Hit)
    }

    /// Store a validation result to the disk cache.
    ///
    /// Always writes regardless of `skip_read`, so running with
    /// `--force-validation` repopulates the cache for future runs.
    ///
    /// `schema_hash` should be obtained from [`schema_hash`] — pass the same
    /// value for all files in a schema group to avoid redundant serialization.
    pub async fn store(
        &self,
        file_content: &str,
        schema_hash: &str,
        validate_formats: bool,
        errors: &[(String, String)],
    ) {
        let key = Self::cache_key(file_content, schema_hash, validate_formats);
        let cache_path = self.cache_dir.join(format!("{key}.json"));

        let cached = CachedResult {
            errors: errors
                .iter()
                .map(|(ip, msg)| CachedError {
                    instance_path: ip.clone(),
                    message: msg.clone(),
                })
                .collect(),
        };

        let Ok(json) = serde_json::to_string(&cached) else {
            return;
        };

        if tokio::fs::create_dir_all(&self.cache_dir).await.is_ok() {
            let _ = tokio::fs::write(&cache_path, json).await;
        }
    }

    /// Compute the SHA-256 cache key from file content, a pre-computed schema hash, and format flag.
    fn cache_key(file_content: &str, schema_hash: &str, validate_formats: bool) -> String {
        let mut hasher = Sha256::new();
        hasher.update(file_content.as_bytes());
        hasher.update(schema_hash.as_bytes());
        hasher.update([u8::from(validate_formats)]);
        format!("{:x}", hasher.finalize())
    }
}

/// Compute a SHA-256 hash of a schema `Value`.
///
/// Call this once per schema group and pass the result to
/// [`ValidationCache::lookup`] and [`ValidationCache::store`].
pub fn schema_hash(schema: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(schema.to_string().as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Return a usable cache directory for validation results, creating it if necessary.
///
/// Tries `<system_cache>/lintel/validations` first, falling back to
/// `<temp_dir>/lintel/validations` when the preferred path is unwritable.
pub fn ensure_cache_dir() -> PathBuf {
    let candidates = [
        dirs::cache_dir().map(|d| d.join("lintel").join("validations")),
        Some(std::env::temp_dir().join("lintel").join("validations")),
    ];
    for candidate in candidates.into_iter().flatten() {
        if std::fs::create_dir_all(&candidate).is_ok() {
            return candidate;
        }
    }
    std::env::temp_dir().join("lintel").join("validations")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_schema() -> Value {
        serde_json::json!({"type": "object", "properties": {"name": {"type": "string"}}})
    }

    #[test]
    fn cache_key_deterministic() {
        let hash = schema_hash(&sample_schema());
        let a = ValidationCache::cache_key("hello", &hash, true);
        let b = ValidationCache::cache_key("hello", &hash, true);
        assert_eq!(a, b);
    }

    #[test]
    fn cache_key_differs_on_content() {
        let hash = schema_hash(&sample_schema());
        let a = ValidationCache::cache_key("hello", &hash, true);
        let b = ValidationCache::cache_key("world", &hash, true);
        assert_ne!(a, b);
    }

    #[test]
    fn cache_key_differs_on_schema() {
        let hash_a = schema_hash(&sample_schema());
        let hash_b = schema_hash(&serde_json::json!({"type": "string"}));
        let a = ValidationCache::cache_key("hello", &hash_a, true);
        let b = ValidationCache::cache_key("hello", &hash_b, true);
        assert_ne!(a, b);
    }

    #[test]
    fn cache_key_differs_on_formats() {
        let hash = schema_hash(&sample_schema());
        let a = ValidationCache::cache_key("hello", &hash, true);
        let b = ValidationCache::cache_key("hello", &hash, false);
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn store_and_lookup() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let cache = ValidationCache::new(tmp.path().to_path_buf(), false);
        let hash = schema_hash(&sample_schema());

        let errors = vec![("/name".to_string(), "missing required property".to_string())];
        cache.store("content", &hash, true, &errors).await;

        let (result, status) = cache.lookup("content", &hash, true).await;
        assert_eq!(status, ValidationCacheStatus::Hit);
        let result = result.expect("expected cache hit");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "/name");
        assert_eq!(result[0].1, "missing required property");
        Ok(())
    }

    #[tokio::test]
    async fn lookup_miss() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let cache = ValidationCache::new(tmp.path().to_path_buf(), false);
        let hash = schema_hash(&sample_schema());

        let (result, status) = cache.lookup("content", &hash, true).await;
        assert_eq!(status, ValidationCacheStatus::Miss);
        assert!(result.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn skip_read_forces_miss() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let cache_write = ValidationCache::new(tmp.path().to_path_buf(), false);
        let cache_skip = ValidationCache::new(tmp.path().to_path_buf(), true);
        let hash = schema_hash(&sample_schema());

        // Store a result
        cache_write.store("content", &hash, true, &[]).await;

        // With skip_read, lookup always returns miss
        let (result, status) = cache_skip.lookup("content", &hash, true).await;
        assert_eq!(status, ValidationCacheStatus::Miss);
        assert!(result.is_none());

        // But store still writes (verify by reading with non-skip cache)
        cache_skip
            .store(
                "other",
                &hash,
                true,
                &[("path".to_string(), "msg".to_string())],
            )
            .await;
        let (result, status) = cache_write.lookup("other", &hash, true).await;
        assert_eq!(status, ValidationCacheStatus::Hit);
        assert!(result.is_some());
        Ok(())
    }

    #[test]
    fn ensure_cache_dir_ends_with_validations() {
        let dir = ensure_cache_dir();
        assert!(dir.ends_with("lintel/validations"));
    }
}
