#![doc = include_str!("../README.md")]

pub mod reporters;

pub use lintel_validate::Reporter;
pub use lintel_validate::format_checked_verbose;
pub use reporters::github::GithubReporter;
pub use reporters::pretty::PrettyReporter;
pub use reporters::text::TextReporter;

// -----------------------------------------------------------------------
// ReporterKind — CLI-parseable enum
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReporterKind {
    Pretty,
    Text,
    Github,
}

impl core::str::FromStr for ReporterKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pretty" => Ok(Self::Pretty),
            "text" => Ok(Self::Text),
            "github" => Ok(Self::Github),
            _ => Err(format!(
                "unknown reporter '{s}', expected: pretty, text, github"
            )),
        }
    }
}

impl core::fmt::Display for ReporterKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Pretty => write!(f, "pretty"),
            Self::Text => write!(f, "text"),
            Self::Github => write!(f, "github"),
        }
    }
}

/// Create a reporter from the kind and verbose flag.
pub fn make_reporter(kind: ReporterKind, verbose: bool) -> Box<dyn Reporter> {
    match kind {
        ReporterKind::Pretty => Box::new(PrettyReporter { verbose }),
        ReporterKind::Text => Box::new(TextReporter { verbose }),
        ReporterKind::Github => Box::new(GithubReporter { verbose }),
    }
}
