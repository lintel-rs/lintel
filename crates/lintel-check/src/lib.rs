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
    #[bpaf(external(lintel_validate::validate_args))]
    pub validate: lintel_validate::ValidateArgs,
}

/// Construct the bpaf parser for `CheckArgs`.
pub fn check_args() -> impl bpaf::Parser<CheckArgs> {
    check_args_inner()
}

/// Run all checks: schema validation (and formatting in the future).
///
/// Returns `Ok(true)` if any errors were found, `Ok(false)` if clean.
///
/// # Errors
///
/// Returns an error if schema validation fails to run (e.g. network or I/O issues).
// TODO: also run formatting checks once lintel-format exists
pub async fn run(args: &mut CheckArgs, reporter: &mut dyn Reporter) -> Result<bool> {
    lintel_validate::run(&mut args.validate, reporter).await
}
