use std::path::Path;

use crate::config::PackageConfig;

pub fn run(
    pkg_key: &str,
    pkg_config: &PackageConfig,
    version: &str,
    artifacts_dir: Option<&Path>,
    output_dir: &Path,
    skip_artifact_copy: bool,
) -> miette::Result<()> {
    super::generate::run(
        pkg_key,
        pkg_config,
        version,
        artifacts_dir,
        output_dir,
        skip_artifact_copy,
    )?;
    super::publish::run(pkg_config, output_dir, false)?;
    Ok(())
}
