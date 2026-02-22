# npm-release-binaries

[![Crates.io](https://img.shields.io/crates/v/npm-release-binaries.svg)](https://crates.io/crates/npm-release-binaries)
[![docs.rs](https://docs.rs/npm-release-binaries/badge.svg)](https://docs.rs/npm-release-binaries)
[![CI](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml/badge.svg)](https://github.com/lintel-rs/lintel/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/npm-release-binaries.svg)](https://github.com/lintel-rs/lintel/blob/master/LICENSE)

Generate and publish platform-specific npm packages from Rust binaries

## Usage

Configure your packages in `npm-release-binaries.toml`:

```toml
[packages.my-cli]
name = "@scope/my-cli"
description = "My CLI tool"
archive-base-url = "https://github.com/org/repo/releases/download/v{{version}}"
target-package-name = "@scope/cli-{{target}}"
access = "public"

[packages.my-cli.targets]
darwin-arm64 = true
darwin-x64 = true
linux-arm64 = true
linux-x64 = true
win32-x64 = true
```

## Artifact naming convention

When using `artifacts-dir` (e.g. in CI), the tool expects archive files named
`{bin}-{rust_triple}.{ext}`, where `bin` defaults to the package key. For
a package with `bin = "my-cli"`, the expected files are:

| Target         | Expected archive file                     |
| -------------- | ----------------------------------------- |
| `darwin-arm64` | `my-cli-aarch64-apple-darwin.tar.gz`      |
| `darwin-x64`   | `my-cli-x86_64-apple-darwin.tar.gz`       |
| `linux-arm64`  | `my-cli-aarch64-unknown-linux-gnu.tar.gz` |
| `linux-x64`    | `my-cli-x86_64-unknown-linux-gnu.tar.gz`  |
| `win32-x64`    | `my-cli-x86_64-pc-windows-msvc.zip`       |

Each archive must contain the binary at any path — the tool matches by filename
(`my-cli` or `my-cli.exe` for Windows).

You can override the archive name for a specific target:

```toml
[packages.my-cli.targets]
darwin-arm64 = { archive = "custom-name.tar.gz" }
```

When `archive-base-url` is set and no `artifacts-dir` is provided (local
testing), archives are fetched from `{archive-base-url}/{archive-name}`.

Generate npm packages:

```sh
npm-release-binaries generate --package my-cli --release-version 1.0.0
```

Publish to npm:

```sh
npm-release-binaries release --package my-cli --release-version 1.0.0
```

## License

Apache-2.0
