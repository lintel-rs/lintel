use core::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use schema_catalog::Catalog;
use tracing::debug;

use crate::catalog::write_catalog_json;
use crate::download::ProcessedSchemas;
use lintel_catalog_builder::config::TargetConfig;

/// Data passed to [`finalize`] for writing output files.
pub struct OutputContext<'a> {
    pub output_dir: &'a Path,
    pub config_path: &'a Path,
    pub config_dir: &'a Path,
    pub catalog: &'a Catalog,
    pub groups_meta: &'a [(String, String, String)],
    pub base_url: &'a str,
    pub source_count: usize,
    /// All processed schema values for in-memory lookups during HTML generation.
    pub processed: &'a ProcessedSchemas,
    /// Optional site description from the target's site config.
    pub site_description: Option<&'a str>,
    /// Optional Google Analytics tracking ID from the target's site config.
    pub ga_tracking_id: Option<&'a str>,
    /// Optional Open Graph image URL from the target's site config.
    pub og_image: Option<&'a str>,
}

/// Resolve the output directory for a target.
pub fn output_dir(target: &TargetConfig, config_dir: &Path) -> PathBuf {
    let path = PathBuf::from(&target.dir);
    if path.is_absolute() {
        path
    } else {
        config_dir.join(path)
    }
}

/// Write all output files for a target.
pub async fn finalize(target: &TargetConfig, ctx: &OutputContext<'_>) -> Result<()> {
    write_common_files(ctx).await?;

    if let Some(gh) = target.site.as_ref().and_then(|s| s.github.as_ref()) {
        tokio::fs::write(ctx.output_dir.join(".nojekyll"), "")
            .await
            .context("failed to write .nojekyll")?;

        if let Some(domain) = &gh.cname {
            tokio::fs::write(ctx.output_dir.join("CNAME"), format!("{domain}\n"))
                .await
                .context("failed to write CNAME")?;
            debug!(domain, "wrote CNAME");
        }

        debug!("wrote .nojekyll");
    }

    // Copy static/ directory contents last so they can override generated files.
    copy_static_dir(ctx.config_dir, ctx.output_dir).await?;

    Ok(())
}

/// Copy files from a `static/` directory (relative to the config file) into the output root.
async fn copy_static_dir(config_dir: &Path, output_dir: &Path) -> Result<()> {
    let static_dir = config_dir.join("static");
    if !static_dir.is_dir() {
        return Ok(());
    }
    debug!(path = %static_dir.display(), "copying static directory");
    copy_dir_recursive(&static_dir, output_dir).await
}

/// Recursively copy all files from `src` into `dst`, creating subdirectories as needed.
fn copy_dir_recursive<'a>(
    src: &'a Path,
    dst: &'a Path,
) -> futures_util::future::BoxFuture<'a, Result<()>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(src)
            .await
            .with_context(|| format!("failed to read directory {}", src.display()))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .with_context(|| format!("failed to read entry in {}", src.display()))?
        {
            let file_type = entry.file_type().await.with_context(|| {
                format!("failed to get file type for {}", entry.path().display())
            })?;
            let dest_path = dst.join(entry.file_name());
            if file_type.is_dir() {
                tokio::fs::create_dir_all(&dest_path)
                    .await
                    .with_context(|| {
                        format!("failed to create directory {}", dest_path.display())
                    })?;
                copy_dir_recursive(&entry.path(), &dest_path).await?;
            } else {
                tokio::fs::copy(entry.path(), &dest_path)
                    .await
                    .with_context(|| {
                        format!(
                            "failed to copy {} to {}",
                            entry.path().display(),
                            dest_path.display()
                        )
                    })?;
                debug!(file = %dest_path.display(), "copied static file");
            }
        }
        Ok(())
    })
}

/// Write `catalog.json`, `README.md`, and the full static site — shared by all targets.
pub async fn write_common_files(ctx: &OutputContext<'_>) -> Result<()> {
    write_catalog_json(ctx.output_dir, ctx.catalog).await?;
    write_readme(ctx).await?;
    crate::html::generate_site(ctx).await?;
    Ok(())
}

/// Generate a `README.md` in the output directory.
async fn write_readme(ctx: &OutputContext<'_>) -> Result<()> {
    let config_dir = ctx.config_path.parent().unwrap_or_else(|| Path::new("."));
    let source_repo = detect_git_remote(config_dir).await;
    let config_filename = ctx
        .config_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let schema_count = ctx.catalog.schemas.len();
    let group_count = ctx.catalog.groups.len();
    let source_count = ctx.source_count;

    let mut md = String::new();
    md.push_str("# Schema Catalog\n\n");
    md.push_str("This directory was generated by [`lintel-catalog-builder`](https://github.com/lintel-rs/lintel).\n");
    md.push_str("**Do not edit files in this directory manually** — they will be overwritten on the next run.\n\n");

    if let Some(ref repo_url) = source_repo {
        let _ = write!(md, "Source repository: <{repo_url}>\n\n");
    }

    md.push_str("## Stats\n\n");
    let _ = writeln!(md, "- **{schema_count}** schemas");
    let _ = writeln!(md, "- **{group_count}** groups");
    let _ = writeln!(md, "- **{source_count}** external sources\n");

    md.push_str("## Regenerate\n\n");
    md.push_str("```sh\n");
    let _ = writeln!(
        md,
        "lintel-catalog-builder generate --config {config_filename}"
    );
    md.push_str("```\n");

    let readme_path = ctx.output_dir.join("README.md");
    tokio::fs::write(&readme_path, md)
        .await
        .with_context(|| format!("failed to write {}", readme_path.display()))?;
    debug!(path = %readme_path.display(), "wrote README.md");
    Ok(())
}

/// Try to detect the git remote URL for a directory.
async fn detect_git_remote(dir: &Path) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .args(["-C", &dir.to_string_lossy(), "remote", "get-url", "origin"])
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = core::str::from_utf8(&output.stdout)
        .ok()?
        .trim()
        .to_string();
    if url.is_empty() {
        return None;
    }
    Some(url)
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use schema_catalog::SchemaEntry;

    use crate::catalog::build_output_catalog;
    use crate::download::ProcessedSchemas;
    use lintel_catalog_builder::config::{GitHubPagesConfig, SiteConfig};

    use super::*;

    #[tokio::test]
    async fn write_readme_generates_file() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let config_path = dir.path().join("lintel-catalog.toml");
        let catalog = build_output_catalog(
            None,
            vec![
                SchemaEntry {
                    name: "A".into(),
                    description: String::new(),
                    url: String::new(),
                    source_url: None,
                    file_match: vec![],
                    versions: BTreeMap::new(),
                },
                SchemaEntry {
                    name: "B".into(),
                    description: String::new(),
                    url: String::new(),
                    source_url: None,
                    file_match: vec![],
                    versions: BTreeMap::new(),
                },
            ],
            vec![schema_catalog::CatalogGroup {
                name: "G".into(),
                description: String::new(),
                schemas: vec![],
            }],
        );
        let processed = ProcessedSchemas::new(dir.path());
        let ctx = OutputContext {
            output_dir: dir.path(),
            config_path: &config_path,
            config_dir: dir.path(),
            catalog: &catalog,
            groups_meta: &[],
            base_url: "https://example.com/",
            source_count: 1,
            processed: &processed,
            site_description: None,
            ga_tracking_id: None,
            og_image: None,
        };
        write_readme(&ctx).await?;
        let content = tokio::fs::read_to_string(dir.path().join("README.md")).await?;
        assert!(content.contains("lintel-catalog-builder"));
        assert!(content.contains("**2** schemas"));
        assert!(content.contains("**1** groups"));
        assert!(content.contains("**1** external sources"));
        Ok(())
    }

    #[tokio::test]
    async fn finalize_writes_nojekyll_and_cname_when_github_present() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let catalog = build_output_catalog(None, vec![], vec![]);
        let processed = ProcessedSchemas::new(dir.path());
        let ctx = OutputContext {
            output_dir: dir.path(),
            config_path: Path::new("lintel-catalog.toml"),
            config_dir: dir.path(),
            catalog: &catalog,
            groups_meta: &[],
            base_url: "https://example.com/",
            source_count: 0,
            processed: &processed,
            site_description: None,
            ga_tracking_id: None,
            og_image: None,
        };
        let target = TargetConfig {
            dir: "out".into(),
            base_url: "https://example.com/".into(),
            site: Some(SiteConfig {
                description: None,
                ga_tracking_id: None,
                og_image: None,
                github: Some(GitHubPagesConfig {
                    cname: Some("example.com".into()),
                }),
            }),
        };
        finalize(&target, &ctx).await?;
        assert!(dir.path().join(".nojekyll").exists());
        let cname = tokio::fs::read_to_string(dir.path().join("CNAME")).await?;
        assert_eq!(cname.trim(), "example.com");
        assert!(dir.path().join("index.html").exists());
        Ok(())
    }

    #[tokio::test]
    async fn finalize_skips_github_files_without_github_config() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let catalog = build_output_catalog(None, vec![], vec![]);
        let processed = ProcessedSchemas::new(dir.path());
        let ctx = OutputContext {
            output_dir: dir.path(),
            config_path: Path::new("lintel-catalog.toml"),
            config_dir: dir.path(),
            catalog: &catalog,
            groups_meta: &[],
            base_url: "https://example.com/",
            source_count: 0,
            processed: &processed,
            site_description: None,
            ga_tracking_id: None,
            og_image: None,
        };
        let target = TargetConfig {
            dir: "out".into(),
            base_url: "https://example.com/".into(),
            site: None,
        };
        finalize(&target, &ctx).await?;
        assert!(!dir.path().join(".nojekyll").exists());
        assert!(!dir.path().join("CNAME").exists());
        Ok(())
    }

    #[tokio::test]
    async fn copy_static_dir_copies_files_recursively() -> Result<()> {
        let config_dir = tempfile::tempdir()?;
        let output_dir = tempfile::tempdir()?;

        // Create static/ with a file and a nested subdir
        let static_dir = config_dir.path().join("static");
        tokio::fs::create_dir_all(static_dir.join("sub")).await?;
        tokio::fs::write(static_dir.join("llms.txt"), "hello").await?;
        tokio::fs::write(static_dir.join("sub").join("nested.txt"), "world").await?;

        copy_static_dir(config_dir.path(), output_dir.path()).await?;

        assert_eq!(
            tokio::fs::read_to_string(output_dir.path().join("llms.txt")).await?,
            "hello"
        );
        assert_eq!(
            tokio::fs::read_to_string(output_dir.path().join("sub").join("nested.txt")).await?,
            "world"
        );
        Ok(())
    }

    #[tokio::test]
    async fn copy_static_dir_is_noop_when_missing() -> Result<()> {
        let config_dir = tempfile::tempdir()?;
        let output_dir = tempfile::tempdir()?;
        // No static/ dir — should succeed silently
        copy_static_dir(config_dir.path(), output_dir.path()).await?;
        Ok(())
    }
}
