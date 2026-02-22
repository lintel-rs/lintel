# lintel-validation-cache

[![Crates.io](https://img.shields.io/crates/v/lintel-validation-cache.svg)](https://crates.io/crates/lintel-validation-cache)
[![docs.rs](https://docs.rs/lintel-validation-cache/badge.svg)](https://docs.rs/lintel-validation-cache)
[![CI](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg)](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/lintel-validation-cache.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Disk-backed cache for JSON Schema validation results. Caches the outcome of validating a file against a schema so that unchanged files can skip re-validation on subsequent runs.

Part of the [Lintel](https://github.com/lintel-rs/lintel) project.

## How it works

Each cache entry is keyed by a SHA-256 digest of the file contents and schema URI. When a file hasn't changed since the last run, the cached validation result is returned instantly — no parsing or schema evaluation needed.

## Usage

```rust
use lintel_validation_cache::{ValidationCache, CacheKey, schema_hash, ensure_cache_dir};

let cache = ValidationCache::new(ensure_cache_dir(), false);

// Compute a schema hash once per schema group
let schema = serde_json::json!({"type": "object"});
let hash = schema_hash(&schema);

// Cache key = SHA-256(file_content + schema_hash + validate_formats)
let ck = CacheKey { file_content: "file contents", schema_hash: &hash, validate_formats: true };
let key = ValidationCache::cache_key(&ck);
drop(key);
```

## License

Apache-2.0
