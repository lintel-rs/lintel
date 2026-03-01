#![doc = include_str!("../README.md")]
#![allow(unused_assignments)] // thiserror/miette derive macros trigger false positives

extern crate alloc;

mod toml;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bpaf::{Bpaf, ShellComp};
use lintel_diagnostics::LintelDiagnostic;

// ---------------------------------------------------------------------------
// Format detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormatKind {
    Json,
    Jsonc,
    Toml,
    Yaml,
    Markdown,
}

fn detect_format(path: &Path) -> Option<FormatKind> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("json") => Some(FormatKind::Json),
        Some("jsonc") => Some(FormatKind::Jsonc),
        Some("yaml" | "yml") => Some(FormatKind::Yaml),
        Some("toml") => Some(FormatKind::Toml),
        Some("md" | "mdx") => Some(FormatKind::Markdown),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// dprint configuration (constructed once, reused)
// ---------------------------------------------------------------------------

/// Pre-built native configs for all formatters.
pub struct FormatConfig {
    json: dprint_plugin_json::configuration::Configuration,
    toml: dprint_plugin_toml::configuration::Configuration,
    markdown: dprint_plugin_markdown::configuration::Configuration,
    yaml: pretty_yaml::config::FormatOptions,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            json: dprint_plugin_json::configuration::ConfigurationBuilder::new().build(),
            toml: dprint_plugin_toml::configuration::ConfigurationBuilder::new().build(),
            markdown: dprint_plugin_markdown::configuration::ConfigurationBuilder::new().build(),
            yaml: pretty_yaml::config::FormatOptions::default(),
        }
    }
}

impl FormatConfig {
    /// Build formatter configs from a `DprintConfig`.
    fn from_dprint(dprint: &dprint_config::DprintConfig) -> Self {
        let global = build_global_config(dprint);

        let json = {
            let map = dprint
                .json
                .as_ref()
                .and_then(|j| serde_json::to_value(j).ok())
                .map(|v| json_value_to_config_key_map(&v))
                .unwrap_or_default();
            dprint_plugin_json::configuration::resolve_config(map, &global).config
        };

        let toml = {
            let map = dprint
                .toml
                .as_ref()
                .and_then(|t| serde_json::to_value(t).ok())
                .map(|v| json_value_to_config_key_map(&v))
                .unwrap_or_default();
            dprint_plugin_toml::configuration::resolve_config(map, &global).config
        };

        let markdown = {
            let map = dprint
                .markdown
                .as_ref()
                .and_then(|m| serde_json::to_value(m).ok())
                .map(|v| json_value_to_config_key_map(&v))
                .unwrap_or_default();
            dprint_plugin_markdown::configuration::resolve_config(map, &global).config
        };

        let yaml = {
            let mut opts = pretty_yaml::config::FormatOptions::default();
            if let Some(w) = dprint.line_width {
                opts.layout.print_width = w as usize;
            }
            if let Some(w) = dprint.indent_width {
                opts.layout.indent_width = w as usize;
            }
            opts
        };

        Self {
            json,
            toml,
            markdown,
            yaml,
        }
    }
}

/// Build a `GlobalConfiguration` from `DprintConfig` global fields.
fn build_global_config(
    dprint: &dprint_config::DprintConfig,
) -> dprint_core::configuration::GlobalConfiguration {
    use dprint_core::configuration::GlobalConfiguration;

    GlobalConfiguration {
        line_width: dprint.line_width,
        use_tabs: dprint.use_tabs,
        indent_width: dprint.indent_width.and_then(|v| u8::try_from(v).ok()),
        new_line_kind: dprint.new_line_kind.map(|nk| match nk {
            dprint_config::NewLineKind::Crlf => {
                dprint_core::configuration::NewLineKind::CarriageReturnLineFeed
            }
            dprint_config::NewLineKind::Lf => dprint_core::configuration::NewLineKind::LineFeed,
            // dprint-core doesn't have a System variant; map to Auto
            dprint_config::NewLineKind::Auto | dprint_config::NewLineKind::System => {
                dprint_core::configuration::NewLineKind::Auto
            }
        }),
    }
}

/// Convert a `serde_json::Value` object into dprint's `ConfigKeyMap`.
///
/// Only top-level string, number (i32), and bool values are converted;
/// nested objects and arrays are skipped since dprint plugins don't use them
/// for their config keys (except `jsonTrailingCommaFiles` which we handle
/// specially).
fn json_value_to_config_key_map(
    value: &serde_json::Value,
) -> dprint_core::configuration::ConfigKeyMap {
    use dprint_core::configuration::{ConfigKeyMap, ConfigKeyValue};

    let Some(obj) = value.as_object() else {
        return ConfigKeyMap::new();
    };

    let mut map = ConfigKeyMap::new();
    for (key, val) in obj {
        let ckv = match val {
            serde_json::Value::String(s) => ConfigKeyValue::from_str(s),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    ConfigKeyValue::from_i32(i32::try_from(i).unwrap_or(i32::MAX))
                } else {
                    continue;
                }
            }
            serde_json::Value::Bool(b) => ConfigKeyValue::from_bool(*b),
            serde_json::Value::Array(arr) => {
                let items: Vec<ConfigKeyValue> = arr
                    .iter()
                    .filter_map(|v| match v {
                        serde_json::Value::String(s) => Some(ConfigKeyValue::from_str(s)),
                        _ => None,
                    })
                    .collect();
                ConfigKeyValue::Array(items)
            }
            _ => continue,
        };
        map.insert(key.clone(), ckv);
    }
    map
}

// ---------------------------------------------------------------------------
// Core formatting
// ---------------------------------------------------------------------------

/// Format a single file's content. Returns `Ok(Some(formatted))` if the content
/// changed, `Ok(None)` if already formatted, or `Err` on parse failure.
///
/// # Errors
///
/// Returns an error if the file content cannot be parsed.
pub fn format_content(path: &Path, content: &str, cfg: &FormatConfig) -> Result<Option<String>> {
    let Some(kind) = detect_format(path) else {
        return Ok(None);
    };

    match kind {
        FormatKind::Json | FormatKind::Jsonc => {
            dprint_plugin_json::format_text(path, content, &cfg.json)
                .map_err(|e| anyhow::anyhow!("{e}"))
        }
        FormatKind::Toml => toml::format_text(path, content, &cfg.toml),
        FormatKind::Yaml => match pretty_yaml::format_text(content, &cfg.yaml) {
            Ok(formatted) => {
                if formatted == content {
                    Ok(None)
                } else {
                    Ok(Some(formatted))
                }
            }
            Err(e) => Err(anyhow::anyhow!("YAML syntax error: {e}")),
        },
        FormatKind::Markdown => {
            dprint_plugin_markdown::format_text(content, &cfg.markdown, |tag, text, _line_width| {
                match tag {
                    "json" => {
                        dprint_plugin_json::format_text(Path::new("code.json"), text, &cfg.json)
                    }
                    "jsonc" => {
                        dprint_plugin_json::format_text(Path::new("code.jsonc"), text, &cfg.json)
                    }
                    "toml" => {
                        dprint_plugin_toml::format_text(Path::new("code.toml"), text, &cfg.toml)
                    }
                    "yaml" | "yml" => match pretty_yaml::format_text(text, &cfg.yaml) {
                        Ok(formatted) if formatted == text => Ok(None),
                        Ok(formatted) => Ok(Some(formatted)),
                        Err(_) => Ok(None),
                    },
                    _ => Ok(None),
                }
            })
            .map_err(|e| anyhow::anyhow!("{e}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

fn plural(n: usize) -> &'static str {
    if n == 1 { "line" } else { "lines" }
}

fn diff_summary(added: usize, removed: usize, color: bool) -> String {
    use ansi_term_styles::{BOLD, DIM, RESET};

    if added == 0 && removed == 0 {
        return String::new();
    }

    let n = |count: usize| {
        if color {
            format!("{BOLD}{count}{RESET}{DIM}")
        } else {
            count.to_string()
        }
    };

    let text = if added == removed {
        format!("Changed {} {}", n(added), plural(added))
    } else if added > 0 && removed > 0 {
        format!(
            "Added {} {}, removed {} {}",
            n(added),
            plural(added),
            n(removed),
            plural(removed)
        )
    } else if added > 0 {
        format!("Added {} {}", n(added), plural(added))
    } else {
        format!("Removed {} {}", n(removed), plural(removed))
    };

    if color {
        format!("{DIM}{text}{RESET}")
    } else {
        text
    }
}

/// Generate a diff between original and formatted content with line numbers.
///
/// When `color` is true, applies delta-inspired ANSI coloring with
/// dark backgrounds for changed lines. Includes a summary header
/// ("Added N lines, removed M lines") and per-line numbers.
fn generate_diff(original: &str, formatted: &str, color: bool) -> String {
    use core::fmt::Write;

    use similar::ChangeTag;

    const DEL: &str = "\x1b[31m"; // red foreground
    const ADD: &str = "\x1b[32m"; // green foreground
    const DIM: &str = ansi_term_styles::DIM;
    const RESET: &str = ansi_term_styles::RESET;

    let diff = similar::TextDiff::from_lines(original, formatted);

    // Count additions/deletions across all changes
    let mut added = 0usize;
    let mut removed = 0usize;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => {}
        }
    }

    // Max line number for column width
    let max_line = original.lines().count().max(formatted.lines().count());
    let width = max_line.to_string().len();

    let mut out = String::with_capacity(original.len() + formatted.len());

    // Summary header
    let _ = writeln!(out, "{}", diff_summary(added, removed, color));

    // Use grouped ops (3 lines of context) to show only relevant hunks
    let mut first_group = true;
    for group in diff.grouped_ops(3) {
        if !first_group {
            if color {
                let _ = writeln!(out, "{DIM}  ...{RESET}");
            } else {
                let _ = writeln!(out, "  ...");
            }
        }
        first_group = false;

        for op in &group {
            for change in diff.iter_changes(op) {
                let value = change.value().trim_end_matches('\n');
                match change.tag() {
                    ChangeTag::Delete => {
                        let lineno = change.old_index().map_or(0, |n| n + 1);
                        if color {
                            let _ = writeln!(out, "{DEL}{lineno:>width$} - {value}{RESET}");
                        } else {
                            let _ = writeln!(out, "{lineno:>width$} - {value}");
                        }
                    }
                    ChangeTag::Insert => {
                        let lineno = change.new_index().map_or(0, |n| n + 1);
                        if color {
                            let _ = writeln!(out, "{ADD}{lineno:>width$} + {value}{RESET}");
                        } else {
                            let _ = writeln!(out, "{lineno:>width$} + {value}");
                        }
                    }
                    ChangeTag::Equal => {
                        let lineno = change.old_index().map_or(0, |n| n + 1);
                        let _ = writeln!(out, "{lineno:>width$}   {value}");
                    }
                }
            }
        }
    }
    out
}

fn make_diagnostic(path_str: String, content: &str, formatted: &str) -> LintelDiagnostic {
    let color = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let styled_path = if color {
        format!("\x1b[1;4;36m{path_str}\x1b[0m")
    } else {
        path_str.clone()
    };
    LintelDiagnostic::Format {
        path: path_str,
        styled_path,
        diff: generate_diff(content, formatted, color),
    }
}

// ---------------------------------------------------------------------------
// File discovery (lightweight, independent of lintel-validate)
// ---------------------------------------------------------------------------

fn discover_files(root: &str, excludes: &[String]) -> Result<Vec<PathBuf>> {
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    let mut files = Vec::new();
    for entry in walker {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if detect_format(path).is_none() {
            continue;
        }
        if is_excluded(path, excludes) {
            continue;
        }
        files.push(path.to_path_buf());
    }

    files.sort();
    Ok(files)
}

fn collect_files(globs: &[String], exclude: &[String]) -> Result<Vec<PathBuf>> {
    if globs.is_empty() {
        return discover_files(".", exclude);
    }

    let mut result = Vec::new();
    for pattern in globs {
        let path = Path::new(pattern);
        if path.is_dir() {
            result.extend(discover_files(pattern, exclude)?);
        } else {
            for entry in
                glob::glob(pattern).with_context(|| format!("invalid glob pattern: {pattern}"))?
            {
                let path = entry?;
                if path.is_file() && !is_excluded(&path, exclude) {
                    result.push(path);
                }
            }
        }
    }
    result.sort();
    result.dedup();
    Ok(result)
}

fn is_excluded(path: &Path, excludes: &[String]) -> bool {
    let path_str = match path.to_str() {
        Some(s) => s.strip_prefix("./").unwrap_or(s),
        None => return false,
    };
    excludes
        .iter()
        .any(|pattern| glob_match::glob_match(pattern, path_str))
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

struct LoadedConfig {
    excludes: Vec<String>,
    format: FormatConfig,
}

fn load_config(globs: &[String], user_excludes: &[String]) -> LoadedConfig {
    let search_dir = globs
        .iter()
        .find(|g| Path::new(g).is_dir())
        .map(PathBuf::from);

    let cfg_result = match &search_dir {
        Some(dir) => lintel_config::find_and_load(dir).map(Option::unwrap_or_default),
        None => lintel_config::load(),
    };

    match cfg_result {
        Ok(cfg) => {
            let format = cfg
                .format
                .as_ref()
                .and_then(|f| f.dprint.as_ref())
                .map(FormatConfig::from_dprint)
                .unwrap_or_default();

            let mut excludes = cfg.exclude;
            excludes.extend(user_excludes.iter().cloned());

            LoadedConfig { excludes, format }
        }
        Err(e) => {
            eprintln!("warning: failed to load lintel.toml: {e}");
            LoadedConfig {
                excludes: user_excludes.to_vec(),
                format: FormatConfig::default(),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Bpaf)]
#[bpaf(generate(format_args_inner))]
pub struct FormatArgs {
    /// Check formatting without writing changes
    #[bpaf(long("check"), switch)]
    pub check: bool,

    #[bpaf(long("exclude"), argument("PATTERN"))]
    pub exclude: Vec<String>,

    #[bpaf(positional("PATH"), complete_shell(ShellComp::File { mask: None }))]
    pub globs: Vec<String>,
}

/// Construct the bpaf parser for `FormatArgs`.
pub fn format_args() -> impl bpaf::Parser<FormatArgs> {
    format_args_inner()
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

pub struct FormatResult {
    /// Files that were formatted (written in place).
    pub formatted: Vec<String>,
    /// Files that were already formatted.
    pub unchanged: usize,
    /// Files skipped (unsupported format).
    pub skipped: usize,
    /// Errors encountered during formatting.
    pub errors: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Check formatting of pre-read file contents, returning diagnostics for unformatted files.
///
/// Loads `lintel.toml` for dprint configuration. Files with unrecognized formats
/// or parse failures are silently skipped.
pub fn check_format_contents(
    file_contents: &[(PathBuf, String)],
    globs: &[String],
    user_excludes: &[String],
) -> Vec<LintelDiagnostic> {
    let loaded = load_config(globs, user_excludes);

    let mut diagnostics = Vec::new();
    for (file_path, content) in file_contents {
        if is_excluded(file_path, &loaded.excludes) {
            continue;
        }
        if detect_format(file_path).is_none() {
            continue;
        }
        if let Ok(Some(formatted)) = format_content(file_path, content, &loaded.format) {
            let path_str = file_path.display().to_string();
            diagnostics.push(make_diagnostic(path_str, content, &formatted));
        }
    }

    diagnostics
}

/// Check formatting of files, returning diagnostics for unformatted files.
///
/// Loads `lintel.toml` and merges exclude patterns. Files that fail to parse
/// are silently skipped (they will be caught by schema validation).
///
/// # Errors
///
/// Returns an error if file discovery fails (e.g. invalid glob pattern or I/O error).
pub fn check_format(globs: &[String], user_excludes: &[String]) -> Result<Vec<LintelDiagnostic>> {
    let loaded = load_config(globs, user_excludes);
    let files = collect_files(globs, &loaded.excludes)?;

    let mut diagnostics = Vec::new();
    for file_path in &files {
        let Ok(content) = fs::read_to_string(file_path) else {
            continue;
        };

        if let Ok(Some(formatted)) = format_content(file_path, &content, &loaded.format) {
            let path_str = file_path.display().to_string();
            diagnostics.push(make_diagnostic(path_str, &content, &formatted));
        }
    }

    Ok(diagnostics)
}

/// Fix formatting of files in place.
///
/// Loads `lintel.toml` and merges exclude patterns. Returns the number of
/// files that were reformatted.
///
/// # Errors
///
/// Returns an error if file discovery fails (e.g. invalid glob pattern or I/O error).
pub fn fix_format(globs: &[String], user_excludes: &[String]) -> Result<usize> {
    let loaded = load_config(globs, user_excludes);
    let files = collect_files(globs, &loaded.excludes)?;

    let mut fixed = 0;
    for file_path in &files {
        let Ok(content) = fs::read_to_string(file_path) else {
            continue;
        };

        if let Ok(Some(formatted)) = format_content(file_path, &content, &loaded.format) {
            fs::write(file_path, formatted)?;
            fixed += 1;
        }
    }

    Ok(fixed)
}

/// Run the format command: format files in place, or check with `--check`.
///
/// Returns `Ok(FormatResult)` on success. In `--check` mode, unformatted
/// files are reported as errors (diffs printed to stderr by the caller).
///
/// # Errors
///
/// Returns an error if file discovery fails (e.g. invalid glob pattern or I/O error).
pub fn run(args: &FormatArgs) -> Result<FormatResult> {
    let loaded = load_config(&args.globs, &args.exclude);
    let files = collect_files(&args.globs, &loaded.excludes)?;

    let mut result = FormatResult {
        formatted: Vec::new(),
        unchanged: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    for file_path in &files {
        let path_str = file_path.display().to_string();

        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                result
                    .errors
                    .push((path_str, format!("failed to read: {e}")));
                continue;
            }
        };

        match format_content(file_path, &content, &loaded.format) {
            Ok(Some(formatted)) => {
                if args.check {
                    let diag = make_diagnostic(path_str.clone(), &content, &formatted);
                    eprintln!("{:?}", miette::Report::new(diag));
                    result.errors.push((path_str, "not formatted".to_string()));
                } else {
                    match fs::write(file_path, &formatted) {
                        Ok(()) => result.formatted.push(path_str),
                        Err(e) => {
                            result
                                .errors
                                .push((path_str, format!("failed to write: {e}")));
                        }
                    }
                }
            }
            Ok(None) => result.unchanged += 1,
            Err(_) => result.skipped += 1,
        }
    }

    Ok(result)
}
