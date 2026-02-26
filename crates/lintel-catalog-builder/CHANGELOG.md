# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.10](https://github.com/lintel-rs/lintel/compare/lintel-catalog-builder-v0.0.9...lintel-catalog-builder-v0.0.10) - 2026-02-26

### Other

- Centralize workspace dependencies in root Cargo.toml
- Add site config, JSON-LD structured data, number formatting, and footer version
- Add exclude-matches to source config and fix ref filename extensions
- Merge origin/master into site-generator
- Add static site generator with ProcessedSchemas for in-memory schema lookups

## [0.0.9](https://github.com/lintel-rs/lintel/compare/lintel-catalog-builder-v0.0.8...lintel-catalog-builder-v0.0.9) - 2026-02-24

### Other

- Merge pull request #90 from lintel-rs/faster-builder
- Add version-based $id resolution and invalid schema detection
- Add x-lintel metadata with source URL and content hash to output schemas
- Unify HTTP concurrency control via semaphore in SchemaCache

## [0.0.8](https://github.com/lintel-rs/lintel/compare/lintel-catalog-builder-v0.0.7...lintel-catalog-builder-v0.0.8) - 2026-02-24

### Other

- Add custom Catalog serializer with $schema field, remove schemastore re-exports
- Merge origin/master and resolve conflicts in generate modules
- Move generate command bpaf args into commands/generate.rs
- Split generate command into separate modules under src/generate/
- Store schemas in versioned directory structure and download all versions

## [0.0.7](https://github.com/lintel-rs/lintel/compare/lintel-catalog-builder-v0.0.6...lintel-catalog-builder-v0.0.7) - 2026-02-23

### Other

- Merge pull request #70 from lintel-rs/fix-sgconfig
- Fix schemas with relative $ref paths (e.g. ast-grep sgconfig.yml)

## [0.0.6](https://github.com/lintel-rs/lintel/compare/lintel-catalog-builder-v0.0.5...lintel-catalog-builder-v0.0.6) - 2026-02-23

### Other

- Add catalog.json schema, rename outputs, and auto-populate entry metadata
- Add rich titles, descriptions, and examples to generated JSON schemas
- Remove re-exports, add lintel-config-schema-generator, simplify flake

## [0.0.5](https://github.com/lintel-rs/lintel/compare/lintel-catalog-builder-v0.0.4...lintel-catalog-builder-v0.0.5) - 2026-02-22

### Added

- auto-generate man pages and shell completions for all CLIs

### Other

- Merge remote-tracking branch 'origin/master' into man-page

## [0.0.4](https://github.com/lintel-rs/lintel/compare/lintel-catalog-builder-v0.0.3...lintel-catalog-builder-v0.0.4) - 2026-02-22

### Added

- add sourceUrl to catalog entries and use kebab-case config keys
- add download logging, $id injection, and cache status visibility

### Other

- reduce too-many-arguments violations across workspace

## [0.0.3](https://github.com/lintel-rs/lintel/compare/lintel-catalog-builder-v0.0.2...lintel-catalog-builder-v0.0.3) - 2026-02-21

### Added

- update all dependencies to latest versions

## [0.0.2](https://github.com/lintel-rs/lintel/compare/lintel-catalog-builder-v0.0.1...lintel-catalog-builder-v0.0.2) - 2026-02-21

### Other

- add SchemaCacheBuilder and centralize TTL defaulting
- Merge remote-tracking branch 'origin/master' into fix-more-stuff

## [0.0.1](https://github.com/lintel-rs/lintel/releases/tag/lintel-catalog-builder-v0.0.1) - 2026-02-21

### Added

- add github config to dir target and move index.html to shared output
- simplify organize entries, add catalog title, and use kebab-case filenames
- add schema-catalog and lintel-catalog-builder crates

### Fixed

- percent-encode invalid characters in $ref URI references
- handle _shared/ filename collisions and apply std_instead_of_alloc lint

### Other

- standardize crate metadata and READMEs
