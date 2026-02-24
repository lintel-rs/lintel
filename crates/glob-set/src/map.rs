use alloc::vec::Vec;

use crate::engine::{self, MatchEngine};
use crate::glob::{Candidate, Glob};

/// A map from glob patterns to values.
///
/// `GlobMap<T>` pairs each glob pattern with an associated value of type `T`.
/// Lookups return the value associated with the first (lowest-index) matching
/// pattern.
///
/// Internally uses the same optimized strategy-based dispatch as [`GlobSet`]
/// (extension hash, literal, prefix, suffix, Aho-Corasick pre-filter).
///
/// [`GlobSet`]: crate::GlobSet
///
/// # Example
///
/// ```
/// use glob_set::{Glob, GlobMapBuilder};
///
/// let mut builder = GlobMapBuilder::new();
/// builder.insert(Glob::new("*.rs").unwrap(), "rust");
/// builder.insert(Glob::new("*.toml").unwrap(), "toml");
/// let map = builder.build().unwrap();
///
/// assert_eq!(map.get("foo.rs"), Some(&"rust"));
/// assert_eq!(map.get("Cargo.toml"), Some(&"toml"));
/// assert_eq!(map.get("foo.js"), None);
/// ```
#[derive(Clone, Debug)]
pub struct GlobMap<T> {
    engine: MatchEngine,
    values: Vec<T>,
}

impl<T> GlobMap<T> {
    /// Return the value associated with the first matching pattern, or `None`.
    pub fn get(&self, path: impl AsRef<str>) -> Option<&T> {
        self.engine
            .first_match(path.as_ref())
            .map(|idx| &self.values[idx])
    }

    /// Return the value associated with the first matching pattern for a candidate, or `None`.
    pub fn get_candidate(&self, candidate: &Candidate<'_>) -> Option<&T> {
        self.engine
            .first_match(candidate.path())
            .map(|idx| &self.values[idx])
    }

    /// Return references to all values whose patterns match the given path.
    ///
    /// Values are returned in match order (same order as [`GlobSet::matches`]).
    ///
    /// [`GlobSet::matches`]: crate::GlobSet::matches
    pub fn get_matches(&self, path: impl AsRef<str>) -> Vec<&T> {
        let mut indices = Vec::new();
        self.engine.matches_into(path.as_ref(), &mut indices);
        indices.iter().map(|&idx| &self.values[idx]).collect()
    }

    /// Return references to all values whose patterns match the given candidate.
    pub fn get_matches_candidate(&self, candidate: &Candidate<'_>) -> Vec<&T> {
        self.get_matches(candidate.path())
    }

    /// Test whether any pattern matches the given path.
    pub fn is_match(&self, path: impl AsRef<str>) -> bool {
        self.engine.is_match(path.as_ref())
    }

    /// Return the number of patterns in this map.
    pub fn len(&self) -> usize {
        self.engine.len()
    }

    /// Return whether this map is empty.
    pub fn is_empty(&self) -> bool {
        self.engine.is_empty()
    }
}

/// A builder for constructing a [`GlobMap`].
#[derive(Clone, Debug)]
pub struct GlobMapBuilder<T> {
    entries: Vec<(Glob, T)>,
}

impl<T> Default for GlobMapBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> GlobMapBuilder<T> {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Insert a glob pattern and its associated value.
    pub fn insert(&mut self, glob: Glob, value: T) -> &mut Self {
        self.entries.push((glob, value));
        self
    }

    /// Build the [`GlobMap`].
    ///
    /// # Errors
    ///
    /// Returns an error if the Aho-Corasick automaton cannot be constructed.
    pub fn build(self) -> Result<GlobMap<T>, crate::error::Error> {
        let (patterns, values): (Vec<Glob>, Vec<T>) = self.entries.into_iter().unzip();
        let engine = engine::build_engine(patterns)?;
        Ok(GlobMap { engine, values })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use alloc::string::String;

    use super::*;

    fn build_map(entries: &[(&str, &str)]) -> GlobMap<String> {
        let mut builder = GlobMapBuilder::new();
        for &(pat, val) in entries {
            builder.insert(Glob::new(pat).unwrap(), String::from(val));
        }
        builder.build().unwrap()
    }

    #[test]
    fn get_returns_first_match() {
        let map = build_map(&[("*.rs", "rust"), ("**/*.rs", "rust-deep")]);
        // "foo.rs" matches both, but *.rs is index 0
        assert_eq!(map.get("foo.rs").map(String::as_str), Some("rust"));
    }

    #[test]
    fn get_returns_none_on_no_match() {
        let map = build_map(&[("*.rs", "rust")]);
        assert_eq!(map.get("foo.js"), None);
    }

    #[test]
    fn get_matches_returns_all() {
        let map = build_map(&[
            ("*.rs", "rust"),
            ("**/*.rs", "rust-deep"),
            ("*.toml", "toml"),
        ]);
        let matches: Vec<&str> = map
            .get_matches("foo.rs")
            .into_iter()
            .map(String::as_str)
            .collect();
        assert!(matches.contains(&"rust"));
        assert!(matches.contains(&"rust-deep"));
        assert!(!matches.contains(&"toml"));
    }

    #[test]
    fn multiple_patterns_correct_priority() {
        let map = build_map(&[
            ("**/*.rs", "catch-all-rs"),
            ("src/**", "src-dir"),
            ("src/**/*.rs", "src-rs"),
        ]);
        // "src/main.rs" matches all three; first is index 0
        assert_eq!(
            map.get("src/main.rs").map(String::as_str),
            Some("catch-all-rs")
        );
        // "src/main.js" only matches src/**
        assert_eq!(map.get("src/main.js").map(String::as_str), Some("src-dir"));
    }

    #[test]
    fn empty_map_returns_none() {
        let map = build_map(&[]);
        assert_eq!(map.get("anything"), None);
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn brace_expansion_works() {
        let map = build_map(&[("*.{rs,toml}", "rust-or-toml"), ("*.js", "javascript")]);
        assert_eq!(map.get("main.rs").map(String::as_str), Some("rust-or-toml"));
        assert_eq!(
            map.get("Cargo.toml").map(String::as_str),
            Some("rust-or-toml")
        );
        assert_eq!(map.get("app.js").map(String::as_str), Some("javascript"));
        assert_eq!(map.get("style.css"), None);
    }

    #[test]
    fn compound_suffix_works() {
        let map = build_map(&[("**/*.test.js", "test"), ("**/*.js", "js")]);
        assert_eq!(map.get("foo.test.js").map(String::as_str), Some("test"));
        assert_eq!(map.get("foo.js").map(String::as_str), Some("js"));
    }

    #[test]
    fn candidate_based_matching() {
        let map = build_map(&[("**/*.rs", "rust")]);
        let c = Candidate::new("src\\main.rs");
        assert_eq!(map.get_candidate(&c).map(String::as_str), Some("rust"));
    }

    #[test]
    fn get_matches_candidate() {
        let map = build_map(&[("**/*.rs", "rust"), ("src/**", "src")]);
        let c = Candidate::new("src\\main.rs");
        let matches: Vec<&str> = map
            .get_matches_candidate(&c)
            .iter()
            .map(|s| s.as_str())
            .collect();
        assert!(matches.contains(&"rust"));
        assert!(matches.contains(&"src"));
    }

    #[test]
    fn is_match_delegates() {
        let map = build_map(&[("*.rs", "rust")]);
        assert!(map.is_match("foo.rs"));
        assert!(!map.is_match("foo.js"));
    }

    #[test]
    fn len_and_is_empty() {
        let map = build_map(&[("*.rs", "rust"), ("*.toml", "toml")]);
        assert_eq!(map.len(), 2);
        assert!(!map.is_empty());
    }

    #[test]
    fn literal_pattern_in_map() {
        let map = build_map(&[("Cargo.toml", "cargo"), ("*.rs", "rust")]);
        assert_eq!(map.get("Cargo.toml").map(String::as_str), Some("cargo"));
        assert_eq!(map.get("foo.rs").map(String::as_str), Some("rust"));
    }

    #[test]
    fn suffix_pattern_in_map() {
        let map = build_map(&[("**/foo.txt", "foo"), ("*.rs", "rust")]);
        assert_eq!(map.get("a/b/foo.txt").map(String::as_str), Some("foo"));
        assert_eq!(map.get("foo.txt").map(String::as_str), Some("foo"));
    }

    #[test]
    fn prefix_pattern_in_map() {
        let map = build_map(&[("src/**", "source"), ("*.rs", "rust")]);
        assert_eq!(map.get("src/main.rs").map(String::as_str), Some("source"));
        assert_eq!(map.get("main.rs").map(String::as_str), Some("rust"));
    }

    #[test]
    fn first_match_priority_across_strategies() {
        // Index 0 is literal (Cargo.toml), index 1 is ext_any (**/*.toml).
        // For "Cargo.toml", literal match (idx 0) should win.
        let map = build_map(&[("Cargo.toml", "exact"), ("**/*.toml", "any-toml")]);
        assert_eq!(map.get("Cargo.toml").map(String::as_str), Some("exact"));
        assert_eq!(map.get("other.toml").map(String::as_str), Some("any-toml"));
    }

    #[test]
    fn always_check_pattern_in_map() {
        // "*" goes to always_check (no extractable literal)
        let map = build_map(&[("*.rs", "rust"), ("*", "catch-all")]);
        assert_eq!(map.get("foo.rs").map(String::as_str), Some("rust"));
        assert_eq!(map.get("anything").map(String::as_str), Some("catch-all"));
    }
}
