# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1](https://github.com/lintel-rs/lintel/compare/jsonschema-schema-v0.2.0...jsonschema-schema-v0.2.1) - 2026-03-06

### Other

- release

## [0.2.0](https://github.com/lintel-rs/lintel/compare/jsonschema-schema-v0.1.0...jsonschema-schema-v0.2.0) - 2026-03-02

### Added

- unify composition rendering and add schema fragment navigation
- add Schema::absolute() and improve allOf flattening
- add Schema::validate() for structural $ref validation
- flatten allOf into root schema and show provenance in INCLUDES section

### Fixed

- allow too_many_lines on Schema Add impl after rebase

### Other

- remove INCLUDES section, keep allOf with $ref-backed entries
- Merge pull request #148 from lintel-rs/more-schema-ext
- Use include_str! for test fixtures and replace unwrap with expect
- Add typed IntelliJ extensions to jsonschema-schema
- Refactor jsonschema-schema extensions into modules and use BTreeMap
- release

## [0.1.0](https://github.com/lintel-rs/lintel/releases/tag/jsonschema-schema-v0.1.0) - 2026-03-01

### Other

- Move schema resolution from lintel-identify into lintel-explain
