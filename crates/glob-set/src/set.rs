use alloc::vec::Vec;

use crate::engine::{self, MatchEngine};
use crate::glob::{Candidate, Glob};

/// A set of glob patterns that can be matched against paths efficiently.
///
/// `GlobSet` classifies each pattern at build time into the fastest applicable
/// strategy (extension hash, literal, prefix, suffix) and only falls back to
/// the full `glob_match` engine for patterns that need it.
///
/// # Example
///
/// ```
/// use glob_set::{Glob, GlobSet, GlobSetBuilder};
///
/// let mut builder = GlobSetBuilder::new();
/// builder.add(Glob::new("*.rs").unwrap());
/// builder.add(Glob::new("*.toml").unwrap());
/// let set = builder.build().unwrap();
///
/// assert!(set.is_match("foo.rs"));
/// assert!(set.is_match("Cargo.toml"));
/// assert!(!set.is_match("foo.js"));
/// ```
#[derive(Clone, Debug)]
pub struct GlobSet {
    engine: MatchEngine,
}

impl Default for GlobSet {
    fn default() -> Self {
        Self {
            engine: MatchEngine::empty(),
        }
    }
}

impl GlobSet {
    /// Return the number of patterns in this set.
    pub fn len(&self) -> usize {
        self.engine.len()
    }

    /// Return whether this set is empty.
    pub fn is_empty(&self) -> bool {
        self.engine.is_empty()
    }

    /// Test whether any pattern matches the given path.
    pub fn is_match(&self, path: impl AsRef<str>) -> bool {
        self.engine.is_match(path.as_ref())
    }

    /// Test whether any pattern matches the given candidate.
    pub fn is_match_candidate(&self, candidate: &Candidate<'_>) -> bool {
        self.engine.is_match(candidate.path())
    }

    /// Return the indices of all patterns that match the given path.
    pub fn matches(&self, path: impl AsRef<str>) -> Vec<usize> {
        let mut result = Vec::new();
        self.engine.matches_into(path.as_ref(), &mut result);
        result
    }

    /// Append the indices of all matching patterns to `into`.
    pub fn matches_into(&self, path: impl AsRef<str>, into: &mut Vec<usize>) {
        self.engine.matches_into(path.as_ref(), into);
    }

    /// Return the indices of all patterns that match the given candidate.
    pub fn matches_candidate(&self, candidate: &Candidate<'_>) -> Vec<usize> {
        self.matches(candidate.path())
    }

    /// Append the indices of all matching patterns for the given candidate to `into`.
    pub fn matches_candidate_into(&self, candidate: &Candidate<'_>, into: &mut Vec<usize>) {
        self.matches_into(candidate.path(), into);
    }
}

/// A builder for constructing a [`GlobSet`].
#[derive(Clone, Debug, Default)]
pub struct GlobSetBuilder {
    patterns: Vec<Glob>,
}

impl GlobSetBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a glob pattern to the set.
    pub fn add(&mut self, glob: Glob) -> &mut Self {
        self.patterns.push(glob);
        self
    }

    /// Build the [`GlobSet`].
    ///
    /// This classifies each pattern into the fastest applicable strategy
    /// (extension, literal, prefix, suffix) and only uses the full glob
    /// engine with Aho-Corasick pre-filtering for patterns that need it.
    ///
    /// # Errors
    ///
    /// Returns an error if the Aho-Corasick automaton cannot be constructed.
    pub fn build(&self) -> Result<GlobSet, crate::error::Error> {
        let engine = engine::build_engine(self.patterns.clone())?;
        Ok(GlobSet { engine })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use alloc::vec;

    use super::*;

    fn build_set(patterns: &[&str]) -> GlobSet {
        let mut builder = GlobSetBuilder::new();
        for p in patterns {
            builder.add(Glob::new(p).unwrap());
        }
        builder.build().unwrap()
    }

    #[test]
    fn empty_set() {
        let set = build_set(&[]);
        assert!(!set.is_match("anything"));
        assert!(set.is_empty());
    }

    #[test]
    fn single_pattern() {
        let set = build_set(&["*.rs"]);
        assert!(set.is_match("foo.rs"));
        assert!(!set.is_match("foo.txt"));
    }

    #[test]
    fn multiple_patterns() {
        let set = build_set(&["*.rs", "*.toml", "*.md"]);
        assert!(set.is_match("main.rs"));
        assert!(set.is_match("Cargo.toml"));
        assert!(set.is_match("README.md"));
        assert!(!set.is_match("main.js"));
    }

    #[test]
    fn matches_returns_indices() {
        let set = build_set(&["*.rs", "*.toml", "**/*.rs"]);
        let mut indices = set.matches("src/main.rs");
        indices.sort_unstable();
        assert!(indices.contains(&2)); // **/*.rs matches
        assert!(!indices.contains(&1)); // *.toml doesn't match
    }

    #[test]
    fn globstar_patterns() {
        let set = build_set(&["**/*.test.js", "src/**/*.rs"]);
        assert!(set.is_match("foo/bar.test.js"));
        assert!(set.is_match("src/lib.rs"));
        assert!(!set.is_match("test/foo.rs"));
    }

    #[test]
    fn wildcard_only_patterns_in_always_check() {
        // "*" has no literal, so it goes into always_check
        let set = build_set(&["*", "*.rs"]);
        assert!(set.is_match("anything"));
        assert!(set.is_match("foo.rs"));
    }

    #[test]
    fn matches_into() {
        let set = build_set(&["*.rs", "*.txt", "**/*"]);
        let mut results = Vec::new();
        set.matches_into("foo.rs", &mut results);
        assert!(results.contains(&0)); // *.rs
        assert!(results.contains(&2)); // **/*
        assert!(!results.contains(&1)); // *.txt
    }

    #[test]
    fn candidate_matching() {
        let set = build_set(&["**/*.rs"]);
        let c = Candidate::new("src\\main.rs");
        assert!(set.is_match_candidate(&c));
    }

    #[test]
    fn default_glob_set() {
        let set = GlobSet::default();
        assert!(set.is_empty());
        assert!(!set.is_match("anything"));
    }

    #[test]
    fn braces_pattern() {
        let set = build_set(&["*.{rs,toml}"]);
        assert!(set.is_match("main.rs"));
        assert!(set.is_match("Cargo.toml"));
        assert!(!set.is_match("main.js"));
    }

    #[test]
    fn question_mark_pattern() {
        let set = build_set(&["a?c"]);
        assert!(set.is_match("abc"));
        assert!(set.is_match("axc"));
        assert!(!set.is_match("abbc"));
    }

    #[test]
    fn char_class_pattern() {
        let set = build_set(&["[abc].txt"]);
        assert!(set.is_match("a.txt"));
        assert!(set.is_match("b.txt"));
        assert!(!set.is_match("d.txt"));
    }

    #[test]
    fn literal_strategy() {
        let set = build_set(&["Cargo.toml"]);
        assert!(set.is_match("Cargo.toml"));
        assert!(!set.is_match("cargo.toml"));
        assert!(!set.is_match("src/Cargo.toml"));
    }

    #[test]
    fn prefix_strategy() {
        let set = build_set(&["src/**"]);
        assert!(set.is_match("src/main.rs"));
        assert!(set.is_match("src/lib/util.rs"));
        assert!(!set.is_match("tests/main.rs"));
    }

    #[test]
    fn suffix_strategy() {
        let set = build_set(&["**/foo.txt"]);
        assert!(set.is_match("a/b/foo.txt"));
        assert!(set.is_match("foo.txt")); // also matches without leading /
        assert!(!set.is_match("bar.txt"));
    }

    #[test]
    fn mixed_strategies() {
        let set = build_set(&[
            "*.rs",          // extension local
            "Cargo.toml",    // literal
            "src/**",        // prefix
            "**/README.md",  // suffix
            "{a,b}/**/*.js", // glob fallback
        ]);
        assert!(set.is_match("foo.rs"));
        assert!(set.is_match("Cargo.toml"));
        assert!(set.is_match("src/lib.rs"));
        assert!(set.is_match("docs/README.md"));
        assert!(set.is_match("a/components/app.js"));
        assert!(!set.is_match("foo.py"));
    }

    #[test]
    fn matches_into_mixed_strategies() {
        let set = build_set(&[
            "**/*.rs", // ext_any (idx 0)
            "src/**",  // prefix  (idx 1)
            "*",       // glob/always-check (idx 2)
        ]);
        let mut results = Vec::new();
        set.matches_into("src/main.rs", &mut results);
        results.sort_unstable();
        assert_eq!(results, vec![0, 1]);

        results.clear();
        set.matches_into("main.rs", &mut results);
        results.sort_unstable();
        assert_eq!(results, vec![0, 2]);
    }

    #[test]
    fn extension_does_not_false_positive() {
        // `*.rs` should NOT match `src/main.rs` (single star doesn't cross /).
        let set = build_set(&["*.rs"]);
        assert!(!set.is_match("src/main.rs"));
    }

    #[test]
    fn ext_any_matches_deep_paths() {
        let set = build_set(&["**/*.rs"]);
        assert!(set.is_match("a/b/c/d.rs"));
        assert!(set.is_match("d.rs"));
    }

    #[test]
    fn ext_local_rejects_deep_paths() {
        let set = build_set(&["*.rs"]);
        assert!(set.is_match("d.rs"));
        assert!(!set.is_match("a/d.rs"));
    }

    #[test]
    fn compound_suffix_strategy() {
        let set = build_set(&["**/*.test.js"]);
        assert!(set.is_match("foo.test.js"));
        assert!(set.is_match("a/b/foo.test.js"));
        assert!(!set.is_match("foo.js"));
        assert!(!set.is_match("foo.test.ts"));
    }

    #[test]
    fn compound_suffix_matches_into() {
        let set = build_set(&["**/*.test.js", "**/*.rs"]);
        let mut results = Vec::new();
        set.matches_into("unit/foo.test.js", &mut results);
        assert_eq!(results, vec![0]);
    }

    #[test]
    fn brace_expansion_in_set() {
        let set = build_set(&["*.{rs,toml}"]);
        assert!(set.is_match("main.rs"));
        assert!(set.is_match("Cargo.toml"));
        assert!(!set.is_match("main.js"));
    }

    #[test]
    fn brace_expansion_globstar() {
        let set = build_set(&["**/*.{rs,toml}"]);
        assert!(set.is_match("src/main.rs"));
        assert!(set.is_match("Cargo.toml"));
        assert!(!set.is_match("main.js"));
    }

    #[test]
    fn prefix_suffix_strategy() {
        let set = build_set(&["src/**/*.js"]);
        assert!(set.is_match("src/app.js"));
        assert!(set.is_match("src/components/button.js"));
        assert!(!set.is_match("lib/app.js"));
        assert!(!set.is_match("src/app.ts"));
    }

    #[test]
    fn prefix_compound_suffix_strategy() {
        let set = build_set(&["tests/**/*.test.ts"]);
        assert!(set.is_match("tests/unit/foo.test.ts"));
        assert!(set.is_match("tests/foo.test.ts"));
        assert!(!set.is_match("src/foo.test.ts"));
        assert!(!set.is_match("tests/foo.ts"));
    }

    #[test]
    fn brace_expansion_prefix_suffix() {
        let set = build_set(&["{src,lib}/**/*.rs"]);
        assert!(set.is_match("src/main.rs"));
        assert!(set.is_match("lib/core/parser.rs"));
        assert!(!set.is_match("tests/main.rs"));
        assert!(!set.is_match("src/main.js"));
    }
}
