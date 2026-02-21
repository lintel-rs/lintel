use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use anyhow::{Context, Result};

/// Information extracted from the workspace root `Cargo.toml`.
pub struct WorkspaceInfo {
    /// Keys present in `[workspace.package]` (e.g. "edition", "license").
    pub package_fields: HashSet<String>,
    /// The `repository` value from `[workspace.package]`, if any.
    pub repository: Option<String>,
    /// The `license` value from `[workspace.package]`, if any.
    pub license: Option<String>,
    /// Whether `[workspace.lints]` (or sub-tables like `[workspace.lints.clippy]`) exists.
    pub has_workspace_lints: bool,
}

/// Parse the workspace root `Cargo.toml` at the given path.
pub fn parse_workspace(workspace_root: &Path) -> Result<WorkspaceInfo> {
    let cargo_toml_path = workspace_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml_path)
        .with_context(|| format!("failed to read {}", cargo_toml_path.display()))?;
    let doc = content
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("failed to parse {}", cargo_toml_path.display()))?;

    let workspace = doc
        .get("workspace")
        .and_then(|w| w.as_table())
        .context("root Cargo.toml has no [workspace] table")?;

    let ws_package = workspace
        .get("package")
        .and_then(|p| p.as_table())
        .context("[workspace.package] is missing from workspace Cargo.toml")?;

    let package_fields: HashSet<String> = ws_package.iter().map(|(k, _)| k.to_string()).collect();

    let repository = ws_package
        .get("repository")
        .and_then(|v| v.as_str())
        .map(String::from);

    let license = ws_package
        .get("license")
        .and_then(|v| v.as_str())
        .map(String::from);

    let has_workspace_lints = workspace.get("lints").is_some();

    Ok(WorkspaceInfo {
        package_fields,
        repository,
        license,
        has_workspace_lints,
    })
}

/// Resolve workspace member directories by expanding glob patterns from `[workspace] members`.
pub fn resolve_member_dirs(workspace_root: &Path) -> Result<Vec<PathBuf>> {
    let cargo_toml_path = workspace_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml_path)
        .with_context(|| format!("failed to read {}", cargo_toml_path.display()))?;
    let doc = content
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("failed to parse {}", cargo_toml_path.display()))?;

    let members = doc
        .get("workspace")
        .and_then(|w| w.as_table())
        .and_then(|t| t.get("members"))
        .and_then(|m| m.as_array())
        .context("[workspace] members not found")?;

    let mut dirs = Vec::new();
    for member in members {
        let pattern = member
            .as_str()
            .context("workspace member is not a string")?;
        let full_pattern = workspace_root.join(pattern);
        let pattern_str = full_pattern.display().to_string();

        // Expand glob
        let matches = glob::glob(&pattern_str)
            .with_context(|| format!("invalid glob pattern: {pattern_str}"))?;

        for entry in matches {
            let path = entry.with_context(|| format!("glob error for {pattern_str}"))?;
            if path.join("Cargo.toml").exists() {
                dirs.push(path);
            }
        }
    }

    dirs.sort();
    Ok(dirs)
}

/// Find a workspace member directory by crate name.
pub fn find_member_by_name(workspace_root: &Path, name: &str) -> Result<Option<PathBuf>> {
    let members = resolve_member_dirs(workspace_root)?;
    for dir in members {
        let cargo_toml = dir.join("Cargo.toml");
        if let Ok(content) = std::fs::read_to_string(&cargo_toml)
            && let Ok(doc) = content.parse::<toml_edit::DocumentMut>()
            && let Some(pkg_name) = doc
                .get("package")
                .and_then(|p| p.as_table())
                .and_then(|t| t.get("name"))
                .and_then(|n| n.as_str())
            && pkg_name == name
        {
            return Ok(Some(dir));
        }
    }
    Ok(None)
}
