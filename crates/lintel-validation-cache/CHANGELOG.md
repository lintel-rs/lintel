# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.6](https://github.com/lintel-rs/lintel/compare/lintel-validation-cache-v0.0.5...lintel-validation-cache-v0.0.6) - 2026-02-22

### Other

- reduce too-many-arguments violations across workspace

## [0.0.5](https://github.com/lintel-rs/lintel/compare/lintel-validation-cache-v0.0.4...lintel-validation-cache-v0.0.5) - 2026-02-21

### Other

- Merge remote-tracking branch 'origin/master' into fix-more-stuff

## [0.0.4](https://github.com/lintel-rs/lintel/compare/lintel-validation-cache-v0.0.3...lintel-validation-cache-v0.0.4) - 2026-02-21

### Added

- add `lintel cache` command and migrate file reads to tokio

### Fixed

- correct README code examples to pass doctests

### Other

- Clean up all crates: fix section ordering, READMEs, and metadata
- Use inline badges in READMEs and add doc includes

## [0.0.3](https://github.com/lintel-rs/lintel/compare/lintel-validation-cache-v0.0.2...lintel-validation-cache-v0.0.3) - 2026-02-20

### Other

- Remove $schema comments from all Cargo.toml files

## [0.0.2](https://github.com/lintel-rs/lintel/compare/lintel-validation-cache-v0.0.1...lintel-validation-cache-v0.0.2) - 2026-02-20

### Other

- Merge pull request #11 from lintel-rs/lintel-wt2
- Add per-crate READMEs with badges and inherit workspace package metadata
- Fix cache dir creation with temp_dir fallback and use async tokio::fs for cache I/O
