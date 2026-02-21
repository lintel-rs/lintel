# lintel-http-cache

[![Crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]
[![License][license-badge]][license-url]

[crates-badge]: https://img.shields.io/crates/v/lintel-http-cache.svg
[crates-url]: https://crates.io/crates/lintel-http-cache
[docs-badge]: https://docs.rs/lintel-http-cache/badge.svg
[docs-url]: https://docs.rs/lintel-http-cache
[license-badge]: https://img.shields.io/crates/l/lintel-http-cache.svg
[license-url]: https://github.com/lintel-rs/lintel/blob/master/LICENSE

Disk-backed HTTP cache with JSON parsing for schema files. Fetches JSON over HTTP and stores results locally for fast subsequent lookups.

## Features

- **SHA-256 keyed caching** — schemas are stored as `<cache_dir>/<sha256>.json` where `<sha256>` is the hex digest of the URI, avoiding hash collisions
- **Conditional requests** — uses `ETag` / `If-None-Match` headers to avoid re-downloading unchanged schemas
- **TTL support** — configurable time-to-live for cache entries based on file modification time
- **In-memory layer** — frequently accessed schemas are also kept in memory for zero-IO lookups
- **jsonschema integration** — implements `jsonschema::AsyncRetrieve` for seamless use as a schema resolver
- **Test-friendly** — `HttpCache::memory()` constructor creates a memory-only cache with no HTTP or disk I/O

## Usage

```rust
use lintel_http_cache::{HttpCache, ensure_cache_dir};
use std::time::Duration;

let cache = HttpCache::new(
    Some(ensure_cache_dir()),
    false,                                   // skip_read
    Some(Duration::from_secs(12 * 60 * 60)), // TTL
);
let (schema, status) = cache.fetch("https://json.schemastore.org/tsconfig.json").await?;
// status: Hit (from disk/memory), Miss (fetched and cached), or Disabled (no cache dir)
```

## Testing

Use the memory-only constructor to avoid network and disk I/O in tests:

```rust
use lintel_http_cache::HttpCache;

let cache = HttpCache::memory();
cache.insert("https://example.com/schema.json", serde_json::json!({"type": "object"}));
let (val, _status) = cache.fetch("https://example.com/schema.json").await?;
```
