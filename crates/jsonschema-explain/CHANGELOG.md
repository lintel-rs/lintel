# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1](https://github.com/lintel-rs/lintel/compare/jsonschema-explain-v0.2.0...jsonschema-explain-v0.2.1) - 2026-02-23

### Other

- Add examples rendering and use compact array type syntax in explain

## [0.2.0](https://github.com/lintel-rs/lintel/compare/jsonschema-explain-v0.1.0...jsonschema-explain-v0.2.0) - 2026-02-22

### Added

- extract man.rs, fix NAME/DESCRIPTION, fix terminal width, add validation errors
- add `lintel explain` command and consolidate cache CLI options
- extract ANSI escape codes into shared ansi-term-codes crate

### Other

- reduce too-many-arguments violations across workspace

## [0.1.0](https://github.com/lintel-rs/lintel/releases/tag/jsonschema-explain-v0.1.0) - 2026-02-21

### Other

- Fix doctest failures in README code examples
- Clean up all crates: fix section ordering, READMEs, and metadata
- Use inline badges in READMEs and add doc includes
- Add cargo-furnish crate and standardize Cargo.toml metadata
- Add markdown-to-ansi crate and refactor jsonschema-explain to use it
- Add lintel identify command with schema-to-docs renderer
