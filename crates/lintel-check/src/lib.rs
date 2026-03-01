#![doc = include_str!("../README.md")]

use std::time::Instant;

use anyhow::Result;
use bpaf::Bpaf;

use lintel_diagnostics::reporter::{CheckResult, CheckedFile, Reporter};

// -----------------------------------------------------------------------
// CheckArgs — CLI struct for the `lintel check` command
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Bpaf)]
#[bpaf(generate(check_args_inner))]
pub struct CheckArgs {
    /// Automatically fix formatting issues
    #[bpaf(long("fix"), switch)]
    pub fix: bool,

    #[bpaf(external(lintel_validate::validate_args))]
    pub validate: lintel_validate::ValidateArgs,
}

/// Construct the bpaf parser for `CheckArgs`.
pub fn check_args() -> impl bpaf::Parser<CheckArgs> {
    check_args_inner()
}

/// Run all checks and return the result without reporting.
///
/// Calls `on_file_checked` for each file as it is validated (for streaming
/// progress). The caller is responsible for reporting the result.
///
/// # Errors
///
/// Returns an error if schema validation fails to run (e.g. network or I/O issues).
pub async fn check(
    args: &mut CheckArgs,
    on_file_checked: impl FnMut(&CheckedFile),
) -> Result<CheckResult> {
    // Save original args before validate's merge_config modifies them.
    let original_globs = args.validate.globs.clone();
    let original_exclude = args.validate.exclude.clone();

    lintel_validate::merge_config(&mut args.validate);

    let lib_args = lintel_validate::validate::ValidateArgs::from(&args.validate);

    // Collect and read files once.
    let files = lintel_validate::validate::collect_files(&lib_args.globs, &lib_args.exclude)?;
    let mut read_errors = Vec::new();
    let file_contents = lintel_validate::validate::read_files(&files, &mut read_errors).await;

    if args.fix {
        let fixed = lintel_format::fix_format(&original_globs, &original_exclude)?;
        if fixed > 0 {
            eprintln!("Fixed formatting in {fixed} file(s).");
        }

        let mut result = lintel_validate::validate::run_with_contents(
            &lib_args,
            file_contents,
            None,
            on_file_checked,
        )
        .await?;
        result.errors.extend(read_errors);
        sort_errors(&mut result.errors);
        Ok(result)
    } else {
        // Check formatting using pre-read contents (borrows, no extra I/O).
        let format_errors = lintel_format::check_format_contents(
            &file_contents,
            &original_globs,
            &original_exclude,
        );

        // Run validation (takes ownership of file contents).
        let mut result = lintel_validate::validate::run_with_contents(
            &lib_args,
            file_contents,
            None,
            on_file_checked,
        )
        .await?;

        // Merge format errors and I/O errors, then sort.
        result.errors.extend(format_errors);
        result.errors.extend(read_errors);
        sort_errors(&mut result.errors);

        Ok(result)
    }
}

/// Run all checks: schema validation and formatting.
///
/// Returns `Ok(true)` if any errors were found, `Ok(false)` if clean.
///
/// # Errors
///
/// Returns an error if schema validation fails to run (e.g. network or I/O issues).
pub async fn run(args: &mut CheckArgs, reporter: &mut dyn Reporter) -> Result<bool> {
    let start = Instant::now();
    let result = check(args, |file| reporter.on_file_checked(file)).await?;
    let had_errors = result.has_errors();
    let elapsed = start.elapsed();
    reporter.report(result, elapsed);
    Ok(had_errors)
}

fn sort_errors(errors: &mut [lintel_diagnostics::LintelDiagnostic]) {
    errors.sort_by(|a, b| {
        a.path()
            .cmp(b.path())
            .then_with(|| a.offset().cmp(&b.offset()))
    });
}
