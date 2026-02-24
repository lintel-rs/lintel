# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.3](https://github.com/lintel-rs/lintel/compare/npm-release-binaries-v0.0.2...npm-release-binaries-v0.0.3) - 2026-02-24

### Other

- Merge pull request #81 from lintel-rs/dependabot/cargo/zip-8.1.0
- Merge pull request #75 from lintel-rs/id-fixes
- Merge pull request #77 from lintel-rs/lintel-action-issue
- Add musl targets, build aarch64-linux-gnu with nix, and simplify npm-release-binaries config

## [0.0.2](https://github.com/lintel-rs/lintel/compare/npm-release-binaries-v0.0.1...npm-release-binaries-v0.0.2) - 2026-02-23

### Other

- Fix npm publish retry and action release tag naming
- release

## [0.0.1](https://github.com/lintel-rs/lintel/releases/tag/npm-release-binaries-v0.0.1) - 2026-02-23

### Added

- add npm provenance support to release workflow

### Other

- Fix lintel-config-schema-generator packaging and clean up crate metadata
- Refactor generate/release args into shared Options struct to fix clippy too-many-arguments
- Add crate metadata, README, and switch npm scope to @lintel
- Replace npm/ with npm-release-binaries crate for platform-specific npm publishing
