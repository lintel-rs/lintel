#![doc = include_str!("../README.md")]

use anyhow::Result;
use bpaf::Bpaf;
pub use lintel_validate::Reporter;

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

/// Run all checks: schema validation and formatting.
///
/// Returns `Ok(true)` if any errors were found, `Ok(false)` if clean.
///
/// # Errors
///
/// Returns an error if schema validation fails to run (e.g. network or I/O issues).
pub async fn run(args: &mut CheckArgs, reporter: &mut dyn Reporter) -> Result<bool> {
    // Save original args before validate's merge_config modifies them.
    let original_globs = args.validate.globs.clone();
    let original_exclude = args.validate.exclude.clone();

    let had_validation_errors = lintel_validate::run(&mut args.validate, reporter).await?;

    if args.fix {
        let fixed = lintel_format::fix_format(&original_globs, &original_exclude)?;
        if fixed > 0 {
            eprintln!("Fixed formatting in {fixed} file(s).");
        }
        Ok(had_validation_errors)
    } else {
        // Check formatting using original (pre-merge) args so lintel-format
        // can load lintel.toml independently without double-merging excludes.
        let format_diagnostics = lintel_format::check_format(&original_globs, &original_exclude)?;
        let had_format_errors = !format_diagnostics.is_empty();

        for diag in format_diagnostics {
            eprintln!("{:?}", miette::Report::new(diag));
        }

        Ok(had_validation_errors || had_format_errors)
    }
}
