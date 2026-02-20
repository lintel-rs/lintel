use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use tracing::{debug, info};

const DEFAULT_REPO: &str = "lintel-rs/schemastore-catalog";
const DEFAULT_BRANCH: &str = "main";

/// Run the `update` subcommand: clone repo, generate, check, commit+push.
pub async fn run(repo: Option<&str>, branch: Option<&str>) -> Result<()> {
    let repo = repo.unwrap_or(DEFAULT_REPO);
    let branch = branch.unwrap_or(DEFAULT_BRANCH);

    // 1. Read GITHUB_TOKEN
    let token = std::env::var("GITHUB_TOKEN")
        .context("GITHUB_TOKEN environment variable is required for the update command")?;

    // 2. Shallow-clone to tempdir
    let tmpdir = tempfile::tempdir().context("failed to create temp directory")?;
    let clone_url = format!("https://x-access-token:{token}@github.com/{repo}.git");
    let clone_dir = tmpdir.path().join("repo");

    info!(repo = %repo, branch = %branch, "cloning repository");
    run_git(&[
        "clone",
        "--depth=1",
        "--branch",
        branch,
        &clone_url,
        &clone_dir.display().to_string(),
    ])?;

    // 3. Generate into cloned dir
    let base_url = format!("https://raw.githubusercontent.com/{repo}/{branch}/schemas");
    info!("generating catalog");
    super::generate::run(&clone_dir, None, Some(&base_url)).await?;

    // 4. Check for changes
    info!("checking for changes");
    let status_output = run_git_in(&clone_dir, &["status", "--porcelain"])?;

    if status_output.trim().is_empty() {
        info!("no changes detected, exiting");
        return Ok(());
    }

    info!(changes = %status_output.trim(), "changes detected");

    // 5. Run lintel check on the generated catalog
    info!("running lintel check");
    run_lintel_check(&clone_dir).await?;
    info!("lintel check passed");

    // 6. Configure git user
    info!("configuring git user");
    run_git_in(&clone_dir, &["config", "user.name", "github-actions[bot]"])?;
    run_git_in(
        &clone_dir,
        &[
            "config",
            "user.email",
            "github-actions[bot]@users.noreply.github.com",
        ],
    )?;

    // 7. Commit and push
    info!("committing and pushing");
    run_git_in(&clone_dir, &["add", "-A"])?;
    run_git_in(&clone_dir, &["commit", "-m", "Update SchemaStore catalog"])?;
    run_git_in(&clone_dir, &["push"])?;

    info!("catalog updated and pushed");
    Ok(())
}

/// Run lintel validation on the given directory using the library directly.
async fn run_lintel_check(dir: &Path) -> Result<()> {
    let args = lintel_check::validate::ValidateArgs {
        globs: vec![dir.display().to_string()],
        exclude: vec![],
        cache_dir: None,
        force_schema_fetch: false,
        force_validation: false,
        no_catalog: false,
        format: None,
        config_dir: Some(dir.to_path_buf()),
        schema_cache_ttl: Some(lintel_check::retriever::DEFAULT_SCHEMA_CACHE_TTL),
    };

    let client = lintel_schema_cache::ReqwestClient::default();
    let result = lintel_check::validate::run(&args, client).await?;

    if result.has_errors() {
        for error in &result.errors {
            tracing::error!("{}", error.path());
        }
        bail!(
            "lintel check failed with {} error(s), refusing to commit",
            result.errors.len()
        );
    }

    info!(
        files_checked = result.files_checked(),
        "lintel check completed"
    );
    Ok(())
}

/// Run a git command and return its stdout.
fn run_git(args: &[&str]) -> Result<String> {
    debug!(cmd = "git", args = ?args, "running command");
    let output = Command::new("git")
        .args(args)
        .output()
        .with_context(|| format!("failed to run git {}", args.first().unwrap_or(&"")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            stderr.trim()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    debug!(exit_code = output.status.code(), "command completed");
    Ok(stdout)
}

/// Run a git command in a specific directory and return its stdout.
fn run_git_in(dir: &std::path::Path, args: &[&str]) -> Result<String> {
    debug!(cmd = "git", args = ?args, dir = %dir.display(), "running command");
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("failed to run git {}", args.first().unwrap_or(&"")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            stderr.trim()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    debug!(exit_code = output.status.code(), "command completed");
    Ok(stdout)
}
