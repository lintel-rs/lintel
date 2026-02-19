use std::time::Instant;

use anyhow::Result;

use lintel_check::retriever::HttpClient;
use lintel_check::validate::{self, LintError};

use crate::ValidateArgs;

use super::merge_config;

/// Run the `ci` command: plain one-line-per-error output suitable for CI pipelines.
///
/// Errors are printed as `error: <path>: <message>` which most CI systems
/// (GitHub Actions, GitLab CI, etc.) can parse for inline annotations.
pub async fn run<C: HttpClient>(args: &mut ValidateArgs, client: C, verbose: bool) -> Result<bool> {
    merge_config(args);

    let lib_args = validate::ValidateArgs::from(&*args);
    let start = Instant::now();
    let result = validate::run_with(&lib_args, client, |file| {
        if verbose {
            eprintln!("{}", super::format_checked_verbose(file));
        }
    })
    .await?;

    let error_count = result.errors.len();
    for error in &result.errors {
        match error {
            LintError::Validation(d) if !d.instance_path.is_empty() => {
                eprintln!("error: {} (at {})", error.message(), d.instance_path);
            }
            _ => {
                eprintln!("error: {}", error.message());
            }
        }
    }

    let ms = start.elapsed().as_millis();
    let n = result.files_checked();
    if error_count > 0 {
        eprintln!("Checked {n} files in {ms}ms. {error_count} error(s) found.");
    } else {
        eprintln!("Checked {n} files in {ms}ms. No errors.");
    }

    Ok(result.has_errors())
}
