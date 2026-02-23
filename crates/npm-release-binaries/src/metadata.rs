use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::PackageConfig;

/// Resolved metadata for npm package generation.
#[derive(Debug)]
pub struct ResolvedMetadata {
    pub name: String,
    pub bin: String,
    pub description: String,
    pub license: String,
    pub repository: String,
    pub homepage: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoToml {
    package: Option<CratePackage>,
    workspace: Option<Workspace>,
}

#[derive(Debug, Deserialize)]
struct Workspace {
    package: Option<WorkspacePackage>,
    members: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct WorkspacePackage {
    repository: Option<String>,
    license: Option<String>,
    homepage: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CratePackage {
    description: Option<String>,
    keywords: Option<Vec<String>>,
    repository: Option<TomlOrWorkspace>,
    license: Option<TomlOrWorkspace>,
    homepage: Option<TomlOrWorkspace>,
}

/// Handles fields that can be either a direct string or `{ workspace = true }`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TomlOrWorkspace {
    Value(String),
    Workspace {},
}

/// Find the workspace root by walking up from `start` looking for a Cargo.toml
/// with a `[workspace]` table.
fn find_workspace_root(start: &Path) -> miette::Result<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            let content = std::fs::read_to_string(&candidate)
                .map_err(|e| miette::miette!("failed to read {}: {e}", candidate.display()))?;
            let doc: CargoToml = toml::from_str(&content)
                .map_err(|e| miette::miette!("failed to parse {}: {e}", candidate.display()))?;
            if doc.workspace.is_some() {
                return Ok(dir);
            }
        }
        if !dir.pop() {
            return Err(miette::miette!("could not find workspace root"));
        }
    }
}

/// Find the crate directory by scanning workspace members for a matching package name.
fn find_crate_dir(workspace_root: &Path, crate_name: &str) -> miette::Result<PathBuf> {
    let ws_toml_path = workspace_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&ws_toml_path)
        .map_err(|e| miette::miette!("failed to read {}: {e}", ws_toml_path.display()))?;
    let ws_doc: CargoToml = toml::from_str(&content)
        .map_err(|e| miette::miette!("failed to parse {}: {e}", ws_toml_path.display()))?;

    let members = ws_doc
        .workspace
        .as_ref()
        .and_then(|w| w.members.as_ref())
        .ok_or_else(|| miette::miette!("no workspace members found"))?;

    for pattern in members {
        let glob_pattern = workspace_root.join(pattern).to_string_lossy().to_string();
        let paths = glob::glob(&glob_pattern).map_err(|e| miette::miette!("bad glob: {e}"))?;
        for entry in paths {
            let dir: PathBuf = entry.map_err(|e| miette::miette!("glob error: {e}"))?;
            let cargo_toml = dir.join("Cargo.toml");
            if !cargo_toml.exists() {
                continue;
            }
            let ct_content = std::fs::read_to_string(&cargo_toml)
                .map_err(|e| miette::miette!("failed to read {}: {e}", cargo_toml.display()))?;
            let raw: toml::Value = toml::from_str(&ct_content)
                .map_err(|e| miette::miette!("failed to parse {}: {e}", cargo_toml.display()))?;
            if raw
                .get("package")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                == Some(crate_name)
            {
                return Ok(dir);
            }
        }
    }

    Err(miette::miette!(
        "crate '{crate_name}' not found in workspace"
    ))
}

/// Resolve metadata from Cargo.toml workspace + crate, overridden by config.
pub fn resolve(pkg_key: &str, pkg_config: &PackageConfig) -> miette::Result<ResolvedMetadata> {
    let crate_name = pkg_config.crate_name(pkg_key);
    let bin = pkg_config.bin_name(pkg_key).to_string();

    let cwd = std::env::current_dir().map_err(|e| miette::miette!("failed to get cwd: {e}"))?;
    let workspace_root = find_workspace_root(&cwd)?;

    // Read workspace-level metadata
    let ws_toml_path = workspace_root.join("Cargo.toml");
    let ws_content = std::fs::read_to_string(&ws_toml_path)
        .map_err(|e| miette::miette!("failed to read {}: {e}", ws_toml_path.display()))?;
    let ws_doc: CargoToml = toml::from_str(&ws_content)
        .map_err(|e| miette::miette!("failed to parse {}: {e}", ws_toml_path.display()))?;
    let ws_pkg = ws_doc.workspace.as_ref().and_then(|w| w.package.as_ref());

    // Start with workspace defaults
    let mut repository = ws_pkg
        .and_then(|p| p.repository.clone())
        .unwrap_or_default();
    let mut license = ws_pkg.and_then(|p| p.license.clone()).unwrap_or_default();
    let mut homepage = ws_pkg.and_then(|p| p.homepage.clone()).unwrap_or_default();
    let mut description = String::new();
    let mut keywords = Vec::new();

    // Read crate-level metadata (overrides workspace for non-inherited fields)
    let crate_dir = find_crate_dir(&workspace_root, crate_name)?;
    let crate_toml_path = crate_dir.join("Cargo.toml");
    let crate_content = std::fs::read_to_string(&crate_toml_path)
        .map_err(|e| miette::miette!("failed to read {}: {e}", crate_toml_path.display()))?;
    let crate_doc: CargoToml = toml::from_str(&crate_content)
        .map_err(|e| miette::miette!("failed to parse {}: {e}", crate_toml_path.display()))?;

    if let Some(pkg) = &crate_doc.package {
        if let Some(desc) = &pkg.description {
            description.clone_from(desc);
        }
        if let Some(kw) = &pkg.keywords {
            keywords.clone_from(kw);
        }
        // Only override workspace values if the crate specifies a direct value (not workspace ref)
        if let Some(TomlOrWorkspace::Value(v)) = &pkg.repository {
            repository.clone_from(v);
        }
        if let Some(TomlOrWorkspace::Value(v)) = &pkg.license {
            license.clone_from(v);
        }
        if let Some(TomlOrWorkspace::Value(v)) = &pkg.homepage {
            homepage.clone_from(v);
        }
    }

    // Config-level overrides (highest priority)
    if let Some(desc) = &pkg_config.description {
        description.clone_from(desc);
    }
    if let Some(lic) = &pkg_config.license {
        license.clone_from(lic);
    }
    if let Some(repo) = &pkg_config.repository {
        repository.clone_from(repo);
    }
    if let Some(hp) = &pkg_config.homepage {
        homepage.clone_from(hp);
    }
    if let Some(kw) = &pkg_config.keywords {
        keywords.clone_from(kw);
    }

    Ok(ResolvedMetadata {
        name: pkg_config.name.clone(),
        bin,
        description,
        license,
        repository: normalize_git_url(&repository),
        homepage,
        keywords,
    })
}

/// Normalize a repository URL to the format npm expects (`git+https://...git`).
fn normalize_git_url(url: &str) -> String {
    let mut url = url.to_string();
    if !url.starts_with("git+") {
        url = format!("git+{url}");
    }
    if !std::path::Path::new(&url)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("git"))
    {
        url.push_str(".git");
    }
    url
}
