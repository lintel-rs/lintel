//! A lightweight glob set using linear scans instead of trie indexing.
//!
//! [`TinyGlobSet`] uses the same pattern classification as [`GlobSet`] (extension,
//! literal, prefix, suffix, Aho-Corasick) but stores prefix and suffix entries in
//! plain `Vec`s and matches them with linear scans. This makes it **~2.5x faster
//! to build** than `GlobSet` at the cost of **~20% slower queries** on typical
//! workloads.
//!
//! # When to use `TinyGlobSet`
//!
//! - **Few prefix/suffix patterns** (roughly < 20) where the linear scan stays
//!   fast and the build-time savings dominate.
//! - **Short-lived or frequently rebuilt** sets, e.g. per-request pattern
//!   matching where construction cost matters more than query throughput.
//! - **Minimal memory overhead** is important — no double-array trie allocation.
//!
//! # When to use [`GlobSet`] instead
//!
//! - **Many prefix/suffix patterns** — `GlobSet` uses a double-array trie with
//!   O(key-length) lookups regardless of how many patterns share the same
//!   strategy bucket.
//! - **Query-heavy workloads** where the set is built once and matched millions
//!   of times (e.g. file-tree walking).
//! - **Priority-aware matching** — `GlobSet` supports [`GlobSet::first_match`]
//!   and powers [`GlobMap`], which `TinyGlobSet` does not.
//!
//! [`GlobSet`]: crate::GlobSet
//! [`GlobSet::first_match`]: crate::GlobSet::first_match
//! [`GlobMap`]: crate::GlobMap

#![allow(clippy::missing_errors_doc)]

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use aho_corasick::AhoCorasick;
use hashbrown::HashMap;

use crate::glob::{Candidate, Glob};
use crate::literal;
use crate::strategy;

/// A lightweight glob set that trades query speed for fast construction.
///
/// See the [module docs](self) for guidance on when to choose this over
/// [`GlobSet`](crate::GlobSet).
#[derive(Clone, Debug, Default)]
pub struct TinyGlobSet {
    patterns: Vec<Glob>,
    ext_any: HashMap<String, Vec<usize>>,
    ext_local: HashMap<String, Vec<usize>>,
    literals: HashMap<String, usize>,
    prefixes: Vec<(String, usize)>,
    suffixes: Vec<(String, usize)>,
    compound_suffixes: Vec<(String, usize)>,
    prefix_suffixes: Vec<(String, String, usize)>,
    ac: Option<AhoCorasick>,
    ac_to_glob: Vec<usize>,
    always_check: Vec<usize>,
}

impl TinyGlobSet {
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn is_match(&self, path: impl AsRef<str>) -> bool {
        let path = path.as_ref();
        if self.patterns.is_empty() {
            return false;
        }

        if let Some(ext) = strategy::path_extension(path) {
            if self.ext_any.contains_key(ext) {
                return true;
            }
            if let Some(indices) = self.ext_local.get(ext) {
                for &idx in indices {
                    if glob_matcher::glob_match(self.patterns[idx].glob(), path) {
                        return true;
                    }
                }
            }
        }

        if self.literals.contains_key(path) {
            return true;
        }

        for (prefix, _) in &self.prefixes {
            if path.starts_with(prefix.as_str()) {
                return true;
            }
        }

        for (suffix, _) in &self.suffixes {
            if path.ends_with(suffix.as_str()) || path == &suffix[1..] {
                return true;
            }
        }

        for (suffix, _) in &self.compound_suffixes {
            if path.ends_with(suffix.as_str()) {
                return true;
            }
        }

        for (prefix, suffix, _) in &self.prefix_suffixes {
            if path.starts_with(prefix.as_str()) && path.ends_with(suffix.as_str()) {
                return true;
            }
        }

        for &idx in &self.always_check {
            if glob_matcher::glob_match(self.patterns[idx].glob(), path) {
                return true;
            }
        }

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

    pub fn is_match_candidate(&self, candidate: &Candidate<'_>) -> bool {
        self.is_match(candidate.path())
    }

    pub fn matches(&self, path: impl AsRef<str>) -> Vec<usize> {
        let mut result = Vec::new();
        self.matches_into(path, &mut result);
        result
    }

    pub fn matches_into(&self, path: impl AsRef<str>, into: &mut Vec<usize>) {
        let path = path.as_ref();
        if self.patterns.is_empty() {
            return;
        }

        let mut seen = vec![false; self.patterns.len()];

        if let Some(ext) = strategy::path_extension(path) {
            if let Some(indices) = self.ext_any.get(ext) {
                for &idx in indices {
                    if !seen[idx] {
                        into.push(idx);
                        seen[idx] = true;
                    }
                }
            }
            if let Some(indices) = self.ext_local.get(ext) {
                for &idx in indices {
                    if !seen[idx] && glob_matcher::glob_match(self.patterns[idx].glob(), path) {
                        into.push(idx);
                        seen[idx] = true;
                    }
                }
            }
        }

        if let Some(&idx) = self.literals.get(path)
            && !seen[idx]
        {
            into.push(idx);
            seen[idx] = true;
        }

        for (prefix, idx) in &self.prefixes {
            if !seen[*idx] && path.starts_with(prefix.as_str()) {
                into.push(*idx);
                seen[*idx] = true;
            }
        }

        for (suffix, idx) in &self.suffixes {
            if !seen[*idx] && (path.ends_with(suffix.as_str()) || path == &suffix[1..]) {
                into.push(*idx);
                seen[*idx] = true;
            }
        }

        for (suffix, idx) in &self.compound_suffixes {
            if !seen[*idx] && path.ends_with(suffix.as_str()) {
                into.push(*idx);
                seen[*idx] = true;
            }
        }

        for (prefix, suffix, idx) in &self.prefix_suffixes {
            if !seen[*idx] && path.starts_with(prefix.as_str()) && path.ends_with(suffix.as_str()) {
                into.push(*idx);
                seen[*idx] = true;
            }
        }

        for &idx in &self.always_check {
            if !seen[idx] && glob_matcher::glob_match(self.patterns[idx].glob(), path) {
                into.push(idx);
                seen[idx] = true;
            }
        }

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

    pub fn matches_candidate(&self, candidate: &Candidate<'_>) -> Vec<usize> {
        self.matches(candidate.path())
    }

    pub fn matches_candidate_into(&self, candidate: &Candidate<'_>, into: &mut Vec<usize>) {
        self.matches_into(candidate.path(), into);
    }
}

/// A builder for constructing a [`TinyGlobSet`].
#[derive(Clone, Debug, Default)]
pub struct TinyGlobSetBuilder {
    patterns: Vec<Glob>,
}

impl TinyGlobSetBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, glob: Glob) -> &mut Self {
        self.patterns.push(glob);
        self
    }

    pub fn build(&self) -> Result<TinyGlobSet, crate::error::Error> {
        let pat_strs: Vec<&str> = self.patterns.iter().map(Glob::glob).collect();
        let strats = strategy::build(&pat_strs);

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

        Ok(TinyGlobSet {
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
