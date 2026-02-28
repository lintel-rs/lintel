#![doc = include_str!("../README.md")]

use core::time::Duration;

use bpaf::{Bpaf, ShellComp};

/// Global options applied to all commands
#[derive(Debug, Clone, Bpaf)]
#[bpaf(generate(cli_global_options))]
#[allow(clippy::upper_case_acronyms)]
pub struct CLIGlobalOptions {
    /// Set the formatting mode for markup: "off" prints everything as plain text,
    /// "force" forces the formatting of markup using ANSI even if the console
    /// output is determined to be incompatible
    #[bpaf(long("colors"), argument("off|force"))]
    pub colors: Option<ColorsArg>,

    /// Print additional diagnostics, and some diagnostics show more information.
    /// Also, print out what files were processed and which ones were modified.
    #[bpaf(short('v'), long("verbose"), switch, fallback(false))]
    pub verbose: bool,

    /// The level of logging. In order, from the most verbose to the least verbose:
    /// debug, info, warn, error.
    #[bpaf(
        long("log-level"),
        argument("none|debug|info|warn|error"),
        fallback(LogLevel::None),
        display_fallback
    )]
    pub log_level: LogLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorsArg {
    Off,
    Force,
}

impl core::str::FromStr for ColorsArg {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "off" => Ok(Self::Off),
            "force" => Ok(Self::Force),
            _ => Err(format!("expected 'off' or 'force', got '{s}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogLevel {
    #[default]
    None,
    Debug,
    Info,
    Warn,
    Error,
}

impl core::str::FromStr for LogLevel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err(format!(
                "expected 'none', 'debug', 'info', 'warn', or 'error', got '{s}'"
            )),
        }
    }
}

impl core::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

// ---------------------------------------------------------------------------
// Shared cache options
// ---------------------------------------------------------------------------

#[allow(clippy::needless_pass_by_value)] // bpaf parse() requires owned String
fn parse_duration(s: String) -> Result<Duration, String> {
    humantime::parse_duration(&s).map_err(|e| format!("invalid duration '{s}': {e}"))
}

/// Cache-related CLI flags shared across commands.
#[derive(Debug, Clone, Bpaf)]
#[bpaf(generate(cli_cache_options))]
#[allow(clippy::struct_excessive_bools)]
pub struct CliCacheOptions {
    #[bpaf(long("cache-dir"), argument("DIR"), complete_shell(ShellComp::Dir { mask: None }))]
    pub cache_dir: Option<String>,

    /// Schema cache TTL (e.g. "12h", "30m", "1d"); default 12h
    #[bpaf(long("schema-cache-ttl"), argument::<String>("DURATION"), parse(parse_duration), optional)]
    pub schema_cache_ttl: Option<Duration>,

    /// Bypass schema cache reads (still writes fetched schemas to cache)
    #[bpaf(long("force-schema-fetch"), switch)]
    pub force_schema_fetch: bool,

    /// Bypass validation cache reads (still writes results to cache)
    #[bpaf(long("force-validation"), switch)]
    pub force_validation: bool,

    /// Bypass all cache reads (combines --force-schema-fetch and --force-validation)
    #[bpaf(long("force"), switch)]
    pub force: bool,

    #[bpaf(long("no-catalog"), switch)]
    pub no_catalog: bool,
}

// ---------------------------------------------------------------------------
// Pager
// ---------------------------------------------------------------------------

/// Pipe content through a pager (respects `$PAGER`, defaults to `less -R`).
///
/// Spawns the pager as a child process and writes `content` to its stdin.
/// Falls back to printing directly if the pager cannot be spawned.
pub fn pipe_to_pager(content: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let pager_env = std::env::var("PAGER").unwrap_or_default();
    let (program, args) = if pager_env.is_empty() {
        ("less", vec!["-R"])
    } else {
        let mut parts: Vec<&str> = pager_env.split_whitespace().collect();
        let prog = parts.remove(0);
        // Ensure less gets -R for ANSI color passthrough
        if prog == "less" && !parts.iter().any(|a| a.contains('R')) {
            parts.push("-R");
        }
        (prog, parts)
    };

    match Command::new(program)
        .args(&args)
        .stdin(Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            if let Some(mut stdin) = child.stdin.take() {
                // Ignore broken-pipe errors (user quit the pager early)
                let _ = write!(stdin, "{content}");
            }
            let _ = child.wait();
        }
        Err(_) => {
            // Pager unavailable -- print directly
            print!("{content}");
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bpaf::Parser;

    fn opts() -> bpaf::OptionParser<CLIGlobalOptions> {
        cli_global_options().to_options()
    }

    fn cache_opts() -> bpaf::OptionParser<CliCacheOptions> {
        cli_cache_options().to_options()
    }

    #[test]
    fn defaults() {
        let parsed = opts().run_inner(&[]).unwrap();
        assert!(!parsed.verbose);
        assert_eq!(parsed.log_level, LogLevel::None);
        assert!(parsed.colors.is_none());
    }

    #[test]
    fn verbose_short() {
        let parsed = opts().run_inner(&["-v"]).unwrap();
        assert!(parsed.verbose);
    }

    #[test]
    fn verbose_long() {
        let parsed = opts().run_inner(&["--verbose"]).unwrap();
        assert!(parsed.verbose);
    }

    #[test]
    fn log_level_debug() {
        let parsed = opts().run_inner(&["--log-level", "debug"]).unwrap();
        assert_eq!(parsed.log_level, LogLevel::Debug);
    }

    #[test]
    fn log_level_info() {
        let parsed = opts().run_inner(&["--log-level", "info"]).unwrap();
        assert_eq!(parsed.log_level, LogLevel::Info);
    }

    #[test]
    fn log_level_warn() {
        let parsed = opts().run_inner(&["--log-level", "warn"]).unwrap();
        assert_eq!(parsed.log_level, LogLevel::Warn);
    }

    #[test]
    fn log_level_error() {
        let parsed = opts().run_inner(&["--log-level", "error"]).unwrap();
        assert_eq!(parsed.log_level, LogLevel::Error);
    }

    #[test]
    fn log_level_invalid() {
        assert!(opts().run_inner(&["--log-level", "trace"]).is_err());
    }

    #[test]
    fn colors_off() {
        let parsed = opts().run_inner(&["--colors", "off"]).unwrap();
        assert_eq!(parsed.colors, Some(ColorsArg::Off));
    }

    #[test]
    fn colors_force() {
        let parsed = opts().run_inner(&["--colors", "force"]).unwrap();
        assert_eq!(parsed.colors, Some(ColorsArg::Force));
    }

    #[test]
    fn colors_invalid() {
        assert!(opts().run_inner(&["--colors", "auto"]).is_err());
    }

    #[test]
    fn combined_flags() {
        let parsed = opts()
            .run_inner(&["-v", "--log-level", "debug", "--colors", "force"])
            .unwrap();
        assert!(parsed.verbose);
        assert_eq!(parsed.log_level, LogLevel::Debug);
        assert_eq!(parsed.colors, Some(ColorsArg::Force));
    }

    // --- CliCacheOptions tests ---

    #[test]
    fn cache_defaults() {
        let parsed = cache_opts().run_inner(&[]).unwrap();
        assert!(parsed.cache_dir.is_none());
        assert!(parsed.schema_cache_ttl.is_none());
        assert!(!parsed.force_schema_fetch);
        assert!(!parsed.force_validation);
        assert!(!parsed.force);
        assert!(!parsed.no_catalog);
    }

    #[test]
    fn cache_dir_parsed() {
        let parsed = cache_opts()
            .run_inner(&["--cache-dir", "/tmp/cache"])
            .unwrap();
        assert_eq!(parsed.cache_dir.as_deref(), Some("/tmp/cache"));
    }

    #[test]
    fn schema_cache_ttl_parsed() {
        let parsed = cache_opts()
            .run_inner(&["--schema-cache-ttl", "12h"])
            .unwrap();
        assert_eq!(
            parsed.schema_cache_ttl,
            Some(Duration::from_secs(12 * 3600))
        );
    }

    #[test]
    fn schema_cache_ttl_invalid() {
        assert!(
            cache_opts()
                .run_inner(&["--schema-cache-ttl", "invalid"])
                .is_err()
        );
    }

    #[test]
    fn force_schema_fetch_flag() {
        let parsed = cache_opts().run_inner(&["--force-schema-fetch"]).unwrap();
        assert!(parsed.force_schema_fetch);
    }

    #[test]
    fn force_validation_flag() {
        let parsed = cache_opts().run_inner(&["--force-validation"]).unwrap();
        assert!(parsed.force_validation);
    }

    #[test]
    fn force_flag() {
        let parsed = cache_opts().run_inner(&["--force"]).unwrap();
        assert!(parsed.force);
    }

    #[test]
    fn no_catalog_flag() {
        let parsed = cache_opts().run_inner(&["--no-catalog"]).unwrap();
        assert!(parsed.no_catalog);
    }

    #[test]
    fn cache_combined_flags() {
        let parsed = cache_opts()
            .run_inner(&[
                "--cache-dir",
                "/tmp/cache",
                "--schema-cache-ttl",
                "30m",
                "--force-schema-fetch",
                "--force-validation",
                "--no-catalog",
            ])
            .unwrap();
        assert_eq!(parsed.cache_dir.as_deref(), Some("/tmp/cache"));
        assert_eq!(parsed.schema_cache_ttl, Some(Duration::from_secs(30 * 60)));
        assert!(parsed.force_schema_fetch);
        assert!(parsed.force_validation);
        assert!(parsed.no_catalog);
    }
}
