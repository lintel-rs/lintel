use std::path::{Path, PathBuf};

use anyhow::Result;

use super::{OutputContext, Target, write_common_files};

/// A target that writes output to a local directory.
pub struct DirTarget {
    pub dir: String,
    pub base_url: String,
}

impl Target for DirTarget {
    fn base_url(&self) -> &str {
        &self.base_url
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
        write_common_files(ctx).await
    }
}
