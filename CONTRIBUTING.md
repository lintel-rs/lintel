# Contributing to Lintel

Welcome! Lintel is a fast, multi-format JSON Schema linter for configuration files. We appreciate your interest in contributing.

For details on how releases work, see [docs/release.md](docs/release.md).

## Getting Started

### Prerequisites

- [Nix](https://nixos.org/download.html) with flakes enabled
- [devenv](https://devenv.sh/)
- [direnv](https://direnv.net/) (recommended)

This project uses [devenv](https://devenv.sh/) to provide a complete, reproducible development environment. You don't need to install Rust, clippy, rustfmt, or any other tooling manually — devenv provides everything, including the correct Rust toolchain, pre-commit hooks, and convenience scripts.

### Setup

```sh
git clone https://github.com/lintel-rs/lintel.git
cd lintel
direnv allow
```

That's it. `direnv allow` activates the devenv shell automatically whenever you enter the project directory. It provides:

- Rust stable toolchain (via Nix)
- Pre-commit hooks (clippy, rustfmt, nixfmt, prettier)
- Convenience scripts: `lintel`, `lintel-debug`, `cargo-furnish`, `npm-release-binaries`
- Cachix binary cache for faster Nix builds

Without direnv, you can enter the shell manually with `devenv shell`.

## Building

### With Cargo

```sh
cargo build
cargo build --release
```

### With Nix

```sh
nix build            # default package
nix build .#all      # all packages
nix build .#lintel-static  # static musl binary (Linux only)
```

## Testing

```sh
cargo test
```

With Nix:

```sh
nix flake check
```

## Code Quality

### Pre-commit Hooks

The devenv shell configures pre-commit hooks that run automatically on each commit:

- **clippy** — lint with all features and all targets, deny warnings
- **rustfmt** — format Rust code
- **nixfmt** — format Nix files
- **prettier** — format Markdown, YAML, JSON, etc.

### Clippy Configuration

The workspace enables `clippy::pedantic` (warn) and `clippy::complexity` (deny), plus several specific lints. Thresholds are set in `clippy.toml`:

| Threshold                        | Value |
| -------------------------------- | ----- |
| `cognitive-complexity-threshold` | 25    |
| `too-many-arguments-threshold`   | 4     |
| `too-many-lines-threshold`       | 100   |
| `type-complexity-threshold`      | 250   |

CI runs clippy with `-D warnings`, so all warnings must be resolved before merging.

## Project Structure

Lintel is a Cargo workspace with crates in `crates/`:

| Group            | Crates                                                                                       | Description                                            |
| ---------------- | -------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| **CLI**          | `lintel`                                                                                     | Main CLI binary                                        |
| **Core**         | `lintel-check`, `lintel-validate`, `lintel-identify`, `lintel-annotate`, `lintel-explain`    | Linting pipeline stages                                |
| **Config**       | `lintel-config`, `lintel-config-schema-generator`                                            | Configuration loading and schema generation            |
| **Schema**       | `lintel-schema-cache`, `lintel-validation-cache`, `schema-catalog`, `lintel-catalog-builder` | Schema fetching, caching, and catalog management       |
| **Output**       | `lintel-reporters`, `lintel-format`                                                          | Result formatting and reporting                        |
| **JSON Schema**  | `jsonschema-explain`, `jsonschema-migrate`                                                   | Human-readable error explanations and schema migration |
| **Utilities**    | `lintel-cli-common`, `glob-matcher`, `glob-set`, `tried`, `dprint-config`, `cargo-furnish`   | Shared CLI helpers, glob matching, and build tooling   |
| **Distribution** | `npm-release-binaries`, `lintel-github-action`, `lintel-benchmark`                           | NPM packaging, GitHub Action, and benchmarks           |

## Submitting Changes

1. Fork the repository and create a branch from `master`.
2. Make your changes, ensuring tests pass and clippy is clean.
3. Push your branch and open a pull request against `master`.
4. CI will run checks, tests, and builds on your PR.

Versioning and changelogs are managed automatically by [release-plz](https://release-plz.dev/). You do not need to bump versions or edit changelogs manually.

## Reporting Bugs

Please open an issue on [GitHub](https://github.com/lintel-rs/lintel/issues) with:

- Steps to reproduce
- Expected vs. actual behavior
- Lintel version (`lintel version`)

## Releases

Releases are fully automated via [release-plz](https://release-plz.dev/) and GitHub Actions. When changes land on `master`, release-plz opens a PR with version bumps and changelog entries. Merging that PR triggers crate publishing, binary uploads, npm packages, and Docker images.

See [docs/release.md](docs/release.md) for the full walkthrough.
