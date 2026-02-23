#![doc = include_str!("../README.md")]

extern crate alloc;

use std::process::ExitCode;

use bpaf::Bpaf;
use tracing_subscriber::prelude::*;

mod catalog;
mod commands;
mod download;
mod generate;
mod refs;
mod targets;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version, fallback_to_usage, generate(cli))]
#[allow(clippy::upper_case_acronyms)]
/// Build a custom schema catalog from local schemas and external sources
struct CLI {
    #[bpaf(external(commands))]
    command: Commands,
}

#[derive(Debug, Clone, Bpaf)]
enum Commands {
    /// Generate catalog.json and download schemas
    #[bpaf(command("generate"))]
    Generate(#[bpaf(external(commands::generate::generate_args))] commands::generate::GenerateArgs),

    /// Print version information
    #[bpaf(command("version"))]
    Version,

    #[bpaf(command("man"), hide)]
    /// Generate man page in roff format
    Man,
}

#[tokio::main]
async fn main() -> ExitCode {
    // Set up tracing subscriber. Uses LINTEL_LOG env var if set, otherwise
    // defaults to `info` level so that fetch URLs and progress are always visible.
    // Verbose entry/exit is only enabled when LINTEL_LOG is explicitly set.
    let (filter, explicit) = match tracing_subscriber::EnvFilter::try_from_env("LINTEL_LOG") {
        Ok(f) => (f, true),
        Err(_) => (tracing_subscriber::EnvFilter::new("info"), false),
    };
    tracing_subscriber::registry()
        .with(
            tracing_tree::HierarchicalLayer::new(2)
                .with_targets(true)
                .with_bracketed_fields(true)
                .with_indent_lines(true)
                .with_verbose_exit(explicit)
                .with_verbose_entry(explicit)
                .with_timer(tracing_tree::time::Uptime::default())
                .with_writer(std::io::stderr),
        )
        .with(filter)
        .init();

    let opts = cli().run();

    let result = match opts.command {
        Commands::Generate(args) => args.run().await,
        Commands::Version => {
            println!("lintel-catalog-builder {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Commands::Man => {
            let roff = cli().render_manpage(
                "lintel-catalog-builder",
                bpaf::doc::Section::General,
                None,
                None,
                Some("Lintel Manual"),
            );
            print!("{roff}");
            return ExitCode::SUCCESS;
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e:#}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn cli_parses_generate_defaults() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["generate"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Generate(args) => {
                assert_eq!(args.config, PathBuf::from("lintel-catalog.toml"));
                assert!(args.target.is_none());
                assert_eq!(args.concurrency, 20);
                assert!(!args.no_cache);
            }
            _ => panic!("expected Generate"),
        }
        Ok(())
    }

    #[test]
    fn cli_parses_generate_with_options() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&[
                "generate",
                "--config",
                "my-catalog.toml",
                "--target",
                "pages",
                "--concurrency",
                "50",
                "--no-cache",
            ])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Generate(args) => {
                assert_eq!(args.config, PathBuf::from("my-catalog.toml"));
                assert_eq!(args.target, Some("pages".to_string()));
                assert_eq!(args.concurrency, 50);
                assert!(args.no_cache);
            }
            _ => panic!("expected Generate"),
        }
        Ok(())
    }

    #[test]
    fn cli_parses_version() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["version"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        assert!(matches!(parsed.command, Commands::Version));
        Ok(())
    }
}
