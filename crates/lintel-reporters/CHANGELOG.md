# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.15](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.14...lintel-reporters-v0.0.15) - 2026-03-01

### Other

- Merge remote-tracking branch 'origin/master' into fix-lintel-check-unify
- Remove unused lintel-validate dependency from lintel-reporters
- Unify Reporter, CheckResult, and file reading across check pipeline

## [0.0.14](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.13...lintel-reporters-v0.0.14) - 2026-03-01

### Other

- updated the following local packages: lintel-validate

## [0.0.13](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.12...lintel-reporters-v0.0.13) - 2026-02-28

### Other

- Merge origin/master into jsonl-support
- Add JSONL/NDJSON support across the lintel pipeline

## [0.0.12](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.11...lintel-reporters-v0.0.12) - 2026-02-27

### Other

- updated the following local packages: lintel-validate

## [0.0.11](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.10...lintel-reporters-v0.0.11) - 2026-02-26

### Other

- Centralize workspace dependencies in root Cargo.toml
- Enable Cargo.toml sorting via dprint and remove ordering from cargo-furnish

## [0.0.10](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.9...lintel-reporters-v0.0.10) - 2026-02-24

### Other

- updated the following local packages: lintel-validate

## [0.0.9](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.8...lintel-reporters-v0.0.9) - 2026-02-24

### Added

- add jsonschema-migrate crate, per-group _shared dirs, and update README badges

### Other

- Merge remote-tracking branch 'origin/master' into id-fixes

## [0.0.8](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.7...lintel-reporters-v0.0.8) - 2026-02-23

### Other

- updated the following local packages: lintel-validate

## [0.0.7](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.6...lintel-reporters-v0.0.7) - 2026-02-23

### Other

- Remove re-exports, add lintel-config-schema-generator, simplify flake
- Extract lintel-validate crate from lintel-check

## [0.0.6](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.5...lintel-reporters-v0.0.6) - 2026-02-22

### Added

- add `lintel explain` command and consolidate cache CLI options

### Other

- Merge origin/master into ansi branch

## [0.0.5](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.4...lintel-reporters-v0.0.5) - 2026-02-21

### Other

- updated the following local packages: lintel-check

## [0.0.4](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.3...lintel-reporters-v0.0.4) - 2026-02-21

### Other

- add SchemaCacheBuilder and centralize TTL defaulting
- Merge remote-tracking branch 'origin/master' into fix-more-stuff

## [0.0.3](https://github.com/lintel-rs/lintel/compare/lintel-reporters-v0.0.2...lintel-reporters-v0.0.3) - 2026-02-21

### Added

- add lintel-cli-common crate with CLIGlobalOptions

### Fixed

- correct README code examples to pass doctests

### Other

- Clean up all crates: fix section ordering, READMEs, and metadata
- Use inline badges in READMEs and add doc includes
- deny std_instead_of_alloc and std_instead_of_core clippy lints

## [0.0.2](https://github.com/lintel-rs/lintel/releases/tag/lintel-reporters-v0.0.2) - 2026-02-20

### Other

- Remove $schema comments from all Cargo.toml files
- Merge remote-tracking branch 'origin/master' into lintel-github-action
- Add lintel-reporters crate, lintel-github-action binary, and --reporter flag
