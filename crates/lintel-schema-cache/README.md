# lintel-schema-cache

[![Crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]
[![License][license-badge]][license-url]

[crates-badge]: https://img.shields.io/crates/v/lintel-schema-cache.svg
[crates-url]: https://crates.io/crates/lintel-schema-cache
[docs-badge]: https://docs.rs/lintel-schema-cache/badge.svg
[docs-url]: https://docs.rs/lintel-schema-cache
[license-badge]: https://img.shields.io/crates/l/lintel-schema-cache.svg
[license-url]: https://github.com/lintel-rs/lintel/blob/master/LICENSE

Disk-backed cache for JSON Schema files. Fetches schemas over HTTP and stores them locally for fast subsequent lookups.

## Features

- **Transparent caching** — schemas are stored as `<cache_dir>/<hash>.json` where `<hash>` is a deterministic hash of the URI
- **Pluggable HTTP** — bring your own HTTP client via the `HttpClient` trait, or use the built-in `UreqClient`
- **jsonschema integration** — implements both `jsonschema::Retrieve` and `jsonschema::AsyncRetrieve` for seamless use as a schema resolver

## Usage

```rust
use lintel_schema_cache::{SchemaCache, UreqClient, default_cache_dir};

let cache = SchemaCache::new(Some(default_cache_dir()), UreqClient);
let (schema, status) = cache.fetch("https://json.schemastore.org/tsconfig.json")?;
// status: Hit (from disk), Miss (fetched and cached), or Disabled (no cache dir)
```

## Custom HTTP Client

```rust
use lintel_schema_cache::{SchemaCache, HttpClient};

#[derive(Clone)]
struct MyClient;

impl HttpClient for MyClient {
    fn get(&self, uri: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // your HTTP implementation
        todo!()
    }
}

let cache = SchemaCache::new(None, MyClient);
```
