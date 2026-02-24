use alloc::string::String;
use core::fmt;

/// An error that occurs when parsing a glob pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    /// The original glob pattern that caused this error.
    glob: Option<String>,
    /// The kind of error.
    kind: ErrorKind,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind) -> Self {
        Self { glob: None, kind }
    }

    pub(crate) fn with_glob(mut self, glob: &str) -> Self {
        self.glob = Some(String::from(glob));
        self
    }

    /// Return the glob pattern that caused this error, if available.
    pub fn glob(&self) -> Option<&str> {
        self.glob.as_deref()
    }

    /// Return the kind of this error.
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.glob {
            Some(glob) => write!(f, "error parsing glob '{}': {}", glob, self.kind),
            None => write!(f, "error parsing glob: {}", self.kind),
        }
    }
}

impl core::error::Error for Error {}

/// The kind of error that can occur when parsing a glob pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// An unclosed character class, e.g., `[a-z`.
    UnclosedClass,
    /// An invalid character range, e.g., `[z-a]`.
    InvalidRange(char, char),
    /// An unopened alternation, e.g., `}`.
    UnopenedAlternates,
    /// An unclosed alternation, e.g., `{a,b`.
    UnclosedAlternates,
    /// A dangling escape, e.g., a pattern ending with `\`.
    DanglingEscape,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnclosedClass => write!(f, "unclosed character class"),
            Self::InvalidRange(lo, hi) => {
                write!(f, "invalid character range '{lo}'-'{hi}'")
            }
            Self::UnopenedAlternates => write!(f, "unopened alternation group '}}' without '{{"),
            Self::UnclosedAlternates => write!(f, "unclosed alternation group '{{' without '}}'"),
            Self::DanglingEscape => write!(f, "dangling escape '\\' at end of pattern"),
        }
    }
}
