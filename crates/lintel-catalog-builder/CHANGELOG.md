# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.1](https://github.com/lintel-rs/lintel/releases/tag/lintel-catalog-builder-v0.0.1) - 2026-02-21

### Added

- improve validation error messages with schema path context
- add github config to dir target and move index.html to shared output
- simplify organize entries, add catalog title, and use kebab-case filenames
- add schema-catalog and lintel-catalog-builder crates

### Fixed

- percent-encode invalid characters in $ref URI references
- handle _shared/ filename collisions and apply std_instead_of_alloc lint

### Other

- Merge remote-tracking branch 'origin/master' into error-check
