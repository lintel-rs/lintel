# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
