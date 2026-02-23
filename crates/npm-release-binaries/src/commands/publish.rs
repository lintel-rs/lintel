use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{PackageConfig, TargetEntry};

pub struct Options<'a> {
    pub access: &'a str,
    pub dry_run: bool,
    pub provenance: bool,
}

pub fn run(
    pkg_config: &PackageConfig,
    output_dir: &Path,
    opts: &Options<'_>,
) -> miette::Result<()> {
    // Publish target packages first
    for (target_key, entry) in &pkg_config.targets {
        if matches!(entry, TargetEntry::Enabled(false)) {
            continue;
        }
        let package_name = pkg_config
            .target_package_name
            .replace("{{target}}", target_key);
        let pkg_path = package_dir(output_dir, &package_name);
        npm_publish(&pkg_path, &package_name, opts)?;
    }

    // Publish main package last
    let main_pkg_path = package_dir(output_dir, &pkg_config.name);
    npm_publish(&main_pkg_path, &pkg_config.name, opts)?;

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

fn npm_publish(pkg_dir: &Path, package_name: &str, opts: &Options<'_>) -> miette::Result<()> {
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY_SECS: u64 = 10;

    for attempt in 1..=MAX_RETRIES {
        let mut cmd = Command::new("npm");
        cmd.arg("publish");
        cmd.arg("--access").arg(opts.access);
        if opts.provenance {
            cmd.arg("--provenance");
        }
        if opts.dry_run {
            cmd.arg("--dry-run");
        }
        cmd.current_dir(pkg_dir);

        eprintln!(
            "{} {package_name} from {}{}",
            if opts.dry_run {
                "Dry-run publishing"
            } else {
                "Publishing"
            },
            pkg_dir.display(),
            if attempt > 1 {
                format!(" (attempt {attempt}/{MAX_RETRIES})")
            } else {
                String::new()
            }
        );

        let output = cmd
            .output()
            .map_err(|e| miette::miette!("failed to run npm publish: {e}"))?;

        if output.status.success() {
            // Forward stdout/stderr so publish details are visible in CI logs
            std::io::Write::write_all(&mut std::io::stdout(), &output.stdout).ok();
            std::io::Write::write_all(&mut std::io::stderr(), &output.stderr).ok();
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Forward output so the error is visible in CI logs
        std::io::Write::write_all(&mut std::io::stdout(), &output.stdout).ok();
        std::io::Write::write_all(&mut std::io::stderr(), &output.stderr).ok();

        // Retry on transient Sigstore transparency log errors
        let is_retryable = stderr.contains("TLOG_CREATE_ENTRY_ERROR");

        if is_retryable && attempt < MAX_RETRIES {
            eprintln!(
                "Transient publish error for {package_name}, retrying in {RETRY_DELAY_SECS}s..."
            );
            std::thread::sleep(core::time::Duration::from_secs(RETRY_DELAY_SECS));
            continue;
        }

        return Err(miette::miette!(
            "npm publish failed for {package_name} (exit code: {})",
            output
                .status
                .code()
                .map_or("signal".to_string(), |c| c.to_string())
        ));
    }

    unreachable!()
}
