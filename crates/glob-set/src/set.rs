use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use aho_corasick::AhoCorasick;
use hashbrown::HashMap;

use crate::glob::{Candidate, Glob};
use crate::literal;
use crate::strategy;

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
#[derive(Clone, Debug, Default)]
pub struct GlobSet {
    patterns: Vec<Glob>,
    /// Extension → indices for `**/*.ext` patterns (extension alone is sufficient).
    ext_any: HashMap<String, Vec<usize>>,
    /// Extension → indices for `*.ext` patterns (need `glob_match` verification).
    ext_local: HashMap<String, Vec<usize>>,
    /// Literal path → pattern index.
    literals: HashMap<String, usize>,
    /// Prefix (with trailing `/`) → pattern index.
    prefixes: Vec<(String, usize)>,
    /// Suffix (with leading `/`) → pattern index.
    suffixes: Vec<(String, usize)>,
    /// Compound suffix (no leading `/`, e.g. `.test.js`) → pattern index.
    compound_suffixes: Vec<(String, usize)>,
    /// Prefix+suffix pairs → pattern index.
    prefix_suffixes: Vec<(String, String, usize)>,
    /// Aho-Corasick automaton for remaining glob patterns.
    ac: Option<AhoCorasick>,
    /// AC pattern index → glob index.
    ac_to_glob: Vec<usize>,
    /// Glob indices with no extractable literal (must always be checked).
    always_check: Vec<usize>,
}

impl GlobSet {
    /// Return the number of patterns in this set.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Return whether this set is empty.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Test whether any pattern matches the given path.
    pub fn is_match(&self, path: impl AsRef<str>) -> bool {
        let path = path.as_ref();
        if self.patterns.is_empty() {
            return false;
        }

        if let Some(ext) = strategy::path_extension(path) {
            // 1a. ExtensionAny — `**/*.ext`: extension match is sufficient.
            if self.ext_any.contains_key(ext) {
                return true;
            }
            // 1b. ExtensionLocal — `*.ext`: verify with glob_match (single star
            //     doesn't cross separators).
            if let Some(indices) = self.ext_local.get(ext) {
                for &idx in indices {
                    if glob_matcher::glob_match(self.patterns[idx].glob(), path) {
                        return true;
                    }
                }
            }
        }

        // 2. Literal strategy.
        if self.literals.contains_key(path) {
            return true;
        }

        // 3. Prefix strategy.
        for (prefix, _) in &self.prefixes {
            if path.starts_with(prefix.as_str()) {
                return true;
            }
        }

        // 4. Suffix strategy.
        //    Suffix has a leading `/`, e.g. "/foo.txt" for `**/foo.txt`.
        //    Also match when path equals the suffix without the `/`
        //    (globstar matches zero segments).
        for (suffix, _) in &self.suffixes {
            if path.ends_with(suffix.as_str()) || path == &suffix[1..] {
                return true;
            }
        }

        // 5. Compound suffix strategy (`**/*<literal>`, e.g. `**/*.test.js`).
        for (suffix, _) in &self.compound_suffixes {
            if path.ends_with(suffix.as_str()) {
                return true;
            }
        }

        // 6. Prefix+suffix strategy (`prefix/**/*<suffix>`).
        for (prefix, suffix, _) in &self.prefix_suffixes {
            if path.starts_with(prefix.as_str()) && path.ends_with(suffix.as_str()) {
                return true;
            }
        }

        // 8. Full glob fallback — always-check patterns.
        for &idx in &self.always_check {
            if glob_matcher::glob_match(self.patterns[idx].glob(), path) {
                return true;
            }
        }

        // 9. Full glob fallback — AC pre-filter.
        if let Some(ac) = &self.ac {
            for mat in ac.find_overlapping_iter(path) {
                let glob_idx = self.ac_to_glob[mat.pattern().as_usize()];
                if glob_matcher::glob_match(self.patterns[glob_idx].glob(), path) {
                    return true;
                }
            }
        }

        false
    }

    /// Test whether any pattern matches the given candidate.
    pub fn is_match_candidate(&self, candidate: &Candidate<'_>) -> bool {
        self.is_match(candidate.path())
    }

    /// Return the indices of all patterns that match the given path.
    pub fn matches(&self, path: impl AsRef<str>) -> Vec<usize> {
        let mut result = Vec::new();
        self.matches_into(path, &mut result);
        result
    }

    /// Append the indices of all matching patterns to `into`.
    pub fn matches_into(&self, path: impl AsRef<str>, into: &mut Vec<usize>) {
        let path = path.as_ref();
        if self.patterns.is_empty() {
            return;
        }

        let mut seen = vec![false; self.patterns.len()];

        // 1. Extension strategies.
        if let Some(ext) = strategy::path_extension(path) {
            // 1a. ExtensionAny — no verification needed.
            if let Some(indices) = self.ext_any.get(ext) {
                for &idx in indices {
                    if !seen[idx] {
                        into.push(idx);
                        seen[idx] = true;
                    }
                }
            }
            // 1b. ExtensionLocal — verify with glob_match.
            if let Some(indices) = self.ext_local.get(ext) {
                for &idx in indices {
                    if !seen[idx] && glob_matcher::glob_match(self.patterns[idx].glob(), path) {
                        into.push(idx);
                        seen[idx] = true;
                    }
                }
            }
        }

        // 2. Literal strategy.
        if let Some(&idx) = self.literals.get(path)
            && !seen[idx]
        {
            into.push(idx);
            seen[idx] = true;
        }

        // 3. Prefix strategy.
        for (prefix, idx) in &self.prefixes {
            if !seen[*idx] && path.starts_with(prefix.as_str()) {
                into.push(*idx);
                seen[*idx] = true;
            }
        }

        // 4. Suffix strategy.
        for (suffix, idx) in &self.suffixes {
            if !seen[*idx] && (path.ends_with(suffix.as_str()) || path == &suffix[1..]) {
                into.push(*idx);
                seen[*idx] = true;
            }
        }

        // 5. Compound suffix strategy.
        for (suffix, idx) in &self.compound_suffixes {
            if !seen[*idx] && path.ends_with(suffix.as_str()) {
                into.push(*idx);
                seen[*idx] = true;
            }
        }

        // 6. Prefix+suffix strategy.
        for (prefix, suffix, idx) in &self.prefix_suffixes {
            if !seen[*idx] && path.starts_with(prefix.as_str()) && path.ends_with(suffix.as_str()) {
                into.push(*idx);
                seen[*idx] = true;
            }
        }

        // 7. Always-check patterns.
        for &idx in &self.always_check {
            if !seen[idx] && glob_matcher::glob_match(self.patterns[idx].glob(), path) {
                into.push(idx);
                seen[idx] = true;
            }
        }

        // 8. AC pre-filter.
        if let Some(ac) = &self.ac {
            for mat in ac.find_overlapping_iter(path) {
                let glob_idx = self.ac_to_glob[mat.pattern().as_usize()];
                if !seen[glob_idx] && glob_matcher::glob_match(self.patterns[glob_idx].glob(), path)
                {
                    into.push(glob_idx);
                    seen[glob_idx] = true;
                }
            }
        }
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
        let pat_strs: Vec<&str> = self.patterns.iter().map(Glob::glob).collect();
        let strats = strategy::build(&pat_strs);

        // Build AC automaton for the glob-fallback patterns only.
        let mut ac_patterns: Vec<String> = Vec::new();
        let mut ac_to_glob: Vec<usize> = Vec::new();
        let mut always_check: Vec<usize> = Vec::new();

        for &idx in &strats.glob_indices {
            match literal::extract_literal(self.patterns[idx].glob()) {
                Some(lit) => {
                    ac_patterns.push(String::from(lit));
                    ac_to_glob.push(idx);
                }
                None => {
                    always_check.push(idx);
                }
            }
        }

        let ac =
            if ac_patterns.is_empty() {
                None
            } else {
                Some(AhoCorasick::builder().build(&ac_patterns).map_err(|_| {
                    crate::error::Error::new(crate::error::ErrorKind::UnclosedClass)
                })?)
            };

        Ok(GlobSet {
            patterns: self.patterns.clone(),
            ext_any: strats.ext_any,
            ext_local: strats.ext_local,
            literals: strats.literals,
            prefixes: strats.prefixes,
            suffixes: strats.suffixes,
            compound_suffixes: strats.compound_suffixes,
            prefix_suffixes: strats.prefix_suffixes,
            ac,
            ac_to_glob,
            always_check,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
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
