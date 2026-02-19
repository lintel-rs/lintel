use std::process::ExitCode;

use bpaf::Bpaf;

mod commands;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Json,
    Json5,
    Jsonc,
    Toml,
    Yaml,
    Markdown,
}

impl core::str::FromStr for FileFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" => Ok(Self::Json),
            "json5" => Ok(Self::Json5),
            "jsonc" => Ok(Self::Jsonc),
            "toml" => Ok(Self::Toml),
            "yaml" => Ok(Self::Yaml),
            "markdown" | "md" => Ok(Self::Markdown),
            _ => Err(format!(
                "unknown format '{s}', expected: json, json5, jsonc, toml, yaml, markdown"
            )),
        }
    }
}

impl From<FileFormat> for lintel_check::parsers::FileFormat {
    fn from(f: FileFormat) -> Self {
        match f {
            FileFormat::Json => lintel_check::parsers::FileFormat::Json,
            FileFormat::Json5 => lintel_check::parsers::FileFormat::Json5,
            FileFormat::Jsonc => lintel_check::parsers::FileFormat::Jsonc,
            FileFormat::Toml => lintel_check::parsers::FileFormat::Toml,
            FileFormat::Yaml => lintel_check::parsers::FileFormat::Yaml,
            FileFormat::Markdown => lintel_check::parsers::FileFormat::Markdown,
        }
    }
}

#[derive(Debug, Clone, Bpaf)]
pub struct ValidateArgs {
    #[bpaf(long("exclude"), argument("PATTERN"))]
    pub exclude: Vec<String>,

    #[bpaf(long("cache-dir"), argument("DIR"))]
    pub cache_dir: Option<String>,

    #[bpaf(long("no-cache"), switch)]
    pub no_cache: bool,

    #[bpaf(long("no-catalog"), switch)]
    pub no_catalog: bool,

    #[bpaf(long("format"), argument("FORMAT"))]
    pub format: Option<FileFormat>,

    #[bpaf(positional("PATH"))]
    pub globs: Vec<String>,
}

impl From<&ValidateArgs> for lintel_check::validate::ValidateArgs {
    fn from(args: &ValidateArgs) -> Self {
        // When a single directory is passed as an arg, use it as the config
        // search directory so that `lintel.toml` inside that directory is found.
        let config_dir = args
            .globs
            .iter()
            .find(|g| std::path::Path::new(g).is_dir())
            .map(std::path::PathBuf::from);

        lintel_check::validate::ValidateArgs {
            globs: args.globs.clone(),
            exclude: args.exclude.clone(),
            cache_dir: args.cache_dir.clone(),
            no_cache: args.no_cache,
            no_catalog: args.no_catalog,
            format: args.format.map(Into::into),
            config_dir,
        }
    }
}

#[derive(Debug, Clone, Bpaf)]
pub struct CliOptions {
    /// Print additional diagnostics and show which files were checked
    #[bpaf(short('v'), long("verbose"), switch, fallback(false))]
    pub verbose: bool,
}

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version, fallback_to_usage)]
/// Validate JSON and YAML files against JSON Schema
struct Cli {
    #[bpaf(external(commands))]
    command: Commands,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

impl core::str::FromStr for Shell {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            "fish" => Ok(Self::Fish),
            "powershell" => Ok(Self::PowerShell),
            _ => Err(format!(
                "unknown shell '{s}', expected: bash, zsh, fish, powershell"
            )),
        }
    }
}

#[derive(Debug, Clone, Bpaf)]
enum Commands {
    #[bpaf(command("check"))]
    /// Validate files against their schemas
    Check(
        #[bpaf(external(cli_options), hide_usage)] CliOptions,
        #[bpaf(external(validate_args))] ValidateArgs,
    ),

    #[bpaf(command("ci"))]
    /// Validate files with CI-friendly output
    CI(
        #[bpaf(external(cli_options), hide_usage)] CliOptions,
        #[bpaf(external(validate_args))] ValidateArgs,
    ),

    #[bpaf(command("init"))]
    /// Create a lintel.toml configuration file
    Init,

    #[bpaf(command("convert"))]
    /// Convert between JSON, YAML, and TOML formats
    Convert(#[bpaf(external(convert_args))] ConvertArgs),

    #[bpaf(command("completions"))]
    /// Generate shell completions
    Completions(
        /// Shell to generate completions for
        #[bpaf(positional("SHELL"))]
        Shell,
    ),

    #[bpaf(command("version"))]
    /// Print version information
    Version,
}

#[tokio::main]
async fn main() -> ExitCode {
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
        Commands::Check(cli_options, mut args) => {
            commands::check::run(
                &mut args,
                lintel_check::retriever::UreqClient,
                cli_options.verbose,
            )
            .await
        }
        Commands::CI(cli_options, mut args) => {
            commands::ci::run(
                &mut args,
                lintel_check::retriever::UreqClient,
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
        Commands::Completions(shell) => {
            commands::completions::run(shell);
            return ExitCode::SUCCESS;
        }
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
    fn cli_parses_check_basic_args() -> Result<(), String> {
        let cli = cli()
            .run_inner(&["check", "*.json"])
            .map_err(|e| format!("{e:?}"))?;
        match cli.command {
            Commands::Check(_, args) => {
                assert_eq!(args.globs, vec!["*.json"]);
                assert!(args.exclude.is_empty());
                assert!(args.cache_dir.is_none());
                assert!(!args.no_cache);
                assert!(!args.no_catalog);
                assert!(args.format.is_none());
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_parses_check_all_options() -> Result<(), String> {
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
                "--no-cache",
                "--no-catalog",
                "--format",
                "jsonc",
            ])
            .map_err(|e| format!("{e:?}"))?;
        match cli.command {
            Commands::Check(_, args) => {
                assert_eq!(args.globs, vec!["*.json", "**/*.json"]);
                assert_eq!(args.exclude, vec!["node_modules/**", "vendor/**"]);
                assert_eq!(args.cache_dir.as_deref(), Some("/tmp/cache"));
                assert!(args.no_cache);
                assert!(args.no_catalog);
                assert_eq!(args.format, Some(FileFormat::Jsonc));
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_check_no_globs_is_valid() -> Result<(), String> {
        let cli = cli().run_inner(&["check"]).map_err(|e| format!("{e:?}"))?;
        match cli.command {
            Commands::Check(_, args) => {
                assert!(args.globs.is_empty());
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_parses_ci_subcommand() -> Result<(), String> {
        let cli = cli()
            .run_inner(&["ci", "*.json", "--no-catalog"])
            .map_err(|e| format!("{e:?}"))?;
        match cli.command {
            Commands::CI(_, args) => {
                assert_eq!(args.globs, vec!["*.json"]);
                assert!(args.no_catalog);
            }
            _ => panic!("expected CI"),
        }
        Ok(())
    }

    #[test]
    fn cli_verbose_short_after_subcommand() -> Result<(), String> {
        let parsed = cli()
            .run_inner(&["check", "-v", "*.json"])
            .map_err(|e| format!("{e:?}"))?;
        match parsed.command {
            Commands::Check(cli_options, args) => {
                assert!(cli_options.verbose);
                assert_eq!(args.globs, vec!["*.json"]);
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }

    #[test]
    fn cli_verbose_long_after_subcommand() -> Result<(), String> {
        let parsed = cli()
            .run_inner(&["check", "--verbose"])
            .map_err(|e| format!("{e:?}"))?;
        match parsed.command {
            Commands::Check(cli_options, _) => {
                assert!(cli_options.verbose);
            }
            _ => panic!("expected Check"),
        }
        Ok(())
    }
}
