use std::path::PathBuf;
use std::process::ExitCode;

use bpaf::Bpaf;
use tracing_subscriber::prelude::*;

mod catalog;
mod commands;
mod config;
mod download;
mod refs;

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
        /// Output directory (defaults to config file's parent directory)
        #[bpaf(short('o'), long("output"), argument("DIR"))]
        output: Option<PathBuf>,

        /// Path to lintel-catalog.toml config file
        #[bpaf(
            long("config"),
            argument("PATH"),
            fallback(PathBuf::from("lintel-catalog.toml"))
        )]
        config: PathBuf,

        /// Maximum concurrent downloads
        #[bpaf(long("concurrency"), argument("N"), fallback(20))]
        concurrency: usize,
    },

    /// Print version information
    #[bpaf(command("version"))]
    Version,
}

#[tokio::main]
async fn main() -> ExitCode {
    // Set up tracing subscriber controlled by LINTEL_LOG env var.
    if let Ok(filter) = tracing_subscriber::EnvFilter::try_from_env("LINTEL_LOG") {
        tracing_subscriber::registry()
            .with(
                tracing_tree::HierarchicalLayer::new(2)
                    .with_targets(true)
                    .with_bracketed_fields(true)
                    .with_indent_lines(true)
                    .with_verbose_exit(true)
                    .with_verbose_entry(true)
                    .with_timer(tracing_tree::time::Uptime::default())
                    .with_writer(std::io::stderr),
            )
            .with(filter)
            .init();
    }

    let cli = cli().run();

    let result = match cli.command {
        Commands::Generate {
            output,
            config,
            concurrency,
        } => commands::generate::run(&config, output.as_deref(), concurrency).await,
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
                output,
                config,
                concurrency,
            } => {
                assert!(output.is_none());
                assert_eq!(config, PathBuf::from("lintel-catalog.toml"));
                assert_eq!(concurrency, 20);
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
                "-o",
                "/tmp/out",
                "--config",
                "my-catalog.toml",
                "--concurrency",
                "50",
            ])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Generate {
                output,
                config,
                concurrency,
            } => {
                assert_eq!(output, Some(PathBuf::from("/tmp/out")));
                assert_eq!(config, PathBuf::from("my-catalog.toml"));
                assert_eq!(concurrency, 50);
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
