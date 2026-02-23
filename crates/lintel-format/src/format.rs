use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use bpaf::Bpaf;

use lintel_cli_common::CLIGlobalOptions;

/// Arguments for the format command
#[derive(Debug, Clone, Bpaf)]
pub struct FormatArgs {
    /// Exclude files matching the given pattern
    #[bpaf(long("exclude"), argument("PATTERN"))]
    pub exclude: Vec<String>,

    /// Don't write formatted files, just check if they are formatted.
    /// Exit with code 1 if any files are unformatted.
    #[bpaf(long("check"), switch, fallback(false))]
    pub check: bool,

    /// Files or directories to format
    #[bpaf(positional("PATH"))]
    pub paths: Vec<String>,
}

/// Supported file format for formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormatKind {
    Json,
    Jsonc,
    Json5,
    Yaml,
    Markdown,
    Toml,
}

/// Detect format from file extension.
fn detect_format(path: &Path) -> Option<FormatKind> {
    let ext = path.extension()?.to_str()?;
    match ext {
        "json" => Some(FormatKind::Json),
        "jsonc" => Some(FormatKind::Jsonc),
        "json5" => Some(FormatKind::Json5),
        "yaml" | "yml" => Some(FormatKind::Yaml),
        "md" | "markdown" => Some(FormatKind::Markdown),
        "toml" => Some(FormatKind::Toml),
        _ => None,
    }
}

/// Format a single file. Returns the formatted content, or None if unsupported.
///
/// # Errors
///
/// Returns an error if the file cannot be read, parsed, or formatted.
fn format_single_file(path: &Path, content: &str, kind: FormatKind) -> Result<String> {
    if kind == FormatKind::Toml {
        return Ok(taplo::formatter::format(
            content,
            taplo::formatter::Options::default(),
        ));
    }
    let opts = prettier_rs::resolve_config(path)?;
    let format = match kind {
        FormatKind::Json => prettier_rs::Format::Json,
        FormatKind::Jsonc => prettier_rs::Format::Jsonc,
        FormatKind::Json5 => prettier_rs::Format::Json5,
        FormatKind::Yaml => prettier_rs::Format::Yaml,
        FormatKind::Markdown => prettier_rs::Format::Markdown,
        FormatKind::Toml => unreachable!(),
    };
    prettier_rs::format_str(content, format, &opts)
}

/// Collect files to format from the given paths.
fn collect_files(paths: &[String], excludes: &[String]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    let effective_paths = if paths.is_empty() {
        vec![".".to_string()]
    } else {
        paths.to_vec()
    };

    for path_str in &effective_paths {
        let path = Path::new(path_str);
        if path.is_file() {
            if detect_format(path).is_some() {
                files.push(path.to_path_buf());
            }
        } else if path.is_dir() {
            walk_directory(path, excludes, &mut files)?;
        } else {
            // Try glob expansion
            let matches = glob::glob(path_str)
                .with_context(|| format!("invalid glob pattern: {path_str}"))?;
            for entry in matches {
                let entry = entry?;
                if entry.is_file() && detect_format(&entry).is_some() {
                    files.push(entry);
                }
            }
        }
    }

    // Apply excludes
    if !excludes.is_empty() {
        files.retain(|f| {
            let f_str = f.to_string_lossy();
            !excludes
                .iter()
                .any(|pattern| glob_match::glob_match(pattern, &f_str))
        });
    }

    Ok(files)
}

/// Walk a directory collecting formattable files, respecting .gitignore.
fn walk_directory(dir: &Path, _excludes: &[String], files: &mut Vec<PathBuf>) -> Result<()> {
    let walker = ignore::WalkBuilder::new(dir)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker {
        let entry = entry?;
        if entry.file_type().is_some_and(|ft| ft.is_file()) {
            let path = entry.path();
            if detect_format(path).is_some() {
                files.push(path.to_path_buf());
            }
        }
    }

    Ok(())
}

fn tool_name(kind: FormatKind) -> &'static str {
    match kind {
        FormatKind::Json
        | FormatKind::Jsonc
        | FormatKind::Json5
        | FormatKind::Yaml
        | FormatKind::Markdown => "prettier",
        FormatKind::Toml => "taplo",
    }
}

/// Run the format command.
///
/// Returns `true` if any files were unformatted (for `--check` mode exit code).
///
/// # Errors
///
/// Returns an error if file collection, reading, or formatting fails.
pub fn run(args: &FormatArgs, global: &CLIGlobalOptions) -> Result<bool> {
    let files = collect_files(&args.paths, &args.exclude)?;

    if files.is_empty() {
        if global.verbose {
            eprintln!("No formattable files found.");
        }
        return Ok(false);
    }

    let mut any_unformatted = false;
    let mut formatted_count: usize = 0;
    let mut error_count: usize = 0;

    for file in &files {
        let Some(kind) = detect_format(file) else {
            continue;
        };

        let start = Instant::now();
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {}: {e}", file.display());
                error_count += 1;
                continue;
            }
        };

        match format_single_file(file, &content, kind) {
            Ok(formatted) => {
                let elapsed = start.elapsed();
                let changed = formatted != content;

                if args.check {
                    if changed {
                        eprintln!("  {} (unformatted)", file.display());
                        any_unformatted = true;
                    } else if global.verbose {
                        eprintln!(
                            "  {} ({}, {}ms)",
                            file.display(),
                            tool_name(kind),
                            elapsed.as_millis()
                        );
                    }
                } else {
                    if changed {
                        std::fs::write(file, &formatted).with_context(|| {
                            format!("writing formatted output to {}", file.display())
                        })?;
                    }

                    if global.verbose {
                        let tag = if changed { " [changed]" } else { "" };
                        eprintln!(
                            "  {} ({}, {}ms){tag}",
                            file.display(),
                            tool_name(kind),
                            elapsed.as_millis()
                        );
                    }
                }

                formatted_count += 1;
            }
            Err(e) => {
                eprintln!("error: {}: {e}", file.display());
                error_count += 1;
            }
        }
    }

    if global.verbose || args.check {
        let status = if args.check {
            if any_unformatted {
                "Some files are not formatted."
            } else {
                "All files are formatted."
            }
        } else {
            "Done."
        };
        eprintln!("Formatted {formatted_count} files ({error_count} errors). {status}");
    }

    Ok(any_unformatted)
}
