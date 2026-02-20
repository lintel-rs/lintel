use std::process::ExitCode;

use bpaf::Bpaf;
use tracing_subscriber::prelude::*;

use lintel_annotate::annotate_args;
use lintel_reporters::{
    CliOptions, ReporterKind, ValidateArgs, cli_options, make_reporter, validate_args,
};

mod commands;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Yaml,
    Toml,
}

impl core::str::FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(Self::Json),
            "yaml" => Ok(Self::Yaml),
            "toml" => Ok(Self::Toml),
            _ => Err(format!(
                "unknown output format '{s}', expected: json, yaml, toml"
            )),
        }
    }
}

#[derive(Debug, Clone, Bpaf)]
pub struct IdentifyArgs {
    /// Show detailed schema documentation
    #[bpaf(long("explain"), switch)]
    pub explain: bool,

    #[bpaf(long("no-catalog"), switch)]
    pub no_catalog: bool,

    #[bpaf(long("cache-dir"), argument("DIR"))]
    pub cache_dir: Option<String>,

    /// Schema cache TTL (e.g. "12h", "30m", "1d"); default 12h
    #[bpaf(long("schema-cache-ttl"), argument("DURATION"))]
    pub schema_cache_ttl: Option<String>,

    /// File to identify
    #[bpaf(positional("FILE"))]
    pub file: String,
}

#[derive(Debug, Clone, Bpaf)]
pub struct ConvertArgs {
    /// Output format
    #[bpaf(long("to"), argument("FORMAT"))]
    pub to: OutputFormat,

    /// Input file to convert
    #[bpaf(positional("FILE"))]
    pub file: String,
}

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version, fallback_to_usage)]
/// Validate JSON and YAML files against JSON Schema
struct Cli {
    #[bpaf(external(commands))]
    command: Commands,
}

#[derive(Debug, Clone, Bpaf)]
enum Commands {
    #[bpaf(command("check"))]
    /// Validate files against their schemas
    Check(
        #[bpaf(external(cli_options), hide_usage)] CliOptions,
        /// Output format
        #[bpaf(
            long("reporter"),
            argument("pretty|text|github"),
            fallback(ReporterKind::Pretty)
        )]
        ReporterKind,
        #[bpaf(external(validate_args))] ValidateArgs,
    ),

    #[bpaf(command("ci"))]
    /// Validate files with CI-friendly output
    CI(
        #[bpaf(external(cli_options), hide_usage)] CliOptions,
        /// Output format
        #[bpaf(
            long("reporter"),
            argument("pretty|text|github"),
            fallback(ReporterKind::Text)
        )]
        ReporterKind,
        #[bpaf(external(validate_args))] ValidateArgs,
    ),

    #[bpaf(command("identify"))]
    /// Show which schema a file resolves to
    Identify(#[bpaf(external(identify_args))] IdentifyArgs),

    #[bpaf(command("init"))]
    /// Create a lintel.toml configuration file
    Init,

    #[bpaf(command("convert"))]
    /// Convert between JSON, YAML, and TOML formats
    Convert(#[bpaf(external(convert_args))] ConvertArgs),

    #[bpaf(command("annotate"))]
    /// Add schema annotations to files
    Annotate(
        #[bpaf(external(cli_options), hide_usage)] CliOptions,
        #[bpaf(external(annotate_args))] lintel_annotate::AnnotateArgs,
    ),

    #[bpaf(command("version"))]
    /// Print version information
    Version,
}

#[tokio::main]
async fn main() -> ExitCode {
    // Set up tracing subscriber controlled by LINTEL_LOG env var.
    // e.g. LINTEL_LOG=info or LINTEL_LOG=lintel=debug,lintel_check=trace
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

    miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .terminal_links(true)
                .context_lines(2)
                .build(),
        )
    }))
    .ok();

    let cli = cli().run();

    let result = match cli.command {
        Commands::Check(cli_options, reporter_kind, mut args)
        | Commands::CI(cli_options, reporter_kind, mut args) => {
            let mut reporter = make_reporter(reporter_kind, cli_options.verbose);
            lintel_reporters::run(
                &mut args,
                lintel_check::retriever::ReqwestClient::default(),
                reporter.as_mut(),
            )
            .await
        }
        Commands::Identify(args) => {
            commands::identify::run(args, lintel_check::retriever::ReqwestClient::default()).await
        }
        Commands::Annotate(cli_options, args) => {
            commands::annotate::run(
                &args,
                lintel_check::retriever::ReqwestClient::default(),
                cli_options.verbose,
            )
            .await
        }
        Commands::Init => match commands::init::run() {
            Ok(()) => return ExitCode::SUCCESS,
            Err(e) => Err(e),
        },
        Commands::Convert(args) => match commands::convert::run(&args) {
            Ok(()) => return ExitCode::SUCCESS,
            Err(e) => Err(e),
        },
        Commands::Version => {
            println!("lintel {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
    };

    match result {
        Ok(had_errors) => {
            if had_errors {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("Error: {e:#}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_check_basic_args() -> anyhow::Result<()> {
        let cli = cli()
            .run_inner(&["check", "*.json"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match cli.command {
            Commands::Check(_, _, args) => {
                assert_eq!(args.globs, vec!["*.json"]);
                assert!(args.exclude.is_empty());
                assert!(args.cache_dir.is_none());
                assert!(!args.force_schema_fetch);
                assert!(!args.force_validation);
                assert!(!args.force);
                assert!(!args.no_catalog);
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_parses_check_all_options() -> anyhow::Result<()> {
        let cli = cli()
            .run_inner(&[
                "check",
                "*.json",
                "**/*.json",
                "--exclude",
                "node_modules/**",
                "--exclude",
                "vendor/**",
                "--cache-dir",
                "/tmp/cache",
                "--force-schema-fetch",
                "--force-validation",
                "--no-catalog",
            ])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match cli.command {
            Commands::Check(_, _, args) => {
                assert_eq!(args.globs, vec!["*.json", "**/*.json"]);
                assert_eq!(args.exclude, vec!["node_modules/**", "vendor/**"]);
                assert_eq!(args.cache_dir.as_deref(), Some("/tmp/cache"));
                assert!(args.force_schema_fetch);
                assert!(args.force_validation);
                assert!(!args.force);
                assert!(args.no_catalog);
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_force_implies_both_force_flags() -> anyhow::Result<()> {
        let cli = cli()
            .run_inner(&["check", "--force"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match cli.command {
            Commands::Check(_, _, args) => {
                assert!(args.force);
                // The individual flags should be false in the CLI struct --
                // the combination happens in the From impl.
                let lib_args = lintel_check::validate::ValidateArgs::from(&args);
                assert!(lib_args.force_schema_fetch);
                assert!(lib_args.force_validation);
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_check_no_globs_is_valid() -> anyhow::Result<()> {
        let cli = cli()
            .run_inner(&["check"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match cli.command {
            Commands::Check(_, _, args) => {
                assert!(args.globs.is_empty());
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_parses_ci_subcommand() -> anyhow::Result<()> {
        let cli = cli()
            .run_inner(&["ci", "*.json", "--no-catalog"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match cli.command {
            Commands::CI(_, _, args) => {
                assert_eq!(args.globs, vec!["*.json"]);
                assert!(args.no_catalog);
            }
            _ => panic!("expected CI"),
        }
        Ok(())
    }

    #[test]
    fn cli_verbose_short_after_subcommand() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["check", "-v", "*.json"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Check(cli_options, _, args) => {
                assert!(cli_options.verbose);
                assert_eq!(args.globs, vec!["*.json"]);
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_verbose_long_after_subcommand() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["check", "--verbose"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Check(cli_options, _, _) => {
                assert!(cli_options.verbose);
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_check_default_reporter_is_pretty() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["check"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Check(_, reporter_kind, _) => {
                assert_eq!(reporter_kind, ReporterKind::Pretty);
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_ci_default_reporter_is_text() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["ci"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::CI(_, reporter_kind, _) => {
                assert_eq!(reporter_kind, ReporterKind::Text);
            }
            _ => panic!("expected CI"),
        }
        Ok(())
    }

    #[test]
    fn cli_check_reporter_github() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["check", "--reporter", "github"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Check(_, reporter_kind, _) => {
                assert_eq!(reporter_kind, ReporterKind::Github);
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_ci_reporter_pretty() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["ci", "--reporter", "pretty"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::CI(_, reporter_kind, _) => {
                assert_eq!(reporter_kind, ReporterKind::Pretty);
            }
            _ => panic!("expected CI"),
        }
        Ok(())
    }

    #[test]
    fn cli_parses_identify_basic() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["identify", "file.json"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Identify(args) => {
                assert_eq!(args.file, "file.json");
                assert!(!args.explain);
                assert!(!args.no_catalog);
                assert!(args.cache_dir.is_none());
                assert!(args.schema_cache_ttl.is_none());
            }
            _ => panic!("expected Identify"),
        }
        Ok(())
    }

    #[test]
    fn cli_parses_identify_explain() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["identify", "--explain", "file.json"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Identify(args) => {
                assert_eq!(args.file, "file.json");
                assert!(args.explain);
            }
            _ => panic!("expected Identify"),
        }
        Ok(())
    }

    #[test]
    fn cli_parses_identify_no_catalog() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["identify", "--no-catalog", "file.json"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Identify(args) => {
                assert_eq!(args.file, "file.json");
                assert!(args.no_catalog);
            }
            _ => panic!("expected Identify"),
        }
        Ok(())
    }

    #[test]
    fn cli_parses_identify_all_options() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&[
                "identify",
                "--explain",
                "--no-catalog",
                "--cache-dir",
                "/tmp/cache",
                "--schema-cache-ttl",
                "30m",
                "tsconfig.json",
            ])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Identify(args) => {
                assert_eq!(args.file, "tsconfig.json");
                assert!(args.explain);
                assert!(args.no_catalog);
                assert_eq!(args.cache_dir.as_deref(), Some("/tmp/cache"));
                assert_eq!(args.schema_cache_ttl.as_deref(), Some("30m"));
            }
            _ => panic!("expected Identify"),
        }
        Ok(())
    }
}
