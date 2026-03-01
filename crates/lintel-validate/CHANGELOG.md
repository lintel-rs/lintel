# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.10](https://github.com/lintel-rs/lintel/compare/lintel-validate-v0.0.9...lintel-validate-v0.0.10) - 2026-03-01

### Other

- Merge remote-tracking branch 'origin/master' into fix-lintel-check-unify

## [0.0.9](https://github.com/lintel-rs/lintel/compare/lintel-validate-v0.0.8...lintel-validate-v0.0.9) - 2026-03-01

### Other

- Fix schema compilation failure when $schema URI contains a fragment

## [0.0.8](https://github.com/lintel-rs/lintel/compare/lintel-validate-v0.0.7...lintel-validate-v0.0.8) - 2026-02-28

### Other

- Support relative $ref resolution in local schemas
- Remove FileFormat re-export from parsers, import from schema_catalog directly
- Merge origin/master into jsonl-support
- Merge pull request #116 from lintel-rs/autocomplete-fix
- Add shell file/directory completion to CLI arguments

## [0.0.7](https://github.com/lintel-rs/lintel/compare/lintel-validate-v0.0.6...lintel-validate-v0.0.7) - 2026-02-27

### Other

- Add fileMatch and parsers to x-lintel metadata

## [0.0.6](https://github.com/lintel-rs/lintel/compare/lintel-validate-v0.0.5...lintel-validate-v0.0.6) - 2026-02-26

### Other

- Centralize workspace dependencies in root Cargo.toml
- Remove schemastore crate, inline CATALOG_URL into lintel-validate
- Resolve merge conflicts: move CompiledCatalog to schema-catalog
- Move CompiledCatalog and SchemaMatch from schemastore to schema-catalog

## [0.0.5](https://github.com/lintel-rs/lintel/compare/lintel-validate-v0.0.4...lintel-validate-v0.0.5) - 2026-02-24

### Other

- Parse .json files as JSONC to support comments and trailing commas

## [0.0.4](https://github.com/lintel-rs/lintel/compare/lintel-validate-v0.0.3...lintel-validate-v0.0.4) - 2026-02-24

### Added

- add jsonschema-migrate crate, per-group _shared dirs, and update README badges

### Other

- Add custom Catalog serializer with $schema field, remove schemastore re-exports
- Merge remote-tracking branch 'origin/master' into id-fixes

## [0.0.3](https://github.com/lintel-rs/lintel/compare/lintel-validate-v0.0.2...lintel-validate-v0.0.3) - 2026-02-23

### Other

- Fix schemas with relative $ref paths (e.g. ast-grep sgconfig.yml)

## [0.0.2](https://github.com/lintel-rs/lintel/compare/lintel-validate-v0.0.1...lintel-validate-v0.0.2) - 2026-02-23

### Other

- updated the following local packages: lintel-config, schemastore
