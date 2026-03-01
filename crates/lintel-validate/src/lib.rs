#![doc = include_str!("../README.md")]
#![allow(unused_assignments)] // thiserror/miette derive macros trigger false positives

extern crate alloc;

use std::time::Instant;

use anyhow::Result;
use bpaf::{Bpaf, ShellComp};
use lintel_diagnostics::reporter::{CheckResult, Reporter};

use lintel_cli_common::CliCacheOptions;

// -----------------------------------------------------------------------
// Core validation modules
// -----------------------------------------------------------------------

pub mod catalog;
pub mod discover;
pub mod parsers;
pub mod registry;
pub mod validate;

// -----------------------------------------------------------------------
// ValidateArgs — shared CLI struct
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Bpaf)]
pub struct ValidateArgs {
    #[bpaf(long("exclude"), argument("PATTERN"))]
    pub exclude: Vec<String>,

    #[bpaf(external(lintel_cli_common::cli_cache_options))]
    pub cache: CliCacheOptions,

    #[bpaf(positional("PATH"), complete_shell(ShellComp::File { mask: None }))]
    pub globs: Vec<String>,
}

impl From<&ValidateArgs> for validate::ValidateArgs {
    fn from(args: &ValidateArgs) -> Self {
        // When a single directory is passed as an arg, use it as the config
        // search directory so that `lintel.toml` inside that directory is found.
        let config_dir = args
            .globs
            .iter()
            .find(|g| std::path::Path::new(g).is_dir())
            .map(std::path::PathBuf::from);

        validate::ValidateArgs {
            globs: args.globs.clone(),
            exclude: args.exclude.clone(),
            cache_dir: args.cache.cache_dir.clone(),
            force_schema_fetch: args.cache.force_schema_fetch || args.cache.force,
            force_validation: args.cache.force_validation || args.cache.force,
            no_catalog: args.cache.no_catalog,
            config_dir,
            schema_cache_ttl: args.cache.schema_cache_ttl,
        }
    }
}

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

/// Load `lintel.toml` and merge its excludes into the args.
///
/// Config excludes are prepended so they have the same priority as CLI excludes.
/// When a directory arg is passed (e.g. `lintel check some/dir`), we search
/// for `lintel.toml` starting from that directory rather than cwd.
pub fn merge_config(args: &mut ValidateArgs) {
    let search_dir = args
        .globs
        .iter()
        .find(|g| std::path::Path::new(g).is_dir())
        .map(std::path::PathBuf::from);

    let cfg_result = match &search_dir {
        Some(dir) => lintel_config::find_and_load(dir).map(Option::unwrap_or_default),
        None => lintel_config::load(),
    };

    match cfg_result {
        Ok(cfg) => {
            // Config excludes first, then CLI excludes.
            let cli_excludes = core::mem::take(&mut args.exclude);
            args.exclude = cfg.exclude;
            args.exclude.extend(cli_excludes);
        }
        Err(e) => {
            eprintln!("warning: failed to load lintel.toml: {e}");
        }
    }
}

// -----------------------------------------------------------------------
// Run function — shared between check/ci/validate commands
// -----------------------------------------------------------------------

/// Run validation and report results via the given reporter.
///
/// Returns `true` if there were validation errors, `false` if clean.
///
/// # Errors
///
/// Returns an error if file collection or schema validation encounters an I/O error.
pub async fn run(args: &mut ValidateArgs, reporter: &mut dyn Reporter) -> Result<bool> {
    merge_config(args);

    let lib_args = validate::ValidateArgs::from(&*args);
    let start = Instant::now();
    let result: CheckResult = validate::run_with(&lib_args, None, |file| {
        reporter.on_file_checked(file);
    })
    .await?;
    let had_errors = result.has_errors();
    let elapsed = start.elapsed();

    reporter.report(result, elapsed);

    Ok(had_errors)
}
