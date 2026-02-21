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
use lintel_validation_cache::ValidationCache;

let cache = ValidationCache::new(Some(cache_dir)).await?;

// Check if a result is cached
if let Some(result) = cache.get(&file_hash, &schema_uri).await? {
    // Use cached result
}

// Store a new result
cache.set(&file_hash, &schema_uri, &result).await?;
```

## License

Apache-2.0
