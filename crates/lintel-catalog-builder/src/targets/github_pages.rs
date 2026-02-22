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

    use super::*;

    #[tokio::test]
    async fn write_gh_pages_creates_nojekyll_and_index() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let catalog = build_output_catalog(None, vec![], vec![]);
        let ctx = OutputContext {
            output_dir: dir.path(),
            config_path: Path::new("lintel-catalog.toml"),
            catalog: &catalog,
            groups_meta: &[],
            source_count: 0,
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

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(
            super::super::html_escape("<b>\"hi\"&</b>"),
            "&lt;b&gt;&quot;hi&quot;&amp;&lt;/b&gt;"
        );
    }

    #[test]
    fn generate_index_html_contains_schema() {
        let catalog = build_output_catalog(
            None,
            vec![SchemaEntry {
                name: "Test Schema".into(),
                description: "A test".into(),
                url: "schemas/test.json".into(),
                source_url: None,
                file_match: vec!["*.test".into()],
                versions: BTreeMap::new(),
            }],
            vec![],
        );
        let html = super::super::generate_index_html(&catalog, &[]);
        assert!(html.contains("Test Schema"));
        assert!(html.contains("A test"));
        assert!(html.contains("schemas/test.json"));
    }
}
