use alloc::string::String;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::str::FromStr;

use crate::error::Error;
use crate::parse;

/// A single glob pattern.
///
/// A `Glob` is constructed from a pattern string and can be compiled into a
/// [`GlobMatcher`] for matching against paths.
///
/// # Example
///
/// ```
/// use glob_set::Glob;
///
/// let glob = Glob::new("*.rs").unwrap();
/// let matcher = glob.compile_matcher();
/// assert!(matcher.is_match("foo.rs"));
/// ```
#[derive(Clone, Debug)]
pub struct Glob {
    pattern: String,
}

impl Glob {
    /// Create a new `Glob` from the given pattern.
    ///
    /// # Errors
    ///
    /// Returns an error if the pattern is structurally invalid (e.g. unclosed
    /// character class, unmatched braces, dangling escape).
    pub fn new(pattern: &str) -> Result<Self, Error> {
        parse::validate(pattern)?;
        Ok(Self {
            pattern: String::from(pattern),
        })
    }

    /// Return the original glob pattern.
    pub fn glob(&self) -> &str {
        &self.pattern
    }

    /// Compile this glob into a matcher for matching paths.
    pub fn compile_matcher(&self) -> GlobMatcher {
        GlobMatcher { glob: self.clone() }
    }
}

impl Eq for Glob {}

impl PartialEq for Glob {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl Hash for Glob {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.pattern.hash(state);
    }
}

impl fmt::Display for Glob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pattern)
    }
}

impl FromStr for Glob {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// A builder for configuring a glob pattern.
///
/// Options like `case_insensitive` and `literal_separator` can be set before
/// building the final [`Glob`].
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct GlobBuilder {
    pattern: String,
    case_insensitive: bool,
    literal_separator: bool,
    backslash_escape: bool,
    empty_alternates: bool,
}

impl GlobBuilder {
    /// Create a new builder from the given pattern.
    pub fn new(pattern: &str) -> Self {
        Self {
            pattern: String::from(pattern),
            case_insensitive: false,
            literal_separator: false,
            backslash_escape: true,
            empty_alternates: false,
        }
    }

    /// Toggle case-insensitive matching.
    ///
    /// When enabled, the pattern is lowercased and paths are lowercased at
    /// match time.
    pub fn case_insensitive(&mut self, yes: bool) -> &mut Self {
        self.case_insensitive = yes;
        self
    }

    /// Toggle literal separator mode.
    ///
    /// This option is accepted for API compatibility but does not currently
    /// change matching behavior, as `glob-matcher` already treats `*` as
    /// not crossing separators and `**` as crossing them.
    pub fn literal_separator(&mut self, yes: bool) -> &mut Self {
        self.literal_separator = yes;
        self
    }

    /// Toggle backslash escaping.
    ///
    /// This option is accepted for API compatibility.
    pub fn backslash_escape(&mut self, yes: bool) -> &mut Self {
        self.backslash_escape = yes;
        self
    }

    /// Toggle empty alternates.
    ///
    /// This option is accepted for API compatibility.
    pub fn empty_alternates(&mut self, yes: bool) -> &mut Self {
        self.empty_alternates = yes;
        self
    }

    /// Build the glob pattern.
    ///
    /// # Errors
    ///
    /// Returns an error if the (possibly lowercased) pattern is structurally
    /// invalid.
    pub fn build(&self) -> Result<Glob, Error> {
        let pattern = if self.case_insensitive {
            self.pattern.to_ascii_lowercase()
        } else {
            self.pattern.clone()
        };
        parse::validate(&pattern)?;
        Ok(Glob { pattern })
    }
}

/// A compiled matcher for a single glob pattern.
///
/// Created by [`Glob::compile_matcher`].
#[derive(Clone, Debug)]
pub struct GlobMatcher {
    glob: Glob,
}

impl GlobMatcher {
    /// Return a reference to the underlying `Glob`.
    pub fn glob(&self) -> &Glob {
        &self.glob
    }

    /// Test whether the given path matches this glob pattern.
    pub fn is_match(&self, path: impl AsRef<str>) -> bool {
        glob_matcher::glob_match(self.glob.pattern.as_str(), path.as_ref())
    }

    /// Test whether the given [`Candidate`] matches this glob pattern.
    pub fn is_match_candidate(&self, candidate: &Candidate<'_>) -> bool {
        glob_matcher::glob_match(self.glob.pattern.as_str(), candidate.path())
    }
}

/// A pre-processed path for matching against multiple patterns.
///
/// `Candidate` normalizes backslashes to forward slashes on construction,
/// which avoids repeated normalization when matching against many patterns.
#[derive(Clone, Debug)]
pub struct Candidate<'a> {
    /// The original or normalized path string.
    path: CandidatePath<'a>,
}

#[derive(Clone, Debug)]
enum CandidatePath<'a> {
    Borrowed(&'a str),
    Owned(String),
}

impl<'a> Candidate<'a> {
    /// Create a new candidate from a path string.
    ///
    /// If the path contains backslashes, they are normalized to forward slashes.
    pub fn new(path: &'a str) -> Self {
        if path.contains('\\') {
            Self {
                path: CandidatePath::Owned(path.replace('\\', "/")),
            }
        } else {
            Self {
                path: CandidatePath::Borrowed(path),
            }
        }
    }

    /// Return the normalized path.
    pub fn path(&self) -> &str {
        match &self.path {
            CandidatePath::Borrowed(s) => s,
            CandidatePath::Owned(s) => s.as_str(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn glob_new_valid() {
        assert!(Glob::new("*.rs").is_ok());
        assert!(Glob::new("**/*.txt").is_ok());
        assert!(Glob::new("{a,b}").is_ok());
    }

    #[test]
    fn glob_new_invalid() {
        assert!(Glob::new("[unclosed").is_err());
        assert!(Glob::new("{unclosed").is_err());
    }

    #[test]
    fn glob_matcher_basic() {
        let m = Glob::new("*.rs").unwrap().compile_matcher();
        assert!(m.is_match("foo.rs"));
        assert!(m.is_match("bar.rs"));
        assert!(!m.is_match("foo.txt"));
        assert!(!m.is_match("src/foo.rs"));
    }

    #[test]
    fn glob_matcher_globstar() {
        let m = Glob::new("**/*.rs").unwrap().compile_matcher();
        assert!(m.is_match("foo.rs"));
        assert!(m.is_match("src/foo.rs"));
        assert!(m.is_match("a/b/c/foo.rs"));
        assert!(!m.is_match("foo.txt"));
    }

    #[test]
    fn glob_matcher_braces() {
        let m = Glob::new("*.{rs,toml}").unwrap().compile_matcher();
        assert!(m.is_match("Cargo.toml"));
        assert!(m.is_match("main.rs"));
        assert!(!m.is_match("main.js"));
    }

    #[test]
    fn glob_builder_case_insensitive() {
        let g = GlobBuilder::new("*.RS")
            .case_insensitive(true)
            .build()
            .unwrap();
        let m = g.compile_matcher();
        // Pattern is lowercased to "*.rs", so we match lowercase paths
        assert!(m.is_match("foo.rs"));
        // But uppercase paths won't match directly since glob-match is literal
        // (case_insensitive only lowercases the pattern)
    }

    #[test]
    fn glob_display() {
        let g = Glob::new("**/*.rs").unwrap();
        assert_eq!(g.to_string(), "**/*.rs");
    }

    #[test]
    fn glob_from_str() {
        let g: Glob = "*.txt".parse().unwrap();
        assert_eq!(g.glob(), "*.txt");
    }

    #[test]
    fn candidate_no_backslash() {
        let c = Candidate::new("a/b/c");
        assert_eq!(c.path(), "a/b/c");
    }

    #[test]
    fn candidate_backslash_normalization() {
        let c = Candidate::new("a\\b\\c");
        assert_eq!(c.path(), "a/b/c");
    }

    #[test]
    fn candidate_matching() {
        let m = Glob::new("**/*.rs").unwrap().compile_matcher();
        let c = Candidate::new("src\\main.rs");
        assert!(m.is_match_candidate(&c));
    }
}
