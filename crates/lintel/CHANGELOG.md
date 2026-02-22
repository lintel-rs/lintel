# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.6](https://github.com/lintel-rs/lintel/compare/lintel-v0.0.5...lintel-v0.0.6) - 2026-02-21

### Other

- update Cargo.lock dependencies

## [0.0.5](https://github.com/lintel-rs/lintel/compare/lintel-v0.0.4...lintel-v0.0.5) - 2026-02-21

### Other

- add SchemaCacheBuilder and centralize TTL defaulting
- Merge remote-tracking branch 'origin/master' into fix-more-stuff

## [0.0.4](https://github.com/lintel-rs/lintel/compare/lintel-v0.0.3...lintel-v0.0.4) - 2026-02-21

### Added

- add lintel-cli-common crate with CLIGlobalOptions

### Fixed

- add missing dependencies to lintel crate for cache command
- apply std_instead_of_core lint to new crates from master merge

### Other

- Clean up all crates: fix section ordering, READMEs, and metadata
- Use inline badges in READMEs and add doc includes
- Merge pull request #26 from lintel-rs/rayon-check
- extract lintel-identify crate from lintel binary
- Merge remote-tracking branch 'origin/master' into clippy-lint
- deny std_instead_of_alloc and std_instead_of_core clippy lints

## [0.0.3](https://github.com/lintel-rs/lintel/compare/lintel-v0.0.2...lintel-v0.0.3) - 2026-02-20

### Other

- Merge pull request #11 from lintel-rs/lintel-wt2
- Add per-crate READMEs with badges and inherit workspace package metadata
- Add tracing instrumentation and generate benchmark results
- Add validation cache, schema cache TTL, and benchmark tooling

## [0.0.2](https://github.com/lintel-rs/lintel/compare/lintel-v0.0.1...lintel-v0.0.2) - 2026-02-19

### Other

- Add logo to README
- Extract lintel-config crate with build-time schema generation
- Fix clippy pedantic warnings and deny unwrap_used
- Add clippy::pedantic lint to workspace

## [0.0.1](https://github.com/lintel-rs/lintel/releases/tag/lintel-v0.0.1) - 2026-02-19

### Other

- Bump json5 to 1.3, jsonc-parser to 0.29, and toml to 1.0
- Add LICENSE file and author metadata to all crates
- Change license from MIT to Apache-2.0 across all crates and npm package
- Add markdown frontmatter validation, CLI commands, and release infrastructure
- Initial commit
