# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.6](https://github.com/lintel-rs/lintel/compare/lintel-config-v0.0.5...lintel-config-v0.0.6) - 2026-02-24

### Added

- add jsonschema-migrate crate, per-group _shared dirs, and update README badges

### Other

- Merge remote-tracking branch 'origin/master' into id-fixes

## [0.0.5](https://github.com/lintel-rs/lintel/compare/lintel-config-v0.0.4...lintel-config-v0.0.5) - 2026-02-23

### Other

- Add rich titles, descriptions, and examples to generated JSON schemas
- Remove re-exports, add lintel-config-schema-generator, simplify flake

## [0.0.4](https://github.com/lintel-rs/lintel/compare/lintel-config-v0.0.3...lintel-config-v0.0.4) - 2026-02-21

### Fixed

- correct README code examples to pass doctests

### Other

- Clean up all crates: fix section ordering, READMEs, and metadata
- Use inline badges in READMEs and add doc includes
- deny std_instead_of_alloc and std_instead_of_core clippy lints

## [0.0.3](https://github.com/lintel-rs/lintel/compare/lintel-config-v0.0.2...lintel-config-v0.0.3) - 2026-02-20

### Other

- Remove $schema comments from all Cargo.toml files
- Merge remote-tracking branch 'origin/master' into lintel-github-action
- Add lintel-reporters crate, lintel-github-action binary, and --reporter flag

## [0.0.2](https://github.com/lintel-rs/lintel/compare/lintel-config-v0.0.1...lintel-config-v0.0.2) - 2026-02-20

### Other

- Merge pull request #11 from lintel-rs/lintel-wt2
- Add per-crate READMEs with badges and inherit workspace package metadata
