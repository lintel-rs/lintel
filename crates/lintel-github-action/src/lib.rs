#![doc = include_str!("../README.md")]

use std::time::Instant;

use anyhow::{Context, Result, bail};
use bpaf::Bpaf;
use serde::Serialize;

use lintel_diagnostics::{DEFAULT_LABEL, LintelDiagnostic, offset_to_line_col};

// -----------------------------------------------------------------------
// CLI args
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Bpaf)]
#[bpaf(generate(github_action_args_inner))]
pub struct GithubActionArgs {
    #[bpaf(external(lintel_check::check_args))]
    pub check: lintel_check::CheckArgs,
}

/// Construct the bpaf parser for `GithubActionArgs`.
pub fn github_action_args() -> impl bpaf::Parser<GithubActionArgs> {
    github_action_args_inner()
}

// -----------------------------------------------------------------------
// GitHub Checks API types
// -----------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct CreateCheckRun {
    name: String,
    head_sha: String,
    status: String,
    conclusion: String,
    output: CheckRunOutput,
}

#[derive(Debug, Serialize)]
struct UpdateCheckRun {
    output: CheckRunOutput,
}

#[derive(Debug, Serialize)]
struct CheckRunOutput {
    title: String,
    summary: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Serialize)]
#[allow(clippy::struct_field_names)]
struct Annotation {
    path: String,
    start_line: usize,
    end_line: usize,
    annotation_level: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

fn error_to_annotation(error: &LintelDiagnostic) -> Annotation {
    let path = error.path().replace('\\', "/");
    let (line, _col) = match error {
        LintelDiagnostic::Parse { src, span, .. }
        | LintelDiagnostic::Validation { src, span, .. } => {
            offset_to_line_col(src.inner(), span.offset())
        }
        LintelDiagnostic::SchemaMismatch { line_number, .. } => (*line_number, 1),
        LintelDiagnostic::Io { .. }
        | LintelDiagnostic::SchemaFetch { .. }
        | LintelDiagnostic::SchemaCompile { .. }
        | LintelDiagnostic::Format { .. } => (1, 1),
    };

    let title = match error {
        LintelDiagnostic::Parse { .. } => Some("parse error".to_string()),
        LintelDiagnostic::Validation { instance_path, .. } if instance_path != DEFAULT_LABEL => {
            Some(instance_path.clone())
        }
        LintelDiagnostic::Validation { .. } => Some("validation error".to_string()),
        LintelDiagnostic::SchemaMismatch { .. } => Some("schema mismatch".to_string()),
        LintelDiagnostic::Io { .. } => Some("io error".to_string()),
        LintelDiagnostic::SchemaFetch { .. } => Some("schema fetch error".to_string()),
        LintelDiagnostic::SchemaCompile { .. } => Some("schema compile error".to_string()),
        LintelDiagnostic::Format { .. } => Some("format error".to_string()),
    };

    Annotation {
        path,
        start_line: line,
        end_line: line,
        annotation_level: "failure".to_string(),
        message: error.message().to_string(),
        title,
    }
}

fn build_summary(files_checked: usize, ms: u128, annotations: &[Annotation]) -> String {
    use core::fmt::Write;

    if annotations.is_empty() {
        return format!("Checked **{files_checked}** files in **{ms}ms**. No errors found.");
    }

    let mut s = format!("Checked **{files_checked}** files in **{ms}ms**.\n\n");
    s.push_str("| File | Line | Error |\n");
    s.push_str("|------|------|-------|\n");
    for ann in annotations {
        let _ = writeln!(
            s,
            "| `{}` | {} | {} |",
            ann.path, ann.start_line, ann.message
        );
    }
    s
}

#[allow(clippy::too_many_arguments)]
async fn post_check_run(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    title: &str,
    summary: &str,
    annotations: &[Annotation],
    sha: &str,
    conclusion: &str,
) -> Result<reqwest::Response> {
    // First batch (up to 50 annotations) — creates the check run
    let first_batch: Vec<Annotation> = annotations.iter().take(50).cloned().collect();
    let body = CreateCheckRun {
        name: "Lintel".to_string(),
        head_sha: sha.to_string(),
        status: "completed".to_string(),
        conclusion: conclusion.to_string(),
        output: CheckRunOutput {
            title: title.to_string(),
            summary: summary.to_string(),
            annotations: first_batch,
        },
    };

    let response = client
        .post(url)
        .header("Authorization", format!("token {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "lintel-github-action")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&body)
        .send()
        .await
        .context("failed to create check run")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        bail!("GitHub API returned {status}: {body}");
    }

    Ok(response)
}

#[allow(clippy::too_many_arguments)]
async fn patch_remaining_annotations(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    title: &str,
    summary: &str,
    annotations: &[Annotation],
    response: reqwest::Response,
) -> Result<()> {
    if annotations.len() <= 50 {
        return Ok(());
    }

    let check_run: serde_json::Value = response.json().await?;
    let check_run_id = check_run["id"]
        .as_u64()
        .context("missing check run id in response")?;
    let patch_url = format!("{url}/{check_run_id}");

    for chunk in annotations[50..].chunks(50) {
        let patch_body = UpdateCheckRun {
            output: CheckRunOutput {
                title: title.to_string(),
                summary: summary.to_string(),
                annotations: chunk.to_vec(),
            },
        };

        let resp = client
            .patch(&patch_url)
            .header("Authorization", format!("token {token}"))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "lintel-github-action")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&patch_body)
            .send()
            .await
            .context("failed to update check run with annotations")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            bail!("GitHub API returned {status} on PATCH: {body}");
        }
    }

    Ok(())
}

// -----------------------------------------------------------------------
// Public runner
// -----------------------------------------------------------------------

/// Run lintel checks and post results as a GitHub Check Run.
///
/// Reads `GITHUB_TOKEN`, `GITHUB_REPOSITORY`, `GITHUB_SHA`, and
/// (optionally) `GITHUB_API_URL` from the environment.
///
/// Returns `Ok(true)` if errors were found, `Ok(false)` if clean.
///
/// # Errors
///
/// Returns an error if environment variables are missing, validation
/// fails to run, or the GitHub Checks API request fails.
pub async fn run(args: &mut GithubActionArgs) -> Result<bool> {
    // Read required environment variables
    let token =
        std::env::var("GITHUB_TOKEN").context("GITHUB_TOKEN environment variable is required")?;
    let repository = std::env::var("GITHUB_REPOSITORY")
        .context("GITHUB_REPOSITORY environment variable is required")?;
    let sha = std::env::var("GITHUB_SHA").context("GITHUB_SHA environment variable is required")?;
    let api_url =
        std::env::var("GITHUB_API_URL").unwrap_or_else(|_| "https://api.github.com".to_string());

    // Run checks via lintel-check.
    let start = Instant::now();
    let result = lintel_check::check(&mut args.check, |_| {}).await?;
    let elapsed = start.elapsed();

    let files_checked = result.files_checked();
    let ms = elapsed.as_millis();

    // Convert all errors to annotations.
    let annotations: Vec<Annotation> = result.errors.iter().map(error_to_annotation).collect();

    let had_errors = !annotations.is_empty();
    let error_count = annotations.len();

    let error_label = if error_count == 1 { "error" } else { "errors" };
    let title = if error_count > 0 {
        format!("{error_count} {error_label} found")
    } else {
        "No errors".to_string()
    };
    let summary = build_summary(files_checked, ms, &annotations);
    let conclusion = if had_errors { "failure" } else { "success" };

    let client = reqwest::Client::new();
    let url = format!("{api_url}/repos/{repository}/check-runs");

    let response = post_check_run(
        &client,
        &url,
        &token,
        &title,
        &summary,
        &annotations,
        &sha,
        conclusion,
    )
    .await?;

    patch_remaining_annotations(
        &client,
        &url,
        &token,
        &title,
        &summary,
        &annotations,
        response,
    )
    .await?;

    eprintln!(
        "Checked {files_checked} files in {ms}ms. {error_count} {error_label}. Check run: {conclusion}."
    );

    Ok(had_errors)
}
