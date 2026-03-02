#![doc = include_str!("../README.md")]

use std::path::PathBuf;

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

/// Load `lintel.toml`, merge excludes, and return the config + merged excludes.
fn load_and_merge_config(
    args: &lintel_validate::ValidateArgs,
) -> (lintel_config::Config, Vec<String>) {
    let search_dir = args
        .globs
        .iter()
        .find(|g| std::path::Path::new(g).is_dir())
        .map(PathBuf::from);

    let cfg = match &search_dir {
        Some(dir) => lintel_config::find_and_load(dir)
            .ok()
            .flatten()
            .unwrap_or_default(),
        None => lintel_config::load().unwrap_or_default(),
    };

    // Config excludes first, then CLI excludes.
    let mut excludes = cfg.exclude.clone();
    excludes.extend(args.exclude.iter().cloned());

    (cfg, excludes)
}

/// Run all checks: schema validation and formatting.
///
/// Discovers files once and shares the file list between validation and
/// formatting. Config is loaded once.
///
/// Returns `Ok(true)` if any errors were found, `Ok(false)` if clean.
///
/// # Errors
///
/// Returns an error if schema validation fails to run (e.g. network or I/O issues).
pub async fn run(args: &mut CheckArgs, reporter: &mut dyn Reporter) -> Result<bool> {
    // 1. Load config once
    let (config, excludes) = load_and_merge_config(&args.validate);

    // 2. Discover files once (using validate's filter — superset of format's)
    let all_files = lintel_config::discover::collect_files(&args.validate.globs, &excludes, |p| {
        lintel_validate::parsers::detect_format(p).is_some()
    })?;

    // 3. Merge config into validate args (for schema mappings, overrides, etc.)
    //    We set excludes to empty since files are already filtered.
    args.validate.exclude = excludes;

    // 4. Run validation on discovered files
    let had_validation_errors =
        lintel_validate::run_with_files(&mut args.validate, all_files.clone(), reporter).await?;

    // 5. Run format check/fix on same files (format skips unsupported extensions)
    let format_config = lintel_format::format_config_from_lintel(&config);

    if args.fix {
        let fixed = lintel_format::fix_format_files(&all_files, &format_config)?;
        if fixed > 0 {
            eprintln!("Fixed formatting in {fixed} file(s).");
        }
        Ok(had_validation_errors)
    } else {
        let format_diagnostics = lintel_format::check_format_files(&all_files, &format_config);
        let had_format_errors = !format_diagnostics.is_empty();

        for diag in format_diagnostics {
            eprintln!("{:?}", miette::Report::new(diag));
        }

        Ok(had_validation_errors || had_format_errors)
    }
}
