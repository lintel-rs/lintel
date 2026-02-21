use bpaf::Bpaf;

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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bpaf::Parser;

    fn opts() -> bpaf::OptionParser<CLIGlobalOptions> {
        cli_global_options().to_options()
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
}
