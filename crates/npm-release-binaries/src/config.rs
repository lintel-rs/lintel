use alloc::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Known target definition with os/cpu metadata.
pub struct KnownTarget {
    pub os: &'static str,
    pub cpu: &'static str,
    pub libc: Option<&'static str>,
}

pub static KNOWN_TARGETS: &[(&str, KnownTarget)] = &[
    (
        "darwin-arm64",
        KnownTarget {
            os: "darwin",
            cpu: "arm64",
            libc: None,
        },
    ),
    (
        "darwin-x64",
        KnownTarget {
            os: "darwin",
            cpu: "x64",
            libc: None,
        },
    ),
    (
        "linux-arm64",
        KnownTarget {
            os: "linux",
            cpu: "arm64",
            libc: Some("glibc"),
        },
    ),
    (
        "linux-arm64-musl",
        KnownTarget {
            os: "linux",
            cpu: "arm64",
            libc: Some("musl"),
        },
    ),
    (
        "linux-x64",
        KnownTarget {
            os: "linux",
            cpu: "x64",
            libc: Some("glibc"),
        },
    ),
    (
        "linux-x64-musl",
        KnownTarget {
            os: "linux",
            cpu: "x64",
            libc: Some("musl"),
        },
    ),
    (
        "win32-x64",
        KnownTarget {
            os: "win32",
            cpu: "x64",
            libc: None,
        },
    ),
];

fn lookup_known_target(key: &str) -> Option<&'static KnownTarget> {
    KNOWN_TARGETS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub output_dir: Option<PathBuf>,
    pub artifacts_dir: Option<PathBuf>,
    pub packages: BTreeMap<String, PackageConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PackageConfig {
    pub name: String,
    #[serde(rename = "crate")]
    pub crate_name: Option<String>,
    pub bin: Option<String>,
    pub description: Option<String>,
    pub archive_base_url: Option<String>,
    pub target_package_name: String,
    pub targets: BTreeMap<String, String>,
    pub readme: Option<PathBuf>,
    pub access: Option<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub keywords: Option<Vec<String>>,
}

/// Fully resolved target with package name, archive source, and binary name.
#[derive(Debug)]
pub struct ResolvedTarget {
    pub key: String,
    pub os: String,
    pub cpu: String,
    pub libc: Option<String>,
    pub package_name: String,
    /// Full local path or URL to the archive.
    pub archive: String,
    pub binary_name: String,
}

impl PackageConfig {
    pub fn crate_name<'a>(&'a self, pkg_key: &'a str) -> &'a str {
        self.crate_name.as_deref().unwrap_or(pkg_key)
    }

    pub fn bin_name<'a>(&'a self, pkg_key: &'a str) -> &'a str {
        self.bin
            .as_deref()
            .unwrap_or_else(|| self.crate_name(pkg_key))
    }
}

/// Resolve configured targets into fully expanded entries.
///
/// Resolution order for each target's archive:
/// 1. `--artifacts-dir` provided → `{artifacts_dir}/{archive_name}` (local file)
/// 2. Archive starting with `http(s)://` → URL directly
/// 3. `archive-base-url` set → `{base_url}/{archive_name}` (URL)
/// 4. Error
pub fn resolve_targets(
    pkg_key: &str,
    pkg_config: &PackageConfig,
    version: &str,
    artifacts_dir: Option<&Path>,
) -> miette::Result<Vec<ResolvedTarget>> {
    let bin = pkg_config.bin_name(pkg_key);
    let mut resolved = Vec::new();

    for (target_key, archive_name) in &pkg_config.targets {
        let known = lookup_known_target(target_key).ok_or_else(|| {
            miette::miette!(
                "unknown target '{target_key}'; known targets: {}",
                KNOWN_TARGETS
                    .iter()
                    .map(|(k, _)| *k)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        let package_name = pkg_config
            .target_package_name
            .replace("{{target}}", target_key);

        let archive_name = archive_name.replace("{{version}}", version);

        let archive = if let Some(dir) = artifacts_dir {
            dir.join(&archive_name).to_string_lossy().to_string()
        } else if archive_name.starts_with("http://") || archive_name.starts_with("https://") {
            archive_name.clone()
        } else if let Some(ref base_url) = pkg_config.archive_base_url {
            let base = base_url.replace("{{version}}", version);
            format!("{base}/{archive_name}")
        } else {
            return Err(miette::miette!(
                "target '{target_key}': no --artifacts-dir and no archive-base-url; \
                 cannot resolve archive '{archive_name}'"
            ));
        };

        let binary_name = if known.os == "win32" {
            format!("{bin}.exe")
        } else {
            bin.to_string()
        };

        resolved.push(ResolvedTarget {
            key: target_key.clone(),
            os: known.os.to_string(),
            cpu: known.cpu.to_string(),
            libc: known.libc.map(String::from),
            package_name,
            archive,
            binary_name,
        });
    }

    Ok(resolved)
}

pub fn load(path: &Path) -> miette::Result<Config> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| miette::miette!("failed to read config {}: {e}", path.display()))?;
    let config: Config = toml::from_str(&content)
        .map_err(|e| miette::miette!("failed to parse config {}: {e}", path.display()))?;
    Ok(config)
}
