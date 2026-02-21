use std::process::ExitCode;

use bpaf::Bpaf;
use lintel_cli_common::CLIGlobalOptions;
use tracing_subscriber::prelude::*;

use lintel_annotate::annotate_args;
use lintel_reporters::{ReporterKind, ValidateArgs, make_reporter, validate_args};

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
pub struct ConvertArgs {
    /// Output format
    #[bpaf(long("to"), argument("FORMAT"))]
    pub to: OutputFormat,

    /// Input file to convert
    #[bpaf(positional("FILE"))]
    pub file: String,
}

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version, fallback_to_usage, generate(cli))]
#[allow(clippy::upper_case_acronyms)]
/// Validate JSON and YAML files against JSON Schema
struct CLI {
    #[bpaf(external(commands))]
    command: Commands,
}

#[derive(Debug, Clone, Bpaf)]
enum Commands {
    #[bpaf(command("check"))]
    /// Validate files against their schemas
    Check(
        #[bpaf(external(lintel_cli_common::cli_global_options), hide_usage)] CLIGlobalOptions,
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
        #[bpaf(external(lintel_cli_common::cli_global_options), hide_usage)] CLIGlobalOptions,
        /// Output format
        #[bpaf(
            long("reporter"),
            argument("pretty|text|github"),
            fallback(ReporterKind::Text)
        )]
        ReporterKind,
        #[bpaf(external(validate_args))] ValidateArgs,
    ),

    #[bpaf(command("init"))]
    /// Create a lintel.toml configuration file
    Init(#[bpaf(external(lintel_cli_common::cli_global_options), hide_usage)] CLIGlobalOptions),

    #[bpaf(command("convert"))]
    /// Convert between JSON, YAML, and TOML formats
    Convert(
        #[bpaf(external(lintel_cli_common::cli_global_options), hide_usage)] CLIGlobalOptions,
        #[bpaf(external(convert_args))] ConvertArgs,
    ),

    #[bpaf(command("annotate"))]
    /// Add schema annotations to files
    Annotate(
        #[bpaf(external(lintel_cli_common::cli_global_options), hide_usage)] CLIGlobalOptions,
        #[bpaf(external(annotate_args))] lintel_annotate::AnnotateArgs,
    ),

    #[bpaf(command("version"))]
    /// Print version information
    Version,
}

/// Set up tracing from CLI `--log-level` flag, falling back to `LINTEL_LOG` env.
fn setup_tracing(global: &CLIGlobalOptions) {
    let filter = match global.log_level {
        lintel_cli_common::LogLevel::None => {
            // Fall back to LINTEL_LOG env var
            match tracing_subscriber::EnvFilter::try_from_env("LINTEL_LOG") {
                Ok(f) => f,
                Err(_) => return,
            }
        }
        lintel_cli_common::LogLevel::Debug => tracing_subscriber::EnvFilter::new("debug"),
        lintel_cli_common::LogLevel::Info => tracing_subscriber::EnvFilter::new("info"),
        lintel_cli_common::LogLevel::Warn => tracing_subscriber::EnvFilter::new("warn"),
        lintel_cli_common::LogLevel::Error => tracing_subscriber::EnvFilter::new("error"),
    };

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

/// Set up miette error handler with colors config.
fn setup_miette(global: &CLIGlobalOptions) {
    let color = match global.colors {
        Some(lintel_cli_common::ColorsArg::Off) => miette::GraphicalTheme::none(),
        Some(lintel_cli_common::ColorsArg::Force) => miette::GraphicalTheme::unicode(),
        None => {
            if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
                miette::GraphicalTheme::unicode()
            } else {
                miette::GraphicalTheme::unicode_nocolor()
            }
        }
    };

    miette::set_hook(Box::new(move |_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .terminal_links(true)
                .context_lines(2)
                .graphical_theme(color.clone())
                .build(),
        )
    }))
    .ok();
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = cli().run();

    let result = match cli.command {
        Commands::Check(global, reporter_kind, mut args)
        | Commands::CI(global, reporter_kind, mut args) => {
            setup_tracing(&global);
            setup_miette(&global);
            let mut reporter = make_reporter(reporter_kind, global.verbose);
            lintel_reporters::run(
                &mut args,
                lintel_check::retriever::ReqwestClient::default(),
                reporter.as_mut(),
            )
            .await
        }
        Commands::Annotate(global, args) => {
            setup_tracing(&global);
            setup_miette(&global);
            commands::annotate::run(
                &args,
                lintel_check::retriever::ReqwestClient::default(),
                global.verbose,
            )
            .await
        }
        Commands::Init(_global) => match commands::init::run() {
            Ok(()) => return ExitCode::SUCCESS,
            Err(e) => Err(e),
        },
        Commands::Convert(_global, args) => match commands::convert::run(&args) {
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
            Commands::Check(global, _, args) => {
                assert!(global.verbose);
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
            Commands::Check(global, _, _) => {
                assert!(global.verbose);
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
    fn cli_check_with_log_level() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["check", "--log-level", "debug"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Check(global, _, _) => {
                assert_eq!(global.log_level, lintel_cli_common::LogLevel::Debug);
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_check_with_colors_off() -> anyhow::Result<()> {
        let parsed = cli()
            .run_inner(&["check", "--colors", "off"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        match parsed.command {
            Commands::Check(global, _, _) => {
                assert_eq!(global.colors, Some(lintel_cli_common::ColorsArg::Off));
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }
}
