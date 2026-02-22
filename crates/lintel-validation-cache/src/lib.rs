#![doc = include_str!("../README.md")]

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

/// A single validation error with its location and schema context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationError {
    /// JSON Pointer to the failing instance (e.g. `/jobs/build`).
    pub instance_path: String,
    /// Human-readable error message.
    pub message: String,
    /// JSON Schema path that triggered the error (e.g. `/properties/jobs/oneOf`).
    #[serde(default)]
    pub schema_path: String,
}

/// The cache lookup/store key: file content, schema hash, and format-validation flag.
pub struct CacheKey<'a> {
    /// The raw file content being validated.
    pub file_content: &'a str,
    /// Pre-computed SHA-256 hash of the schema (see [`schema_hash`]).
    pub schema_hash: &'a str,
    /// Whether format validation was enabled.
    pub validate_formats: bool,
}

#[derive(Serialize, Deserialize)]
struct CachedResult {
    errors: Vec<ValidationError>,
}

/// A disk-backed cache for JSON Schema validation results.
///
/// Results are keyed by `SHA-256(crate_version + file_content + schema_json + validate_formats_byte)`.
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
    /// Returns `(Some(errors), Hit)` on cache hit.
    /// Returns `(None, Miss)` on cache miss or when `skip_read` is set.
    ///
    /// `key.schema_hash` should be obtained from [`schema_hash`] — pass the same
    /// value for all files in a schema group to avoid redundant serialization.
    pub async fn lookup(
        &self,
        key: &CacheKey<'_>,
    ) -> (Option<Vec<ValidationError>>, ValidationCacheStatus) {
        if self.skip_read {
            return (None, ValidationCacheStatus::Miss);
        }

        let hash = Self::cache_key(key);
        let cache_path = self.cache_dir.join(format!("{hash}.json"));

        let Ok(data) = tokio::fs::read_to_string(&cache_path).await else {
            return (None, ValidationCacheStatus::Miss);
        };

        let Ok(cached) = serde_json::from_str::<CachedResult>(&data) else {
            return (None, ValidationCacheStatus::Miss);
        };

        (Some(cached.errors), ValidationCacheStatus::Hit)
    }

    /// Store a validation result to the disk cache.
    ///
    /// Always writes regardless of `skip_read`, so running with
    /// `--force-validation` repopulates the cache for future runs.
    ///
    /// `key.schema_hash` should be obtained from [`schema_hash`] — pass the same
    /// value for all files in a schema group to avoid redundant serialization.
    pub async fn store(&self, key: &CacheKey<'_>, errors: &[ValidationError]) {
        let hash = Self::cache_key(key);
        let cache_path = self.cache_dir.join(format!("{hash}.json"));

        let cached = CachedResult {
            errors: errors.to_vec(),
        };

        let Ok(json) = serde_json::to_string(&cached) else {
            return;
        };

        if tokio::fs::create_dir_all(&self.cache_dir).await.is_ok() {
            let _ = tokio::fs::write(&cache_path, json).await;
        }
    }

    /// Compute the SHA-256 cache key from a [`CacheKey`].
    ///
    /// The crate version is included in the hash so that upgrading lintel
    /// automatically invalidates stale cache entries.
    pub fn cache_key(key: &CacheKey<'_>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
        hasher.update(key.file_content.as_bytes());
        hasher.update(key.schema_hash.as_bytes());
        hasher.update([u8::from(key.validate_formats)]);
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
        let key = CacheKey {
            file_content: "hello",
            schema_hash: &hash,
            validate_formats: true,
        };
        let a = ValidationCache::cache_key(&key);
        let b = ValidationCache::cache_key(&key);
        assert_eq!(a, b);
    }

    #[test]
    fn cache_key_differs_on_content() {
        let hash = schema_hash(&sample_schema());
        let a = ValidationCache::cache_key(&CacheKey {
            file_content: "hello",
            schema_hash: &hash,
            validate_formats: true,
        });
        let b = ValidationCache::cache_key(&CacheKey {
            file_content: "world",
            schema_hash: &hash,
            validate_formats: true,
        });
        assert_ne!(a, b);
    }

    #[test]
    fn cache_key_differs_on_schema() {
        let hash_a = schema_hash(&sample_schema());
        let hash_b = schema_hash(&serde_json::json!({"type": "string"}));
        let a = ValidationCache::cache_key(&CacheKey {
            file_content: "hello",
            schema_hash: &hash_a,
            validate_formats: true,
        });
        let b = ValidationCache::cache_key(&CacheKey {
            file_content: "hello",
            schema_hash: &hash_b,
            validate_formats: true,
        });
        assert_ne!(a, b);
    }

    #[test]
    fn cache_key_differs_on_formats() {
        let hash = schema_hash(&sample_schema());
        let a = ValidationCache::cache_key(&CacheKey {
            file_content: "hello",
            schema_hash: &hash,
            validate_formats: true,
        });
        let b = ValidationCache::cache_key(&CacheKey {
            file_content: "hello",
            schema_hash: &hash,
            validate_formats: false,
        });
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn store_and_lookup() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let cache = ValidationCache::new(tmp.path().to_path_buf(), false);
        let hash = schema_hash(&sample_schema());

        let errors = vec![ValidationError {
            instance_path: "/name".to_string(),
            message: "missing required property".to_string(),
            schema_path: "/required".to_string(),
        }];
        let key = CacheKey {
            file_content: "content",
            schema_hash: &hash,
            validate_formats: true,
        };
        cache.store(&key, &errors).await;

        let (result, status) = cache.lookup(&key).await;
        assert_eq!(status, ValidationCacheStatus::Hit);
        let result = result.expect("expected cache hit");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].instance_path, "/name");
        assert_eq!(result[0].message, "missing required property");
        assert_eq!(result[0].schema_path, "/required");
        Ok(())
    }

    #[tokio::test]
    async fn lookup_miss() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let cache = ValidationCache::new(tmp.path().to_path_buf(), false);
        let hash = schema_hash(&sample_schema());

        let key = CacheKey {
            file_content: "content",
            schema_hash: &hash,
            validate_formats: true,
        };
        let (result, status) = cache.lookup(&key).await;
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
        let key = CacheKey {
            file_content: "content",
            schema_hash: &hash,
            validate_formats: true,
        };
        cache_write.store(&key, &[]).await;

        // With skip_read, lookup always returns miss
        let (result, status) = cache_skip.lookup(&key).await;
        assert_eq!(status, ValidationCacheStatus::Miss);
        assert!(result.is_none());

        // But store still writes (verify by reading with non-skip cache)
        let key_other = CacheKey {
            file_content: "other",
            schema_hash: &hash,
            validate_formats: true,
        };
        cache_skip
            .store(
                &key_other,
                &[ValidationError {
                    instance_path: "path".to_string(),
                    message: "msg".to_string(),
                    schema_path: String::new(),
                }],
            )
            .await;
        let (result, status) = cache_write.lookup(&key_other).await;
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
