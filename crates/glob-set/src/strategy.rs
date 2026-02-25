//! Build-time pattern classification for fast-path matching.
//!
//! Each glob is analysed once and assigned to the cheapest strategy that can
//! decide a match:
//!
//! | Strategy | Example pattern | Match cost |
//! |----------|----------------|------------|
//! | Extension | `*.rs`, `**/*.rs` | hash lookup on file extension |
//! | Prefix | `src/**` | `starts_with` |
//! | Suffix | `**/foo.txt`, `**/*.test.js` | `ends_with` |
//! | Literal | `Cargo.toml` | hash lookup |
//! | Glob | everything else | AC pre-filter + `glob_match` |
//!
//! Patterns containing `{a,b}` brace alternations are expanded at build time
//! so that each alternative can be classified independently.

use alloc::string::String;
use alloc::vec::Vec;

use hashbrown::HashMap;

/// The strategy chosen for a single pattern at build time.
#[derive(Debug)]
pub(crate) enum PatternStrategy {
    /// `**/*.ext` — any path with this extension matches. No verification needed.
    ExtensionAny(String),
    /// `*.ext` — extension matches only in the current directory (no `/` allowed).
    /// Still needs `glob_match` verification.
    ExtensionLocal(String),
    /// No wildcards at all — exact string match.
    Literal(String),
    /// `prefix/**` — match by prefix.
    Prefix(String),
    /// `**/suffix` — match by `ends_with` + bare-name fallback.
    /// Suffix has a leading `/`, e.g. `/foo.txt`.
    Suffix(String),
    /// `**/*<literal>` — match by `ends_with` only.
    /// Suffix has no leading `/`, e.g. `.test.js`.
    CompoundSuffix(String),
    /// `prefix/**/*.ext` or `prefix/**/*<literal>` — `starts_with` + `ends_with`.
    PrefixSuffix { prefix: String, suffix: String },
    /// Needs the full glob engine.
    Glob,
}

/// Classify a validated glob pattern into its optimal strategy.
pub(crate) fn classify(pattern: &str) -> PatternStrategy {
    let bytes = pattern.as_bytes();

    // No wildcard characters at all → literal.
    if !bytes
        .iter()
        .any(|&b| matches!(b, b'*' | b'?' | b'[' | b'{' | b'\\'))
    {
        return PatternStrategy::Literal(String::from(pattern));
    }

    // Try extension: `*.ext` or `**/*.ext`
    if let Some((ext, any_depth)) = extract_extension(pattern) {
        return if any_depth {
            PatternStrategy::ExtensionAny(ext)
        } else {
            PatternStrategy::ExtensionLocal(ext)
        };
    }

    // Try prefix: `prefix/**`  (with no other wildcards in the prefix part)
    if let Some(prefix) = extract_prefix(pattern) {
        return PatternStrategy::Prefix(prefix);
    }

    // Try suffix: `**/suffix` (with no wildcards in the suffix part)
    if let Some(suffix) = extract_basename_suffix(pattern) {
        return PatternStrategy::Suffix(suffix);
    }

    // Try compound suffix: `**/*<literal>` (e.g. `**/*.test.js`)
    if let Some(suffix) = extract_compound_suffix(pattern) {
        return PatternStrategy::CompoundSuffix(suffix);
    }

    // Try prefix+suffix: `prefix/**/*.ext` or `prefix/**/*<literal>`
    if let Some((prefix, suffix)) = extract_prefix_suffix(pattern) {
        return PatternStrategy::PrefixSuffix { prefix, suffix };
    }

    PatternStrategy::Glob
}

// ---------------------------------------------------------------------------
// Extension extraction
// ---------------------------------------------------------------------------

/// If pattern matches `*.ext` or `**/*.ext` (no wildcards/specials in ext),
/// return `(extension, any_depth)` where `any_depth` is true for `**/*.ext`
/// (matches at any depth) and false for `*.ext` (current dir only).
///
/// Only matches patterns where the entire part after the wildcard prefix is a
/// simple extension (single dot + literal). Patterns like `*.test.js` are
/// rejected because the extension alone (`.js`) isn't sufficient.
fn extract_extension(pattern: &str) -> Option<(String, bool)> {
    let bytes = pattern.as_bytes();

    // Find the last dot.
    let dot = bytes.iter().rposition(|&b| b == b'.')?;

    // Extension part (after the dot) must be pure literal — no wildcards.
    let ext_bytes = &bytes[dot + 1..];
    if ext_bytes.is_empty() {
        return None;
    }
    if ext_bytes.iter().any(|&b| is_special(b)) {
        return None;
    }

    // Part before the dot must be a wildcard-only prefix:
    //   `*`, `**/*`, or `**/` segments followed by `*`.
    let prefix = &bytes[..dot];
    if !is_pure_wildcard_prefix(prefix) {
        return None;
    }

    // Reject multi-dot extensions like `*.test.js` — the part between the
    // wildcard and the last dot contains a literal dot, so extension-only
    // matching would be incorrect.
    let prefix_end = wildcard_prefix_len(prefix);
    if prefix_end < dot {
        return None;
    }

    // `**/*.ext` → any_depth=true, `*.ext` → any_depth=false.
    let any_depth = prefix != [b'*'];

    // Build extension string with the dot.
    let mut ext = String::with_capacity(ext_bytes.len() + 1);
    ext.push('.');
    for &b in ext_bytes {
        ext.push(b as char);
    }
    Some((ext, any_depth))
}

fn wildcard_prefix_len(prefix: &[u8]) -> usize {
    match prefix {
        [b'*'] => 1,
        _ if prefix.ends_with(b"**/*") => prefix.len(),
        _ => 0,
    }
}

fn is_pure_wildcard_prefix(prefix: &[u8]) -> bool {
    match prefix {
        [b'*'] => true,
        _ if prefix.ends_with(b"**/*") => {
            let before = &prefix[..prefix.len() - 4];
            before.is_empty() || is_globstar_segments(before)
        }
        _ => false,
    }
}

fn is_globstar_segments(bytes: &[u8]) -> bool {
    if !bytes.len().is_multiple_of(3) {
        return false;
    }
    bytes.chunks(3).all(|chunk| chunk == b"**/")
}

// ---------------------------------------------------------------------------
// Prefix extraction
// ---------------------------------------------------------------------------

/// If pattern is `prefix/**` (literal prefix, no wildcards), return the prefix
/// with trailing slash.
fn extract_prefix(pattern: &str) -> Option<String> {
    let bytes = pattern.as_bytes();

    if !bytes.ends_with(b"/**") {
        return None;
    }

    let prefix = &bytes[..bytes.len() - 3];

    if prefix.is_empty() || prefix.iter().any(|&b| is_special(b)) {
        return None;
    }

    let mut s = String::with_capacity(prefix.len() + 1);
    for &b in prefix {
        s.push(b as char);
    }
    s.push('/');
    Some(s)
}

// ---------------------------------------------------------------------------
// Suffix extraction
// ---------------------------------------------------------------------------

/// If pattern is `**/suffix` where suffix is purely literal, return the suffix
/// with a leading slash (e.g. `/foo.txt` for `**/foo.txt`).
fn extract_basename_suffix(pattern: &str) -> Option<String> {
    let bytes = pattern.as_bytes();

    if !bytes.starts_with(b"**/") {
        return None;
    }

    let suffix = &bytes[3..];

    if suffix.is_empty() || suffix.iter().any(|&b| is_special(b)) {
        return None;
    }

    let mut s = String::with_capacity(suffix.len() + 1);
    s.push('/');
    for &b in suffix {
        s.push(b as char);
    }
    Some(s)
}

/// If pattern is `**/*<literal>` (e.g. `**/*.test.js`), return the literal
/// suffix (e.g. `.test.js`). No leading slash — `ends_with` alone is correct.
fn extract_compound_suffix(pattern: &str) -> Option<String> {
    let bytes = pattern.as_bytes();

    // Must start with `**/*`
    if !bytes.starts_with(b"**/*") {
        return None;
    }

    let rest = &bytes[4..]; // after `**/*`

    // Rest must be non-empty and purely literal.
    if rest.is_empty() || rest.iter().any(|&b| is_special(b)) {
        return None;
    }

    let mut s = String::with_capacity(rest.len());
    for &b in rest {
        s.push(b as char);
    }
    Some(s)
}

// ---------------------------------------------------------------------------
// Prefix + suffix extraction
// ---------------------------------------------------------------------------

/// If pattern is `prefix/**/*.ext` or `prefix/**/*<literal>` (where prefix is
/// purely literal), return `(prefix_with_slash, suffix)`.
///
/// The prefix includes a trailing `/` and the suffix is the literal tail after
/// `**/*` (e.g. `.js`, `.test.ts`). Match by `starts_with(prefix) && ends_with(suffix)`.
fn extract_prefix_suffix(pattern: &str) -> Option<(String, String)> {
    let bytes = pattern.as_bytes();

    // Find `/**/*` in the pattern.
    let marker = b"/**/*";
    let pos = bytes.windows(marker.len()).position(|w| w == marker)?;

    // Prefix must be non-empty and purely literal.
    let prefix_bytes = &bytes[..pos];
    if prefix_bytes.is_empty() || prefix_bytes.iter().any(|&b| is_special(b)) {
        return None;
    }

    // Suffix (after `/**/*`) must be non-empty and purely literal.
    let suffix_bytes = &bytes[pos + marker.len()..];
    if suffix_bytes.is_empty() || suffix_bytes.iter().any(|&b| is_special(b)) {
        return None;
    }

    let mut prefix = String::with_capacity(prefix_bytes.len() + 1);
    for &b in prefix_bytes {
        prefix.push(b as char);
    }
    prefix.push('/');

    let mut suffix = String::with_capacity(suffix_bytes.len());
    for &b in suffix_bytes {
        suffix.push(b as char);
    }

    Some((prefix, suffix))
}

// ---------------------------------------------------------------------------
// Brace expansion
// ---------------------------------------------------------------------------

/// Expand `{a,b}` alternations in a glob pattern.
///
/// Returns a list of expanded patterns. If there are no braces, returns a
/// single-element list with the original pattern. Handles nested braces
/// and multiple brace groups.
pub(crate) fn expand_braces(pattern: &str) -> Vec<String> {
    let bytes = pattern.as_bytes();

    // Find the first unescaped `{`.
    let mut i = 0;
    let open = loop {
        if i >= bytes.len() {
            return alloc::vec![String::from(pattern)];
        }
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'{' {
            break i;
        }
        i += 1;
    };

    // Find the matching `}`.
    let mut depth: u32 = 1;
    i = open + 1;
    let close = loop {
        if i >= bytes.len() {
            // Unclosed brace — don't expand.
            return alloc::vec![String::from(pattern)];
        }
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    break i;
                }
            }
            _ => {}
        }
        i += 1;
    };

    let prefix = &pattern[..open];
    let suffix = &pattern[close + 1..];
    let inner = &pattern[open + 1..close];

    let alternatives = split_brace_alternatives(inner);

    let mut results = Vec::new();
    for alt in alternatives {
        let expanded = alloc::format!("{prefix}{alt}{suffix}");
        // Recursively expand in case there are more brace groups.
        results.extend(expand_braces(&expanded));
    }
    results
}

/// Split a brace interior by top-level commas (respecting nested braces).
fn split_brace_alternatives(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth: u32 = 0;
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&s[start..]);
    parts
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_special(b: u8) -> bool {
    matches!(b, b'*' | b'?' | b'[' | b'{' | b'\\')
}

/// Extract the file extension (with leading dot) from a path.
///
/// Returns `None` if there is no extension or the extension is empty.
pub(crate) fn path_extension(path: &str) -> Option<&str> {
    let last_sep = path.rfind('/').map_or(0, |i| i + 1);
    let basename = &path[last_sep..];
    let dot = basename.rfind('.')?;
    if dot + 1 >= basename.len() {
        return None;
    }
    Some(&basename[dot..])
}

// ---------------------------------------------------------------------------
// Strategy builder
// ---------------------------------------------------------------------------

/// Collected fast-path strategies built from a set of patterns.
#[derive(Debug)]
pub(crate) struct Strategies {
    /// Extension → glob indices where extension alone is sufficient (`**/*.ext`).
    pub ext_any: HashMap<String, Vec<usize>>,
    /// Extension → glob indices that need `glob_match` verification (`*.ext`).
    pub ext_local: HashMap<String, Vec<usize>>,
    /// Literal path → glob index.
    pub literals: HashMap<String, usize>,
    /// Prefix strings (with trailing `/`) → glob index.
    pub prefixes: Vec<(String, usize)>,
    /// Basename suffixes (with leading `/`) → glob index. Check `ends_with` + bare match.
    pub suffixes: Vec<(String, usize)>,
    /// Compound suffixes (no leading `/`) → glob index. Check `ends_with` only.
    pub compound_suffixes: Vec<(String, usize)>,
    /// Prefix+suffix pairs → glob index. Check `starts_with` + `ends_with`.
    pub prefix_suffixes: Vec<(String, String, usize)>,
    /// Indices of patterns that need full glob matching.
    pub glob_indices: Vec<usize>,
}

/// Build strategies from a list of patterns (by glob string).
///
/// Patterns with `{a,b}` braces are expanded before classification so each
/// alternative can be assigned to the fastest strategy independently.
pub(crate) fn build(patterns: &[&str]) -> Strategies {
    let mut ext_any: HashMap<String, Vec<usize>> = HashMap::new();
    let mut ext_local: HashMap<String, Vec<usize>> = HashMap::new();
    let mut literals = HashMap::new();
    let mut prefixes = Vec::new();
    let mut suffixes = Vec::new();
    let mut compound_suffixes = Vec::new();
    let mut prefix_suffixes = Vec::new();
    let mut glob_indices = Vec::new();

    for (i, pat) in patterns.iter().enumerate() {
        // Expand braces: `{a,b}/**/*.rs` → `a/**/*.rs`, `b/**/*.rs`.
        // Each expanded variant maps back to the original pattern index `i`.
        let expanded = expand_braces(pat);
        let variants: Vec<&str> = expanded.iter().map(String::as_str).collect();

        // Classify each variant. If ALL variants resolve to fast strategies,
        // register them. Otherwise, fall back to Glob for the whole pattern.
        let mut all_fast = true;
        let mut pending: Vec<PatternStrategy> = Vec::new();

        for variant in &variants {
            let strat = classify(variant);
            if matches!(strat, PatternStrategy::Glob) {
                all_fast = false;
                break;
            }
            pending.push(strat);
        }

        if !all_fast || pending.is_empty() {
            glob_indices.push(i);
            continue;
        }

        for strat in pending {
            match strat {
                PatternStrategy::ExtensionAny(ext) => {
                    ext_any.entry(ext).or_default().push(i);
                }
                PatternStrategy::ExtensionLocal(ext) => {
                    ext_local.entry(ext).or_default().push(i);
                }
                PatternStrategy::Literal(lit) => {
                    literals.insert(lit, i);
                }
                PatternStrategy::Prefix(pfx) => {
                    prefixes.push((pfx, i));
                }
                PatternStrategy::Suffix(sfx) => {
                    suffixes.push((sfx, i));
                }
                PatternStrategy::CompoundSuffix(sfx) => {
                    compound_suffixes.push((sfx, i));
                }
                PatternStrategy::PrefixSuffix { prefix, suffix } => {
                    prefix_suffixes.push((prefix, suffix, i));
                }
                PatternStrategy::Glob => unreachable!(),
            }
        }
    }

    Strategies {
        ext_any,
        ext_local,
        literals,
        prefixes,
        suffixes,
        compound_suffixes,
        prefix_suffixes,
        glob_indices,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use alloc::vec;

    use super::*;

    // -- classify tests --

    #[test]
    fn classify_extension_star() {
        assert!(matches!(classify("*.rs"), PatternStrategy::ExtensionLocal(ref e) if e == ".rs"));
    }

    #[test]
    fn classify_extension_globstar() {
        assert!(matches!(classify("**/*.rs"), PatternStrategy::ExtensionAny(ref e) if e == ".rs"));
    }

    #[test]
    fn classify_multi_dot_compound_suffix() {
        assert!(
            matches!(classify("**/*.test.js"), PatternStrategy::CompoundSuffix(ref s) if s == ".test.js")
        );
    }

    #[test]
    fn classify_star_multi_dot_is_glob() {
        // `*.test.js` (no `**/`) — can't be suffix, falls to Glob.
        assert!(matches!(classify("*.test.js"), PatternStrategy::Glob));
    }

    #[test]
    fn classify_literal() {
        assert!(
            matches!(classify("Cargo.toml"), PatternStrategy::Literal(ref s) if s == "Cargo.toml")
        );
    }

    #[test]
    fn classify_prefix() {
        assert!(matches!(classify("src/**"), PatternStrategy::Prefix(ref s) if s == "src/"));
    }

    #[test]
    fn classify_suffix() {
        assert!(
            matches!(classify("**/foo.txt"), PatternStrategy::Suffix(ref s) if s == "/foo.txt")
        );
    }

    #[test]
    fn classify_complex_as_glob() {
        assert!(matches!(
            classify("{src,lib}/**/*.rs"),
            PatternStrategy::Glob
        ));
    }

    #[test]
    fn classify_question_mark_as_glob() {
        assert!(matches!(classify("a?c"), PatternStrategy::Glob));
    }

    #[test]
    fn classify_char_class_as_glob() {
        assert!(matches!(classify("[abc].txt"), PatternStrategy::Glob));
    }

    #[test]
    fn classify_prefix_suffix() {
        assert!(matches!(
            classify("src/**/*.rs"),
            PatternStrategy::PrefixSuffix { ref prefix, ref suffix }
                if prefix == "src/" && suffix == ".rs"
        ));
    }

    #[test]
    fn classify_prefix_compound_suffix() {
        assert!(matches!(
            classify("tests/**/*.test.ts"),
            PatternStrategy::PrefixSuffix { ref prefix, ref suffix }
                if prefix == "tests/" && suffix == ".test.ts"
        ));
    }

    // -- brace expansion tests --

    #[test]
    fn expand_no_braces() {
        assert_eq!(expand_braces("*.rs"), vec!["*.rs"]);
    }

    #[test]
    fn expand_simple_braces() {
        let mut result = expand_braces("{a,b}.rs");
        result.sort();
        assert_eq!(result, vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn expand_braces_with_glob() {
        let mut result = expand_braces("{src,lib}/**/*.rs");
        result.sort();
        assert_eq!(result, vec!["lib/**/*.rs", "src/**/*.rs"]);
    }

    #[test]
    fn expand_nested_braces() {
        let mut result = expand_braces("{a,{b,c}}");
        result.sort();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn expand_multiple_brace_groups() {
        let mut result = expand_braces("{a,b}.{c,d}");
        result.sort();
        assert_eq!(result, vec!["a.c", "a.d", "b.c", "b.d"]);
    }

    #[test]
    fn expand_unclosed_brace_no_expansion() {
        assert_eq!(expand_braces("{a,b"), vec!["{a,b"]);
    }

    // -- brace expansion → strategy classification --

    #[test]
    fn build_expands_braces_into_fast_paths() {
        // `{src,lib}/**/*.rs` → `src/**/*.rs` + `lib/**/*.rs` → both PrefixSuffix.
        let s = build(&["{src,lib}/**/*.rs"]);
        assert_eq!(s.prefix_suffixes.len(), 2);
        assert!(s.glob_indices.is_empty());
    }

    #[test]
    fn build_expands_braces_simple() {
        // `{src,lib}/**` → `src/**` + `lib/**` → both are Prefix.
        let s = build(&["{src,lib}/**"]);
        assert_eq!(s.prefixes.len(), 2);
        assert!(s.glob_indices.is_empty());
    }

    #[test]
    fn build_expands_extension_braces() {
        // `*.{rs,toml}` → `*.rs` + `*.toml` → both ExtensionLocal.
        let s = build(&["*.{rs,toml}"]);
        assert_eq!(s.ext_local.len(), 2);
        assert!(s.glob_indices.is_empty());
    }

    #[test]
    fn build_expands_globstar_extension_braces() {
        // `**/*.{rs,toml}` → `**/*.rs` + `**/*.toml` → both ExtensionAny.
        let s = build(&["**/*.{rs,toml}"]);
        assert_eq!(s.ext_any.len(), 2);
        assert!(s.glob_indices.is_empty());
    }

    // -- compound suffix tests --

    #[test]
    fn build_compound_suffix() {
        let s = build(&["**/*.test.js"]);
        assert_eq!(s.compound_suffixes.len(), 1);
        assert_eq!(s.compound_suffixes[0].0, ".test.js");
        assert!(s.glob_indices.is_empty());
    }

    // -- path_extension tests --

    #[test]
    fn path_ext_simple() {
        assert_eq!(path_extension("foo.rs"), Some(".rs"));
    }

    #[test]
    fn path_ext_nested() {
        assert_eq!(path_extension("src/main.rs"), Some(".rs"));
    }

    #[test]
    fn path_ext_none() {
        assert_eq!(path_extension("Makefile"), None);
    }

    #[test]
    fn path_ext_dotfile() {
        assert_eq!(path_extension(".gitignore"), Some(".gitignore"));
    }

    #[test]
    fn path_ext_multi_dot() {
        assert_eq!(path_extension("foo.test.js"), Some(".js"));
    }

    // -- build counts --

    #[test]
    fn build_strategy_counts() {
        let s = build(&[
            "**/foo.txt",     // suffix
            "*.rs",           // ext_local
            "**/*.toml",      // ext_any
            "src/**",         // prefix
            "Cargo.toml",     // literal
            "**/*.test.ts",   // compound_suffix
            "docs/**/*.html", // prefix_suffix
        ]);
        assert_eq!(s.suffixes.len(), 1); // **/foo.txt
        assert_eq!(s.ext_local.len(), 1); // *.rs
        assert_eq!(s.ext_any.len(), 1); // **/*.toml
        assert_eq!(s.prefixes.len(), 1); // src/**
        assert_eq!(s.literals.len(), 1); // Cargo.toml
        assert_eq!(s.compound_suffixes.len(), 1); // **/*.test.ts
        assert_eq!(s.prefix_suffixes.len(), 1); // docs/**/*.html
        assert!(s.glob_indices.is_empty());
    }
}
