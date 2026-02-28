# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.1](https://github.com/lintel-rs/lintel/releases/tag/glob-set-v0.0.1) - 2026-02-26

### Fixed

- fix brace expansion bug

### Other

- Centralize workspace dependencies in root Cargo.toml
- Update glob-matcher and glob-set dev-dependencies
- Add skip_char_class/skip_braces to glob-matcher public API, deduplicate glob-set
- Add upstream globset benchmarks, compatibility tests, and fix dotfile matching
- Replace custom trie with tried double-array trie; add MatchEngine, GlobMap, TinyGlobSet
- Add glob-matcher and glob-set crates; fix cargo-furnish license override
