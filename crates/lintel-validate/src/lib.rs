#![doc = include_str!("../README.md")]
#![allow(unused_assignments)] // thiserror/miette derive macros trigger false positives

extern crate alloc;

use std::time::Instant;

use anyhow::Result;
use bpaf::{Bpaf, ShellComp};
use lintel_config::ConfigContext;
use lintel_diagnostics::reporter::{CheckResult, Reporter};

use lintel_cli_common::CliCacheOptions;

// -----------------------------------------------------------------------
// Core validation modules
// -----------------------------------------------------------------------

pub mod catalog;
pub mod parsers;
pub mod registry;
pub(crate) mod suggest;
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

impl ValidateArgs {
    /// Build internal [`validate::ValidateArgs`] using the given [`ConfigContext`].
    pub fn to_lib_args(&self, _ctx: &ConfigContext) -> validate::ValidateArgs {
        validate::ValidateArgs {
            globs: self.globs.clone(),
            cache_dir: self.cache.cache_dir.clone(),
            force_schema_fetch: self.cache.force_schema_fetch || self.cache.force,
            force_validation: self.cache.force_validation || self.cache.force,
            no_catalog: self.cache.no_catalog,
            schema_cache_ttl: self.cache.schema_cache_ttl,
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
pub async fn run(
    args: &ValidateArgs,
    ctx: &ConfigContext,
    reporter: &mut dyn Reporter,
) -> Result<bool> {
    let lib_args = args.to_lib_args(ctx);
    let start = Instant::now();
    let result: CheckResult = validate::run_with(&lib_args, ctx, None, |file| {
        reporter.on_file_checked(file);
    })
    .await?;
    let had_errors = result.has_errors();
    let elapsed = start.elapsed();

    reporter.report(result, elapsed);

    Ok(had_errors)
}
