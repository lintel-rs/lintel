extern crate alloc;

mod commands;
mod config;
mod metadata;

use std::path::PathBuf;

use bpaf::Bpaf;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version, fallback_to_usage, generate(cli))]
/// Generate and publish platform-specific npm packages from Rust binaries.
struct Cli {
    /// Path to config file
    #[bpaf(
        long("config"),
        argument("FILE"),
        fallback(PathBuf::from("npm-release-binaries.toml"))
    )]
    config_path: PathBuf,

    #[bpaf(external(command))]
    command: Command,
}

#[derive(Debug, Clone, Bpaf)]
enum Command {
    #[bpaf(command("release"))]
    /// Generate and publish npm packages (generate + publish)
    Release {
        /// Package key from config to release (e.g. lintel)
        #[bpaf(long("package"), argument("NAME"))]
        package: String,
        /// Release version (e.g. 0.0.7)
        #[bpaf(long("release-version"), argument("VERSION"))]
        version: String,
        /// Skip copying binaries from archives (generate metadata only)
        #[bpaf(long("skip-artifact-copy"), switch)]
        skip_artifact_copy: bool,
    },

    #[bpaf(command("generate"))]
    /// Generate npm packages without publishing
    Generate {
        /// Package key from config to generate (e.g. lintel)
        #[bpaf(long("package"), argument("NAME"))]
        package: String,
        /// Release version (e.g. 0.0.7)
        #[bpaf(long("release-version"), argument("VERSION"))]
        version: String,
        /// Skip copying binaries from archives (generate metadata only)
        #[bpaf(long("skip-artifact-copy"), switch)]
        skip_artifact_copy: bool,
    },

    #[bpaf(command("publish"))]
    /// Publish previously generated npm packages
    Publish {
        /// Package key from config to publish (e.g. lintel)
        #[bpaf(long("package"), argument("NAME"))]
        package: String,
        /// Perform a dry run (don't actually publish)
        #[bpaf(long("dry-run"), switch)]
        dry_run: bool,
    },
}

fn main() -> miette::Result<()> {
    setup_miette();
    let cli = cli().run();

    let config = config::load(&cli.config_path)?;
    let output_dir = resolve_output_dir(&config);

    match cli.command {
        Command::Release {
            package,
            version,
            skip_artifact_copy,
        } => {
            let pkg_config = resolve_package(&config, &package)?;
            commands::release::run(
                &package,
                pkg_config,
                &version,
                config.artifacts_dir.as_deref(),
                &output_dir,
                skip_artifact_copy,
            )?;
        }
        Command::Generate {
            package,
            version,
            skip_artifact_copy,
        } => {
            let pkg_config = resolve_package(&config, &package)?;
            commands::generate::run(
                &package,
                pkg_config,
                &version,
                config.artifacts_dir.as_deref(),
                &output_dir,
                skip_artifact_copy,
            )?;
        }
        Command::Publish { package, dry_run } => {
            let pkg_config = resolve_package(&config, &package)?;
            commands::publish::run(pkg_config, &output_dir, dry_run)?;
        }
    }

    Ok(())
}

fn resolve_package<'a>(
    config: &'a config::Config,
    key: &str,
) -> miette::Result<&'a config::PackageConfig> {
    config.packages.get(key).ok_or_else(|| {
        miette::miette!(
            "package '{key}' not found in config; available: {}",
            config
                .packages
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

fn resolve_output_dir(config: &config::Config) -> PathBuf {
    config
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("npm-publish"))
}

fn setup_miette() {
    let theme = if std::io::IsTerminal::is_terminal(&std::io::stderr()) {
        miette::GraphicalTheme::unicode()
    } else {
        miette::GraphicalTheme::unicode_nocolor()
    };
    miette::set_hook(Box::new(move |_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .terminal_links(true)
                .context_lines(2)
                .graphical_theme(theme.clone())
                .build(),
        )
    }))
    .ok();
}
