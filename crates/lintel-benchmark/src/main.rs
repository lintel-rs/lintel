#![doc = include_str!("../README.md")]

mod commands;
mod report;
mod runner;
mod tools;

use anyhow::Result;
use bpaf::Bpaf;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
/// Benchmark lintel against other JSON Schema validators.
///
/// Requires `cargo build --release --package lintel` first.
enum Cmd {
    #[bpaf(command("run"))]
    /// Run benchmarks and write results to BENCHMARKS.md
    Run {
        /// Which benchmarks to run (single, multi, repo)
        #[bpaf(positional("FILTER"))]
        filter: Option<commands::run::Filter>,
    },

    #[bpaf(command("setup"))]
    /// Install other validator tools for comparison
    Setup,
}

fn main() -> Result<()> {
    let cmd = cmd().run();

    match cmd {
        Cmd::Setup => commands::setup::run(),
        Cmd::Run { filter } => commands::run::run(filter.as_ref()),
    }
}
