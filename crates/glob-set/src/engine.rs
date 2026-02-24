use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use aho_corasick::AhoCorasick;
use hashbrown::HashMap;
use tried::{DoubleArray, DoubleArrayBuilder};

use crate::glob::Glob;
use crate::literal;
use crate::strategy;

/// Side table for prefix DA matches.
#[derive(Clone)]
struct PrefixEntry {
    /// Pattern indices for pure prefix matches.
    indices: Box<[usize]>,
    /// Suffix string + pattern index pairs needing suffix verification.
    prefix_suffix: Box<[(Box<str>, usize)]>,
}

/// Side table for suffix DA matches.
#[derive(Clone)]
struct SuffixEntry {
    /// Pattern indices for `ends_with` matches.
    indices: Box<[usize]>,
    /// Pattern indices for exact path matches.
    exact_indices: Box<[usize]>,
}

/// Shared matching engine used by both `GlobSet` and `GlobMap`.
///
/// Contains the classified pattern strategies and matching logic.
/// All collections are frozen after construction (`Box<[T]>`).
#[derive(Clone)]
pub(crate) struct MatchEngine {
    patterns: Box<[Glob]>,
    /// Extension -> indices for `**/*.ext` patterns (extension alone is sufficient).
    ext_any: HashMap<String, Box<[usize]>>,
    /// Extension -> indices for `*.ext` patterns (need `glob_match` verification).
    ext_local: HashMap<String, Box<[usize]>>,
    /// Literal path -> pattern index.
    literals: HashMap<String, usize>,
    /// Forward double-array trie for prefix and prefix+suffix matching.
    prefix_da: Option<DoubleArray<Box<[u8]>>>,
    prefix_entries: Box<[PrefixEntry]>,
    /// Reverse double-array trie for suffix and compound-suffix matching.
    suffix_da: Option<DoubleArray<Box<[u8]>>>,
    suffix_entries: Box<[SuffixEntry]>,
    /// Aho-Corasick automaton for remaining glob patterns.
    ac: Option<AhoCorasick>,
    /// AC pattern index -> glob index.
    ac_to_glob: Box<[usize]>,
    /// Glob indices with no extractable literal (must always be checked).
    always_check: Box<[usize]>,
}

impl fmt::Debug for MatchEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MatchEngine")
            .field("patterns", &self.patterns)
            .field("ext_any", &self.ext_any)
            .field("ext_local", &self.ext_local)
            .field("literals", &self.literals)
            .field("prefix_da", &self.prefix_da.as_ref().map(|_| ".."))
            .field("prefix_entries_len", &self.prefix_entries.len())
            .field("suffix_da", &self.suffix_da.as_ref().map(|_| ".."))
            .field("suffix_entries_len", &self.suffix_entries.len())
            .field("ac", &self.ac)
            .field("ac_to_glob", &self.ac_to_glob)
            .field("always_check", &self.always_check)
            .finish()
    }
}

impl MatchEngine {
    /// Create an empty engine with no patterns.
    pub(crate) fn empty() -> Self {
        Self {
            patterns: Box::default(),
            ext_any: HashMap::new(),
            ext_local: HashMap::new(),
            literals: HashMap::new(),
            prefix_da: None,
            prefix_entries: Box::default(),
            suffix_da: None,
            suffix_entries: Box::default(),
            ac: None,
            ac_to_glob: Box::default(),
            always_check: Box::default(),
        }
    }

    /// Return the number of patterns in this engine.
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Return whether this engine has no patterns.
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Test whether any pattern matches the given path.
    #[inline]
    pub(crate) fn is_match(&self, path: &str) -> bool {
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

        if let Some(da) = &self.prefix_da {
            for (slot, _len) in da.common_prefix_search(path) {
                let e = &self.prefix_entries[slot as usize];
                if !e.indices.is_empty() {
                    return true;
                }
                for (sfx, _) in &*e.prefix_suffix {
                    if path.ends_with(&**sfx) {
                        return true;
                    }
                }
            }
        }

        if let Some(da) = &self.suffix_da {
            let rev: Vec<u8> = path.as_bytes().iter().rev().copied().collect();
            for (slot, len) in da.common_prefix_search(&rev) {
                let e = &self.suffix_entries[slot as usize];
                if !e.indices.is_empty() {
                    return true;
                }
                if len == path.len() && !e.exact_indices.is_empty() {
                    return true;
                }
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

    /// Append the indices of all matching patterns to `into`.
    #[inline]
    #[allow(clippy::cognitive_complexity)]
    pub(crate) fn matches_into(&self, path: &str, into: &mut Vec<usize>) {
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

        if let Some(da) = &self.prefix_da {
            for (slot, _len) in da.common_prefix_search(path) {
                let e = &self.prefix_entries[slot as usize];
                for &pat_idx in &*e.indices {
                    if !seen[pat_idx] {
                        into.push(pat_idx);
                        seen[pat_idx] = true;
                    }
                }
                for &(ref sfx, pat_idx) in &*e.prefix_suffix {
                    if !seen[pat_idx] && path.ends_with(&**sfx) {
                        into.push(pat_idx);
                        seen[pat_idx] = true;
                    }
                }
            }
        }

        if let Some(da) = &self.suffix_da {
            let rev: Vec<u8> = path.as_bytes().iter().rev().copied().collect();
            for (slot, len) in da.common_prefix_search(&rev) {
                let e = &self.suffix_entries[slot as usize];
                for &pat_idx in &*e.indices {
                    if !seen[pat_idx] {
                        into.push(pat_idx);
                        seen[pat_idx] = true;
                    }
                }
                if len == path.len() {
                    for &pat_idx in &*e.exact_indices {
                        if !seen[pat_idx] {
                            into.push(pat_idx);
                            seen[pat_idx] = true;
                        }
                    }
                }
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

    /// Find the minimum pattern index that matches the given path.
    ///
    /// Checks every strategy and tracks the minimum matching index. Each
    /// strategy skips work once it can't beat the current best. No allocation.
    #[inline]
    #[allow(clippy::cognitive_complexity)]
    pub(crate) fn first_match(&self, path: &str) -> Option<usize> {
        if self.patterns.is_empty() {
            return None;
        }
        let mut best: usize = usize::MAX;

        // 1. Extension — O(1) hash lookup
        if let Some(ext) = strategy::path_extension(path) {
            // 1a. ExtensionAny — indices are in insertion order; [0] is minimum.
            if let Some(indices) = self.ext_any.get(ext) {
                best = best.min(indices[0]);
            }
            // 1b. ExtensionLocal — verify with glob_match.
            if let Some(indices) = self.ext_local.get(ext) {
                for &idx in indices {
                    if idx >= best {
                        break;
                    }
                    if glob_matcher::glob_match(self.patterns[idx].glob(), path) {
                        best = idx;
                        break;
                    }
                }
            }
        }

        // 2. Literal — O(1) hash lookup
        if let Some(&idx) = self.literals.get(path) {
            best = best.min(idx);
        }

        // 3. Prefix + prefix-suffix — DA walk
        if let Some(da) = &self.prefix_da {
            for (slot, _len) in da.common_prefix_search(path) {
                let e = &self.prefix_entries[slot as usize];
                for &pat_idx in &*e.indices {
                    best = best.min(pat_idx);
                }
                for &(ref sfx, pat_idx) in &*e.prefix_suffix {
                    if pat_idx < best && path.ends_with(&**sfx) {
                        best = pat_idx;
                    }
                }
            }
        }

        // 4. Suffix + compound suffix — reverse DA walk
        if let Some(da) = &self.suffix_da {
            let rev: Vec<u8> = path.as_bytes().iter().rev().copied().collect();
            for (slot, len) in da.common_prefix_search(&rev) {
                let e = &self.suffix_entries[slot as usize];
                for &pat_idx in &*e.indices {
                    best = best.min(pat_idx);
                }
                if len == path.len() {
                    for &pat_idx in &*e.exact_indices {
                        best = best.min(pat_idx);
                    }
                }
            }
        }

        // 5. Always-check — glob_match with idx < best guard
        for &idx in &self.always_check {
            if idx >= best {
                continue;
            }
            if glob_matcher::glob_match(self.patterns[idx].glob(), path) {
                best = idx;
            }
        }

        // 6. AC pre-filter — glob_match with idx < best guard
        if let Some(ac) = &self.ac {
            for mat in ac.find_overlapping_iter(path) {
                let glob_idx = self.ac_to_glob[mat.pattern().as_usize()];
                if glob_idx >= best {
                    continue;
                }
                if glob_matcher::glob_match(self.patterns[glob_idx].glob(), path) {
                    best = glob_idx;
                }
            }
        }

        if best == usize::MAX { None } else { Some(best) }
    }
}

// ---------------------------------------------------------------------------
// DA builders
// ---------------------------------------------------------------------------

struct PrefixEntryBuilder {
    indices: Vec<usize>,
    prefix_suffix: Vec<(Box<str>, usize)>,
}

struct SuffixEntryBuilder {
    indices: Vec<usize>,
    exact_indices: Vec<usize>,
}

#[allow(clippy::type_complexity)]
fn build_prefix_da(
    strats: &strategy::Strategies,
) -> (Option<DoubleArray<Box<[u8]>>>, Box<[PrefixEntry]>) {
    let mut map: HashMap<String, PrefixEntryBuilder> = HashMap::new();

    for (prefix, idx) in &strats.prefixes {
        map.entry(prefix.clone())
            .or_insert_with(|| PrefixEntryBuilder {
                indices: Vec::new(),
                prefix_suffix: Vec::new(),
            })
            .indices
            .push(*idx);
    }
    for (prefix, suffix, idx) in &strats.prefix_suffixes {
        map.entry(prefix.clone())
            .or_insert_with(|| PrefixEntryBuilder {
                indices: Vec::new(),
                prefix_suffix: Vec::new(),
            })
            .prefix_suffix
            .push((Box::from(suffix.as_str()), *idx));
    }

    if map.is_empty() {
        return (None, Box::default());
    }

    let mut sorted_keys: Vec<String> = map.keys().cloned().collect();
    sorted_keys.sort();

    #[allow(clippy::cast_possible_truncation)]
    let keyset: Vec<(&[u8], u32)> = sorted_keys
        .iter()
        .enumerate()
        .map(|(slot, key)| (key.as_bytes(), slot as u32))
        .collect();

    let entries: Vec<PrefixEntry> = sorted_keys
        .iter()
        .map(|key| {
            let builder = map.remove(key.as_str()).expect("key must exist");
            PrefixEntry {
                indices: builder.indices.into_boxed_slice(),
                prefix_suffix: builder.prefix_suffix.into_boxed_slice(),
            }
        })
        .collect();

    let da =
        DoubleArrayBuilder::build(&keyset).map(|bytes| DoubleArray::new(bytes.into_boxed_slice()));

    (da, entries.into_boxed_slice())
}

#[allow(clippy::type_complexity)]
fn build_suffix_da(
    strats: &strategy::Strategies,
) -> (Option<DoubleArray<Box<[u8]>>>, Box<[SuffixEntry]>) {
    let mut map: HashMap<Vec<u8>, SuffixEntryBuilder> = HashMap::new();

    for (suffix, idx) in &strats.suffixes {
        // Full suffix (with leading `/`) reversed -> ends_with match
        let reversed: Vec<u8> = suffix.as_bytes().iter().rev().copied().collect();
        map.entry(reversed)
            .or_insert_with(|| SuffixEntryBuilder {
                indices: Vec::new(),
                exact_indices: Vec::new(),
            })
            .indices
            .push(*idx);

        // Without leading `/` reversed -> exact path match
        let bare = &suffix[1..];
        let reversed_bare: Vec<u8> = bare.as_bytes().iter().rev().copied().collect();
        map.entry(reversed_bare)
            .or_insert_with(|| SuffixEntryBuilder {
                indices: Vec::new(),
                exact_indices: Vec::new(),
            })
            .exact_indices
            .push(*idx);
    }
    for (suffix, idx) in &strats.compound_suffixes {
        let reversed: Vec<u8> = suffix.as_bytes().iter().rev().copied().collect();
        map.entry(reversed)
            .or_insert_with(|| SuffixEntryBuilder {
                indices: Vec::new(),
                exact_indices: Vec::new(),
            })
            .indices
            .push(*idx);
    }

    if map.is_empty() {
        return (None, Box::default());
    }

    let mut sorted_keys: Vec<Vec<u8>> = map.keys().cloned().collect();
    sorted_keys.sort();

    #[allow(clippy::cast_possible_truncation)]
    let keyset: Vec<(&[u8], u32)> = sorted_keys
        .iter()
        .enumerate()
        .map(|(slot, key)| (key.as_slice(), slot as u32))
        .collect();

    let entries: Vec<SuffixEntry> = sorted_keys
        .iter()
        .map(|key| {
            let builder = map.remove(key).expect("key must exist");
            SuffixEntry {
                indices: builder.indices.into_boxed_slice(),
                exact_indices: builder.exact_indices.into_boxed_slice(),
            }
        })
        .collect();

    let da =
        DoubleArrayBuilder::build(&keyset).map(|bytes| DoubleArray::new(bytes.into_boxed_slice()));

    (da, entries.into_boxed_slice())
}

/// Build a `MatchEngine` from a list of glob patterns.
///
/// Classifies each pattern into the fastest applicable strategy and builds
/// an Aho-Corasick automaton for patterns that need full glob matching.
pub(crate) fn build_engine(patterns: Vec<Glob>) -> Result<MatchEngine, crate::error::Error> {
    let pat_strs: Vec<&str> = patterns.iter().map(Glob::glob).collect();
    let strats = strategy::build(&pat_strs);

    let mut ac_patterns: Vec<String> = Vec::new();
    let mut ac_to_glob: Vec<usize> = Vec::new();
    let mut always_check: Vec<usize> = Vec::new();

    for &idx in &strats.glob_indices {
        match literal::extract_literal(patterns[idx].glob()) {
            Some(lit) => {
                ac_patterns.push(String::from(lit));
                ac_to_glob.push(idx);
            }
            None => {
                always_check.push(idx);
            }
        }
    }

    let ac = if ac_patterns.is_empty() {
        None
    } else {
        Some(
            AhoCorasick::builder()
                .build(&ac_patterns)
                .map_err(|_| crate::error::Error::new(crate::error::ErrorKind::UnclosedClass))?,
        )
    };

    let freeze_index_map = |mut m: HashMap<String, Vec<usize>>| -> HashMap<String, Box<[usize]>> {
        m.shrink_to_fit();
        m.into_iter()
            .map(|(k, v)| (k, v.into_boxed_slice()))
            .collect()
    };

    let (prefix_da, prefix_entries) = build_prefix_da(&strats);
    let (suffix_da, suffix_entries) = build_suffix_da(&strats);

    let mut literals = strats.literals;
    literals.shrink_to_fit();

    Ok(MatchEngine {
        patterns: patterns.into_boxed_slice(),
        ext_any: freeze_index_map(strats.ext_any),
        ext_local: freeze_index_map(strats.ext_local),
        literals,
        prefix_da,
        prefix_entries,
        suffix_da,
        suffix_entries,
        ac,
        ac_to_glob: ac_to_glob.into_boxed_slice(),
        always_check: always_check.into_boxed_slice(),
    })
}
