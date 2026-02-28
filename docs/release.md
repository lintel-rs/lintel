# How Releases Work

Lintel uses [release-plz](https://release-plz.dev/) to fully automate versioning, changelogs, and publishing. No manual version bumps or changelog edits are needed.

## Release Flow

```
PR merged to master
  │
  ├─► release-plz-pr.yml ─► Opens/updates a release PR with version bumps + changelogs
  │
  └─► build.yml ─► Builds binaries and Docker images, pushes to Cachix
                      │
                      └─► (on success) release.yml
                            │
                            └─► release-plz release
                                  │
                                  ├─ (not a release PR) ─► Verifies crates can be published. No-op.
                                  │
                                  └─ (release PR) ─► Publishes crates to crates.io + creates GitHub releases
                                                       │
                                                       └─ (lintel CLI released)
                                                            │
                                                            ├─► Upload binary artifacts to GitHub release
                                                            │
                                                            ├─► Publish platform-specific npm packages
                                                            │
                                                            └─► Push pre-built multi-arch Docker images
```

## Step by Step

### 1. Release PR (release-plz-pr.yml)

On every push to `master`, release-plz analyzes commits since the last release and opens (or updates) a pull request that:

- Bumps crate versions in `Cargo.toml` files
- Updates `CHANGELOG.md` entries based on conventional commits

This PR does not publish anything — it just prepares the version bump.

### 2. Build (build.yml)

Also triggered on push to `master`. Builds binaries and Docker images for all supported platforms, uploading everything as workflow artifacts:

**Static binaries (Nix, musl):**

- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`

**Dynamic binaries (Cargo):**

- `x86_64-unknown-linux-gnu` (Debian bullseye for glibc compat)
- `aarch64-unknown-linux-gnu` (Debian bullseye)
- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `x86_64-pc-windows-msvc`

**Docker images (Nix):**

- `amd64`
- `arm64`

Docker images are built via `nix build .#docker` and uploaded as tarballs so that release.yml can push them without rebuilding.

### 3. Publish (release.yml)

Triggered automatically when the Build workflow succeeds on `master` (via `workflow_run`). This is where actual publishing happens.

#### 3a. release-plz release

Runs `release-plz release` on every Build success on `master`. What happens depends on whether the commit is a merged release PR:

- **Not a release PR:** release-plz verifies that changed crates _can_ be published (a dry-run). Nothing is published, no tags are created.
- **Merged release PR:** release-plz publishes updated crates to [crates.io](https://crates.io) and creates GitHub releases with tags. The lintel CLI crate uses `v{version}` tags (e.g., `v0.1.0`) configured in `release-plz.toml`; other crates use the default `{crate-name}-v{version}` format.

#### 3b. Lintel CLI release

The remaining steps only run **if `crates/lintel` was released** (i.e., a lintel release tag was produced). If only library crates were updated, the pipeline stops after crates.io publishing.

**Upload release assets** — Downloads the binary artifacts from the Build workflow run and attaches them to the GitHub release.

**NPM publish** — Downloads release assets from the GitHub release, runs the `npm-release-binaries` tool to generate platform-specific npm packages, and publishes to the npm registry with provenance.

**Docker push** — Downloads the pre-built Docker image tarballs from the Build workflow, loads them, tags with the release version, and pushes to `ghcr.io/lintel-rs/lintel`. Multi-arch manifests are created for `X.Y.Z`, `X.Y`, and `latest`.

## What This Means for Contributors

- **You don't need to bump versions.** release-plz handles it.
- **Write conventional commits** so changelogs are generated correctly.
- **The release PR is automated.** Just review and merge it when ready.
- **Only merging the release PR triggers publishing.** Regular PRs build and test but don't release.
