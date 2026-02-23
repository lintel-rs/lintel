# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
