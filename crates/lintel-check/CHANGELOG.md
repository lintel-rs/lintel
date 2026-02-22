# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.8](https://github.com/lintel-rs/lintel/compare/lintel-check-v0.0.7...lintel-check-v0.0.8) - 2026-02-22

### Added

- add `lintel explain` command and consolidate cache CLI options

### Other

- Merge pull request #43 from lintel-rs/default-catalog-correct
- Deduplicate catalog fetching and enforce precedence order
- Point default catalog at catalog.lintel.tools

## [0.0.7](https://github.com/lintel-rs/lintel/compare/lintel-check-v0.0.6...lintel-check-v0.0.7) - 2026-02-21

### Other

- updated the following local packages: lintel-schema-cache

## [0.0.6](https://github.com/lintel-rs/lintel/compare/lintel-check-v0.0.5...lintel-check-v0.0.6) - 2026-02-21

### Other

- add SchemaCacheBuilder and centralize TTL defaulting
- Merge remote-tracking branch 'origin/master' into fix-more-stuff

## [0.0.5](https://github.com/lintel-rs/lintel/compare/lintel-check-v0.0.4...lintel-check-v0.0.5) - 2026-02-21

### Added

- add `lintel cache` command and migrate file reads to tokio

### Fixed

- correct README code examples to pass doctests
- isolate test schema cache to temp dirs to prevent cache corruption
- point root-level validation errors at content, not modeline comments

### Other

- Merge remote-tracking branch 'origin/master' into claude-skill
- Fix doctest failures in README code examples
- Clean up all crates: fix section ordering, READMEs, and metadata
- Use inline badges in READMEs and add doc includes
- Merge remote-tracking branch 'origin/master' into clippy-lint
- deny std_instead_of_alloc and std_instead_of_core clippy lints

## [0.0.4](https://github.com/lintel-rs/lintel/compare/lintel-check-v0.0.3...lintel-check-v0.0.4) - 2026-02-20

### Other

- Remove $schema comments from all Cargo.toml files
- Merge remote-tracking branch 'origin/master' into lintel-github-action
- Add lintel-reporters crate, lintel-github-action binary, and --reporter flag

## [0.0.3](https://github.com/lintel-rs/lintel/compare/lintel-check-v0.0.2...lintel-check-v0.0.3) - 2026-02-20

### Other

- Merge pull request #11 from lintel-rs/lintel-wt2
- Add per-crate READMEs with badges and inherit workspace package metadata
- Fix cache dir creation with temp_dir fallback and use async tokio::fs for cache I/O
- Optimize validation pipeline with GlobSet, rayon, and schema hash caching
- Add tracing instrumentation and generate benchmark results
- Add validation cache, schema cache TTL, and benchmark tooling

## [0.0.2](https://github.com/lintel-rs/lintel/compare/lintel-check-v0.0.1...lintel-check-v0.0.2) - 2026-02-19

### Other

- Extract lintel-config crate with build-time schema generation
- Fix clippy pedantic warnings and deny unwrap_used
- Add clippy::pedantic lint to workspace
- Skip unrecognized file extensions and extract registry URL resolution

## [0.0.1](https://github.com/lintel-rs/lintel/releases/tag/lintel-check-v0.0.1) - 2026-02-19

### Other

- Bump json5 to 1.3, jsonc-parser to 0.29, and toml to 1.0
- Add LICENSE file and author metadata to all crates
- Change license from MIT to Apache-2.0 across all crates and npm package
- Add markdown frontmatter validation, CLI commands, and release infrastructure
- Initial commit
