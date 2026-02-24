# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.5](https://github.com/lintel-rs/lintel/compare/schema-catalog-v0.0.4...schema-catalog-v0.0.5) - 2026-02-24

### Added

- add jsonschema-migrate crate, per-group _shared dirs, and update README badges

### Other

- Add custom Catalog serializer with $schema field, remove schemastore re-exports
- Merge remote-tracking branch 'origin/master' into id-fixes

## [0.0.4](https://github.com/lintel-rs/lintel/compare/schema-catalog-v0.0.3...schema-catalog-v0.0.4) - 2026-02-23

### Other

- Make schema-catalog no_std and replace futures with futures-util

## [0.0.3](https://github.com/lintel-rs/lintel/compare/schema-catalog-v0.0.2...schema-catalog-v0.0.3) - 2026-02-23

### Other

- Add catalog.json schema, rename outputs, and auto-populate entry metadata

## [0.0.2](https://github.com/lintel-rs/lintel/compare/schema-catalog-v0.0.1...schema-catalog-v0.0.2) - 2026-02-22

### Added

- add sourceUrl to catalog entries and use kebab-case config keys

## [0.0.1](https://github.com/lintel-rs/lintel/releases/tag/schema-catalog-v0.0.1) - 2026-02-21

### Added

- simplify organize entries, add catalog title, and use kebab-case filenames
- add schema-catalog and lintel-catalog-builder crates

### Fixed

- handle _shared/ filename collisions and apply std_instead_of_alloc lint

### Other

- standardize crate metadata and READMEs
