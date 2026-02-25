use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::debug;

use lintel_catalog_builder::config::SiteConfig;

use super::{OutputContext, Target, write_common_files};

/// A target that writes output to a local directory.
pub struct DirTarget {
    pub dir: String,
    pub base_url: String,
    pub site: Option<SiteConfig>,
}

impl Target for DirTarget {
    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn site_description(&self) -> Option<&str> {
        self.site.as_ref().and_then(|s| s.description.as_deref())
    }

    fn output_dir(&self, _target_name: &str, config_dir: &Path) -> PathBuf {
        let path = PathBuf::from(&self.dir);
        if path.is_absolute() {
            path
        } else {
            config_dir.join(path)
        }
    }

    async fn finalize(&self, ctx: &OutputContext<'_>) -> Result<()> {
        write_common_files(ctx).await?;

        if let Some(gh) = self.site.as_ref().and_then(|s| s.github.as_ref()) {
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

        Ok(())
    }
}
