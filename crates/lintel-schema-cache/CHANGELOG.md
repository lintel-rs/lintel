# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.11](https://github.com/lintel-rs/lintel/compare/lintel-schema-cache-v0.0.10...lintel-schema-cache-v0.0.11) - 2026-02-26

### Other

- Centralize workspace dependencies in root Cargo.toml
- Merge origin/master into site-generator
- Add static site generator with ProcessedSchemas for in-memory schema lookups

## [0.0.10](https://github.com/lintel-rs/lintel/compare/lintel-schema-cache-v0.0.9...lintel-schema-cache-v0.0.10) - 2026-02-24

### Other

- Merge pull request #90 from lintel-rs/faster-builder
- Add x-lintel metadata with source URL and content hash to output schemas
- Unify HTTP concurrency control via semaphore in SchemaCache

## [0.0.9](https://github.com/lintel-rs/lintel/compare/lintel-schema-cache-v0.0.8...lintel-schema-cache-v0.0.9) - 2026-02-24

### Added

- add jsonschema-migrate crate, per-group _shared dirs, and update README badges

## [0.0.8](https://github.com/lintel-rs/lintel/compare/lintel-schema-cache-v0.0.7...lintel-schema-cache-v0.0.8) - 2026-02-22

### Added

- add download logging, $id injection, and cache status visibility

## [0.0.7](https://github.com/lintel-rs/lintel/compare/lintel-schema-cache-v0.0.6...lintel-schema-cache-v0.0.7) - 2026-02-21

### Added

- update all dependencies to latest versions

## [0.0.6](https://github.com/lintel-rs/lintel/compare/lintel-schema-cache-v0.0.5...lintel-schema-cache-v0.0.6) - 2026-02-21

### Other

- add SchemaCacheBuilder and centralize TTL defaulting
- Merge remote-tracking branch 'origin/master' into fix-more-stuff
- rename lintel-http-cache back to lintel-schema-cache and HttpCache to SchemaCache
- rename lintel-schema-cache to lintel-http-cache and simplify API

## [0.0.5](https://github.com/lintel-rs/lintel/compare/lintel-schema-cache-v0.0.4...lintel-schema-cache-v0.0.5) - 2026-02-21

### Added

- add `lintel cache` command and migrate file reads to tokio

### Fixed

- correct README code examples to pass doctests

### Other

- Merge remote-tracking branch 'origin/master' into claude-skill
- Clean up all crates: fix section ordering, READMEs, and metadata
- Use inline badges in READMEs and add doc includes
- deny std_instead_of_alloc and std_instead_of_core clippy lints

## [0.0.4](https://github.com/lintel-rs/lintel/compare/lintel-schema-cache-v0.0.3...lintel-schema-cache-v0.0.4) - 2026-02-20

### Other

- Remove $schema comments from all Cargo.toml files

## [0.0.3](https://github.com/lintel-rs/lintel/compare/lintel-schema-cache-v0.0.2...lintel-schema-cache-v0.0.3) - 2026-02-20

### Other

- Merge pull request #11 from lintel-rs/lintel-wt2
- Add per-crate READMEs with badges and inherit workspace package metadata
- Fix cache dir creation with temp_dir fallback and use async tokio::fs for cache I/O
- Optimize catalog matching and schema fetching performance
- Add tracing instrumentation and generate benchmark results
- Add validation cache, schema cache TTL, and benchmark tooling

## [0.0.2](https://github.com/lintel-rs/lintel/compare/lintel-schema-cache-v0.0.1...lintel-schema-cache-v0.0.2) - 2026-02-19

### Other

- Extract lintel-config crate with build-time schema generation
- Fix clippy pedantic warnings and deny unwrap_used
- Add clippy::pedantic lint to workspace
- release v0.0.1

## [0.0.1](https://github.com/lintel-rs/lintel/releases/tag/lintel-schema-cache-v0.0.1) - 2026-02-19

### Other

- Add LICENSE file and author metadata to all crates
- Change license from MIT to Apache-2.0 across all crates and npm package
- Initial commit
