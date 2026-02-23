# lintel-schema-cache

[![Crates.io](https://img.shields.io/crates/v/lintel-schema-cache.svg)](https://crates.io/crates/lintel-schema-cache)
[![docs.rs](https://docs.rs/lintel-schema-cache/badge.svg)](https://docs.rs/lintel-schema-cache)
[![GitHub](https://img.shields.io/github/stars/lintel-rs/lintel?style=flat)](https://github.com/lintel-rs/lintel)
[![License](https://img.shields.io/crates/l/lintel-schema-cache.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Disk-backed schema cache with HTTP fetching and JSON parsing. Part of the [Lintel](https://github.com/lintel-rs/lintel) project. Fetches JSON schemas over HTTP and stores results locally for fast subsequent lookups.

## Features

- **SHA-256 keyed caching** — schemas are stored as `<cache_dir>/<sha256>.json` where `<sha256>` is the hex digest of the URI, avoiding hash collisions
- **Conditional requests** — uses `ETag` / `If-None-Match` headers to avoid re-downloading unchanged schemas
- **TTL support** — configurable time-to-live for cache entries based on file modification time
- **In-memory layer** — frequently accessed schemas are also kept in memory for zero-IO lookups
- **jsonschema integration** — implements `jsonschema::AsyncRetrieve` for seamless use as a schema resolver
- **Test-friendly** — `SchemaCache::memory()` constructor creates a memory-only cache with no HTTP or disk I/O

## Usage

```rust,ignore
use lintel_schema_cache::SchemaCache;
use std::time::Duration;

// Uses sensible defaults: system cache dir, 12h TTL
let cache = SchemaCache::builder().build();

// Or customize:
let cache = SchemaCache::builder()
    .force_fetch(true)
    .ttl(Duration::from_secs(3600))
    .build();

let (schema, status) = cache.fetch("https://json.schemastore.org/tsconfig.json").await?;
// status: Hit (from disk/memory), Miss (fetched and cached), or Disabled (no cache dir)
```

## Testing

Use the memory-only constructor to avoid network and disk I/O in tests:

```rust,ignore
use lintel_schema_cache::SchemaCache;

let cache = SchemaCache::memory();
cache.insert("https://example.com/schema.json", serde_json::json!({"type": "object"}));
let (val, _status) = cache.fetch("https://example.com/schema.json").await?;
```

## License

Apache-2.0
