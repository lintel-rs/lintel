# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.15](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.14...lintel-github-action-v0.0.15) - 2026-03-01

### Other

- Merge remote-tracking branch 'origin/master' into fix-lintel-check-unify

## [0.0.14](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.13...lintel-github-action-v0.0.14) - 2026-03-01

### Other

- updated the following local packages: lintel-validate, lintel-format, lintel-check

## [0.0.13](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.12...lintel-github-action-v0.0.13) - 2026-02-28

### Other

- Merge origin/master into jsonl-support
- Add JSONL/NDJSON support across the lintel pipeline

## [0.0.12](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.11...lintel-github-action-v0.0.12) - 2026-02-27

### Other

- updated the following local packages: lintel-validate, lintel-check

## [0.0.11](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.10...lintel-github-action-v0.0.11) - 2026-02-26

### Other

- Centralize workspace dependencies in root Cargo.toml
- Convert lintel-github-action from standalone binary to library subcommand
- Enable Cargo.toml sorting via dprint and remove ordering from cargo-furnish

## [0.0.10](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.9...lintel-github-action-v0.0.10) - 2026-02-24

### Other

- Fix static musl builds and add smoke tests
- Fix action version references from v1 to v0
- Rewrite lintel-github-action README with quick start, examples, and action inputs

## [0.0.9](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.8...lintel-github-action-v0.0.9) - 2026-02-24

### Added

- add jsonschema-migrate crate, per-group _shared dirs, and update README badges

### Other

- Merge remote-tracking branch 'origin/master' into id-fixes

## [0.0.8](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.7...lintel-github-action-v0.0.8) - 2026-02-23

### Other

- updated the following local packages: lintel-validate

## [0.0.7](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.6...lintel-github-action-v0.0.7) - 2026-02-23

### Other

- Remove re-exports, add lintel-config-schema-generator, simplify flake
- Extract lintel-validate crate from lintel-check

## [0.0.6](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.5...lintel-github-action-v0.0.6) - 2026-02-22

### Other

- reduce too-many-arguments violations across workspace

## [0.0.5](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.4...lintel-github-action-v0.0.5) - 2026-02-21

### Added

- update all dependencies to latest versions

## [0.0.4](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.3...lintel-github-action-v0.0.4) - 2026-02-21

### Other

- Merge remote-tracking branch 'origin/master' into fix-more-stuff

## [0.0.3](https://github.com/lintel-rs/lintel/compare/lintel-github-action-v0.0.2...lintel-github-action-v0.0.3) - 2026-02-21

### Added

- add lintel-cli-common crate with CLIGlobalOptions

### Other

- Clean up all crates: fix section ordering, READMEs, and metadata
- Use inline badges in READMEs and add doc includes
- deny std_instead_of_alloc and std_instead_of_core clippy lints

## [0.0.2](https://github.com/lintel-rs/lintel/releases/tag/lintel-github-action-v0.0.2) - 2026-02-20

### Other

- Remove $schema comments from all Cargo.toml files
- Merge remote-tracking branch 'origin/master' into lintel-github-action
- Add lintel-reporters crate, lintel-github-action binary, and --reporter flag
