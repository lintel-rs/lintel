use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::debug;

use super::{OutputContext, Target, write_common_files};

/// A target optimized for GitHub Pages deployment.
pub struct GitHubPagesTarget {
    pub base_url: String,
    pub cname: Option<String>,
    pub dir: Option<String>,
}

impl Target for GitHubPagesTarget {
    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn output_dir(&self, target_name: &str, config_dir: &Path) -> PathBuf {
        if let Some(dir) = &self.dir {
            let path = PathBuf::from(dir);
            if path.is_absolute() {
                path
            } else {
                config_dir.join(path)
            }
        } else {
            config_dir.join(".lintel-pages-output").join(target_name)
        }
    }

    async fn finalize(&self, ctx: &OutputContext<'_>) -> Result<()> {
        write_common_files(ctx).await?;

        // .nojekyll
        tokio::fs::write(ctx.output_dir.join(".nojekyll"), "")
            .await
            .context("failed to write .nojekyll")?;

        // CNAME
        if let Some(domain) = &self.cname {
            tokio::fs::write(ctx.output_dir.join("CNAME"), format!("{domain}\n"))
                .await
                .context("failed to write CNAME")?;
            debug!(domain, "wrote CNAME");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;

    use schema_catalog::SchemaEntry;

    use crate::catalog::build_output_catalog;
    use crate::download::ProcessedSchemas;

    use super::*;

    #[tokio::test]
    async fn write_gh_pages_creates_nojekyll_and_index() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let catalog = build_output_catalog(None, vec![], vec![]);
        let processed = ProcessedSchemas::new(dir.path());
        let ctx = OutputContext {
            output_dir: dir.path(),
            config_path: Path::new("lintel-catalog.toml"),
            catalog: &catalog,
            groups_meta: &[],
            base_url: "https://example.com/",
            source_count: 0,
            processed: &processed,
        };
        let target = GitHubPagesTarget {
            base_url: "https://example.com/".into(),
            cname: Some("example.com".into()),
            dir: None,
        };
        // Call finalize which writes common files + github pages files
        // We just test the github-pages-specific parts here
        target.finalize(&ctx).await?;
        assert!(dir.path().join(".nojekyll").exists());
        let cname = tokio::fs::read_to_string(dir.path().join("CNAME")).await?;
        assert_eq!(cname.trim(), "example.com");
        assert!(dir.path().join("index.html").exists());
        Ok(())
    }

    #[tokio::test]
    async fn generate_site_contains_schema() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let catalog = build_output_catalog(
            None,
            vec![SchemaEntry {
                name: "Test Schema".into(),
                description: "A test".into(),
                url: "https://example.com/schemas/test/latest.json".into(),
                source_url: None,
                file_match: vec!["*.test".into()],
                versions: BTreeMap::new(),
            }],
            vec![],
        );
        let processed = ProcessedSchemas::new(dir.path());
        let ctx = OutputContext {
            output_dir: dir.path(),
            config_path: Path::new("lintel-catalog.toml"),
            catalog: &catalog,
            groups_meta: &[],
            base_url: "https://example.com/",
            source_count: 0,
            processed: &processed,
        };
        crate::html::generate_site(&ctx).await?;

        let index = std::fs::read_to_string(dir.path().join("index.html"))?;
        assert!(index.contains("Test Schema"));
        assert!(index.contains("A test"));

        // Schema detail page should exist
        let schema_page = dir.path().join("schemas/test/index.html");
        assert!(schema_page.exists());
        let schema_html = std::fs::read_to_string(&schema_page)?;
        assert!(schema_html.contains("Test Schema"));
        assert!(schema_html.contains("*.test"));
        Ok(())
    }
}
