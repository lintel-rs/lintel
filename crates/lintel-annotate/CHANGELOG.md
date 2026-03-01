# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.15](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.14...lintel-annotate-v0.0.15) - 2026-03-01

### Other

- updated the following local packages: lintel-config, schema-catalog, lintel-validate

## [0.0.14](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.13...lintel-annotate-v0.0.14) - 2026-02-28

### Other

- Remove FileFormat re-export from parsers, import from schema_catalog directly
- Merge origin/master into jsonl-support
- Merge pull request #116 from lintel-rs/autocomplete-fix
- Add shell file/directory completion to CLI arguments

## [0.0.13](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.12...lintel-annotate-v0.0.13) - 2026-02-27

### Other

- updated the following local packages: lintel-schema-cache, schema-catalog, lintel-validate

## [0.0.12](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.11...lintel-annotate-v0.0.12) - 2026-02-26

### Other

- Centralize workspace dependencies in root Cargo.toml
- Resolve merge conflicts: move CompiledCatalog to schema-catalog
- Move CompiledCatalog and SchemaMatch from schemastore to schema-catalog

## [0.0.11](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.10...lintel-annotate-v0.0.11) - 2026-02-24

### Other

- updated the following local packages: lintel-schema-cache, lintel-validate

## [0.0.10](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.9...lintel-annotate-v0.0.10) - 2026-02-24

### Added

- add jsonschema-migrate crate, per-group _shared dirs, and update README badges

### Other

- Merge remote-tracking branch 'origin/master' into id-fixes

## [0.0.9](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.8...lintel-annotate-v0.0.9) - 2026-02-23

### Other

- updated the following local packages: lintel-validate, schemastore

## [0.0.8](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.7...lintel-annotate-v0.0.8) - 2026-02-23

### Other

- Remove re-exports, add lintel-config-schema-generator, simplify flake
- Extract lintel-validate crate from lintel-check

## [0.0.7](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.6...lintel-annotate-v0.0.7) - 2026-02-22

### Added

- add `lintel explain` command and consolidate cache CLI options

### Other

- Deduplicate catalog fetching and enforce precedence order

## [0.0.6](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.5...lintel-annotate-v0.0.6) - 2026-02-21

### Other

- updated the following local packages: lintel-check

## [0.0.5](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.4...lintel-annotate-v0.0.5) - 2026-02-21

### Other

- add SchemaCacheBuilder and centralize TTL defaulting
- Merge remote-tracking branch 'origin/master' into fix-more-stuff
- rename lintel-http-cache back to lintel-schema-cache and HttpCache to SchemaCache
- Merge remote-tracking branch 'origin/master' into fix-more-stuff
- rename lintel-schema-cache to lintel-http-cache and simplify API

## [0.0.4](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.3...lintel-annotate-v0.0.4) - 2026-02-21

### Other

- Clean up all crates: fix section ordering, READMEs, and metadata
- Use inline badges in READMEs and add doc includes
- Merge remote-tracking branch 'origin/master' into claude-skill
- Add cargo-furnish crate and standardize Cargo.toml metadata

## [0.0.3](https://github.com/lintel-rs/lintel/compare/lintel-annotate-v0.0.2...lintel-annotate-v0.0.3) - 2026-02-20

### Other

- release

## [0.0.2](https://github.com/lintel-rs/lintel/releases/tag/lintel-annotate-v0.0.2) - 2026-02-20

### Other

- Tighten clippy checks and add rust-cache to CI lint job
- Fix shell completions and annotate cache dir import
- Add lintel annotate subcommand with --update flag
