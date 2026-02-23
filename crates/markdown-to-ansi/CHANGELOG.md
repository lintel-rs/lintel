# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/lintel-rs/lintel/compare/markdown-to-ansi-v0.1.1...markdown-to-ansi-v0.1.2) - 2026-02-23

### Fixed

- render markdown headers properly in descriptions

## [0.1.1](https://github.com/lintel-rs/lintel/compare/markdown-to-ansi-v0.1.0...markdown-to-ansi-v0.1.1) - 2026-02-22

### Added

- extract man.rs, fix NAME/DESCRIPTION, fix terminal width, add validation errors
- extract ANSI escape codes into shared ansi-term-codes crate

### Other

- reduce too-many-arguments violations across workspace

## [0.1.0](https://github.com/lintel-rs/lintel/releases/tag/markdown-to-ansi-v0.1.0) - 2026-02-21

### Other

- Clean up all crates: fix section ordering, READMEs, and metadata
- Use inline badges in READMEs and add doc includes
- Merge remote-tracking branch 'origin/master' into claude-skill
- Add cargo-furnish crate and standardize Cargo.toml metadata
- Add sublime-syntaxes crate with build-time precompiled syntax definitions
- Add markdown-to-ansi crate and refactor jsonschema-explain to use it
