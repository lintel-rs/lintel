#![doc = include_str!("../README.md")]
#![allow(unused_assignments)] // thiserror/miette derive macros trigger false positives

mod cargo_toml;
mod commands;
mod doc_injection;
mod readme;
mod workspace;

use std::path::{Path, PathBuf};

use bpaf::Bpaf;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version, fallback_to_usage, generate(cli))]
/// Furnish crates with standardized Cargo.toml metadata, READMEs, and doc attributes.
struct Cli {
    #[bpaf(external(commands))]
    command: Commands,
}

#[derive(Debug, Clone, Bpaf)]
enum Commands {
    #[bpaf(command("check"))]
    /// Check crates for metadata issues
    Check {
        /// Auto-fix issues that don't require user input
        #[bpaf(long("fix"), switch)]
        fix: bool,
        /// Crate directory or name (omit for all workspace members)
        #[bpaf(positional("CRATE"))]
        target: Option<String>,
    },

    #[bpaf(command("update"))]
    /// Update crate metadata, README, and doc attributes
    Update {
        #[bpaf(external(commands::update::update_args))]
        args: commands::update::UpdateArgs,
        /// Crate directory or name
        #[bpaf(positional("CRATE"))]
        target: String,
    },

    #[bpaf(command("man"), hide)]
    /// Generate man page in roff format
    Man,
}

fn main() -> miette::Result<()> {
    setup_miette();
    let opts = cli().run();

    if matches!(opts.command, Commands::Man) {
        let roff = cli().render_manpage(
            "cargo-furnish",
            bpaf::doc::Section::General,
            None,
            None,
            Some("Cargo Furnish Manual"),
        );
        print!("{roff}");
        return Ok(());
    }

    let cwd = std::env::current_dir()
        .map_err(|e| miette::miette!("failed to get current directory: {e}"))?;
    let workspace_root = find_workspace_root(&cwd)
        .ok_or_else(|| miette::miette!("could not find workspace root from {}", cwd.display()))?;

    let ws = workspace::parse_workspace(&workspace_root).map_err(|e| miette::miette!("{e:#}"))?;

    match opts.command {
        Commands::Check { fix, target } => {
            let crate_dirs = resolve_and_relativize(target.as_ref(), &workspace_root, &cwd)?;
            if fix {
                commands::check::run_fix(&crate_dirs, &ws);
            } else {
                commands::check::run(&crate_dirs, &ws);
            }
        }
        Commands::Update { args, target } => {
            let crate_dirs = resolve_and_relativize(Some(&target), &workspace_root, &cwd)?;
            commands::update::run(&crate_dirs, &ws, args);
        }
        Commands::Man => unreachable!(),
    }

    Ok(())
}

fn resolve_and_relativize(
    target: Option<&String>,
    workspace_root: &Path,
    cwd: &Path,
) -> miette::Result<Vec<PathBuf>> {
    Ok(resolve_targets(target, workspace_root)?
        .into_iter()
        .map(|d| d.strip_prefix(cwd).map(Path::to_path_buf).unwrap_or(d))
        .collect())
}

/// Resolve the target argument into one or more crate directories.
fn resolve_targets(target: Option<&String>, workspace_root: &Path) -> miette::Result<Vec<PathBuf>> {
    match target {
        None => {
            workspace::resolve_member_dirs(workspace_root).map_err(|e| miette::miette!("{e:#}"))
        }
        Some(arg) => {
            let path = PathBuf::from(arg);
            if path.join("Cargo.toml").exists() {
                let canonical = path
                    .canonicalize()
                    .map_err(|e| miette::miette!("invalid path '{}': {e}", path.display()))?;
                Ok(vec![canonical])
            } else {
                let found = workspace::find_member_by_name(workspace_root, arg)
                    .map_err(|e| miette::miette!("{e:#}"))?;
                match found {
                    Some(dir) => Ok(vec![dir]),
                    None => Err(miette::miette!(
                        "'{arg}' is not a crate directory or workspace member name"
                    )),
                }
            }
        }
    }
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

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists()
            && let Ok(content) = std::fs::read_to_string(&candidate)
            && let Ok(doc) = content.parse::<toml_edit::DocumentMut>()
            && doc.get("workspace").is_some()
        {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}
