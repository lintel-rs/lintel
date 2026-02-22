#![doc = include_str!("../README.md")]

extern crate alloc;

use std::path::PathBuf;
use std::process::ExitCode;

use bpaf::Bpaf;
use tracing_subscriber::prelude::*;

mod catalog;
mod commands;
mod config;
mod download;
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
    Generate {
        /// Path to lintel-catalog.toml config file
        #[bpaf(
            long("config"),
            argument("PATH"),
            fallback(PathBuf::from("lintel-catalog.toml"))
        )]
        config: PathBuf,

        /// Build only a specific target (default: all targets)
        #[bpaf(long("target"), argument("NAME"))]
        target: Option<String>,

        /// Maximum concurrent downloads
        #[bpaf(long("concurrency"), argument("N"), fallback(20))]
        concurrency: usize,

        /// Skip reading from cache (still writes fetched schemas to cache)
        #[bpaf(long("no-cache"), switch)]
        no_cache: bool,
    },

    /// Print version information
    #[bpaf(command("version"))]
    Version,
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

    let cli = cli().run();

    let result = match cli.command {
        Commands::Generate {
            config,
            target,
            concurrency,
            no_cache,
        } => commands::generate::run(&config, target.as_deref(), concurrency, no_cache).await,
        Commands::Version => {
            println!("lintel-catalog-builder {}", env!("CARGO_PKG_VERSION"));
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
    use super::*;

    #[test]
    fn cli_parses_generate_defaults() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["generate"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Generate {
                config,
                target,
                concurrency,
                no_cache,
            } => {
                assert_eq!(config, PathBuf::from("lintel-catalog.toml"));
                assert!(target.is_none());
                assert_eq!(concurrency, 20);
                assert!(!no_cache);
            }
            Commands::Version => panic!("expected Generate"),
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
            Commands::Generate {
                config,
                target,
                concurrency,
                no_cache,
            } => {
                assert_eq!(config, PathBuf::from("my-catalog.toml"));
                assert_eq!(target, Some("pages".to_string()));
                assert_eq!(concurrency, 50);
                assert!(no_cache);
            }
            Commands::Version => panic!("expected Generate"),
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
