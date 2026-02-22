use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{PackageConfig, TargetEntry};

pub fn run(pkg_config: &PackageConfig, output_dir: &Path, dry_run: bool) -> miette::Result<()> {
    let access = pkg_config.access.as_deref().unwrap_or("public");

    // Publish target packages first
    for (target_key, entry) in &pkg_config.targets {
        if matches!(entry, TargetEntry::Enabled(false)) {
            continue;
        }
        let package_name = pkg_config
            .target_package_name
            .replace("{{target}}", target_key);
        let pkg_path = package_dir(output_dir, &package_name);
        npm_publish(&pkg_path, &package_name, dry_run, access)?;
    }

    // Publish main package last
    let main_pkg_path = package_dir(output_dir, &pkg_config.name);
    npm_publish(&main_pkg_path, &pkg_config.name, dry_run, access)?;

    Ok(())
}

fn package_dir(output_dir: &Path, package_name: &str) -> PathBuf {
    if let Some(rest) = package_name.strip_prefix('@') {
        let (scope, name) = rest.split_once('/').expect("scoped package must have /");
        output_dir.join(format!("@{scope}")).join(name)
    } else {
        output_dir.join(package_name)
    }
}

fn npm_publish(
    pkg_dir: &Path,
    package_name: &str,
    dry_run: bool,
    access: &str,
) -> miette::Result<()> {
    let mut cmd = Command::new("npm");
    cmd.arg("publish");
    cmd.arg("--access").arg(access);
    if dry_run {
        cmd.arg("--dry-run");
    }
    cmd.current_dir(pkg_dir);

    eprintln!(
        "{} {package_name} from {}",
        if dry_run {
            "Dry-run publishing"
        } else {
            "Publishing"
        },
        pkg_dir.display()
    );

    let status = cmd
        .status()
        .map_err(|e| miette::miette!("failed to run npm publish: {e}"))?;

    if !status.success() {
        return Err(miette::miette!(
            "npm publish failed for {package_name} (exit code: {})",
            status
                .code()
                .map_or("signal".to_string(), |c| c.to_string())
        ));
    }

    Ok(())
}
