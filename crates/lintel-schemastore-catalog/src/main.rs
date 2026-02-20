use std::path::PathBuf;
use std::process::ExitCode;

use bpaf::Bpaf;
use tracing_subscriber::prelude::*;

mod catalog;
mod commands;
mod download;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version, fallback_to_usage)]
/// Mirror the `SchemaStore` catalog into a self-hosted git repo
struct Cli {
    #[bpaf(external(commands))]
    command: Commands,
}

#[derive(Debug, Clone, Bpaf)]
enum Commands {
    /// Fetch catalog and download all schemas to an output directory
    #[bpaf(command("generate"))]
    Generate {
        /// Output directory for catalog.json and schemas/
        #[bpaf(short('o'), long("output"), argument("DIR"))]
        output: PathBuf,

        /// Maximum concurrent downloads
        #[bpaf(long("concurrency"), argument("N"), fallback(20))]
        concurrency: usize,

        /// Base URL prefix for rewritten schema URLs in catalog.json
        #[bpaf(long("base-url"), argument("URL"))]
        base_url: Option<String>,
    },

    /// Clone repo, generate catalog, run lintel check, commit and push
    #[bpaf(command("update"))]
    Update {
        /// GitHub repository in OWNER/NAME format
        #[bpaf(long("repo"), argument("OWNER/NAME"))]
        repo: Option<String>,

        /// Branch to update
        #[bpaf(long("branch"), argument("BRANCH"))]
        branch: Option<String>,
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
            concurrency,
            base_url,
        } => commands::generate::run(&output, Some(concurrency), base_url.as_deref()).await,
        Commands::Update { repo, branch } => {
            commands::update::run(repo.as_deref(), branch.as_deref()).await
        }
        Commands::Version => {
            println!("lintel-schemastore-catalog {}", env!("CARGO_PKG_VERSION"));
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
    fn cli_parses_generate() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["generate", "-o", "/tmp/out"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Generate {
                output,
                concurrency,
                base_url,
            } => {
                assert_eq!(output, PathBuf::from("/tmp/out"));
                assert_eq!(concurrency, 20);
                assert!(base_url.is_none());
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
                "-o",
                "/tmp/out",
                "--concurrency",
                "50",
                "--base-url",
                "https://example.com/schemas",
            ])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Generate {
                output,
                concurrency,
                base_url,
            } => {
                assert_eq!(output, PathBuf::from("/tmp/out"));
                assert_eq!(concurrency, 50);
                assert_eq!(base_url.as_deref(), Some("https://example.com/schemas"));
            }
            _ => panic!("expected Generate"),
        }
        Ok(())
    }

    #[test]
    fn cli_parses_update_defaults() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["update"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Update { repo, branch } => {
                assert!(repo.is_none());
                assert!(branch.is_none());
            }
            _ => panic!("expected Update"),
        }
        Ok(())
    }

    #[test]
    fn cli_parses_update_with_options() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["update", "--repo", "my-org/my-repo", "--branch", "dev"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Update { repo, branch } => {
                assert_eq!(repo.as_deref(), Some("my-org/my-repo"));
                assert_eq!(branch.as_deref(), Some("dev"));
            }
            _ => panic!("expected Update"),
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
