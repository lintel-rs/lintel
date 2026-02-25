//! Compatibility tests sourced from the upstream globset crate by Andrew Gallant
//! (`BurntSushi`).
//! <https://github.com/BurntSushi/ripgrep/blob/master/crates/globset/src/glob.rs>
//! <https://github.com/BurntSushi/ripgrep/blob/master/crates/globset/src/lib.rs>
//!
//! These tests verify that glob-set produces the same matching results as globset
//! for both `GlobSet` and `TinyGlobSet`.
//!
//! ## Known behavioral differences
//!
//! glob-set treats `*` as NOT matching path separators (POSIX/gitignore semantics),
//! while globset's default allows `*` to cross `/`. Tests that depend on `*` crossing
//! `/` are marked `#[ignore]` with a note. Use `**` for recursive matching in glob-set.
//!
//! glob-set's `escape()` uses backslash escaping (`\*`) rather than globset's
//! character-class escaping (`[*]`). Both produce functionally equivalent patterns.

#![allow(clippy::unwrap_used)]

use glob_set::{Glob, GlobSet, GlobSetBuilder, TinyGlobSet, TinyGlobSetBuilder};

fn build_set(patterns: &[&str]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        builder.add(Glob::new(p).unwrap());
    }
    builder.build().unwrap()
}

fn build_tiny_set(patterns: &[&str]) -> TinyGlobSet {
    let mut builder = TinyGlobSetBuilder::new();
    for p in patterns {
        builder.add(Glob::new(p).unwrap());
    }
    builder.build().unwrap()
}

/// Assert that both `GlobSet` and `TinyGlobSet` match.
fn assert_match(pat: &str, path: &str) {
    let set = build_set(&[pat]);
    let tiny = build_tiny_set(&[pat]);
    assert!(
        set.is_match(path),
        "GlobSet: pattern {pat:?} should match {path:?}",
    );
    assert!(
        tiny.is_match(path),
        "TinyGlobSet: pattern {pat:?} should match {path:?}",
    );
}

/// Assert that both `GlobSet` and `TinyGlobSet` do NOT match.
fn assert_no_match(pat: &str, path: &str) {
    let set = build_set(&[pat]);
    let tiny = build_tiny_set(&[pat]);
    assert!(
        !set.is_match(path),
        "GlobSet: pattern {pat:?} should NOT match {path:?}",
    );
    assert!(
        !tiny.is_match(path),
        "TinyGlobSet: pattern {pat:?} should NOT match {path:?}",
    );
}

// ---- Positive matches (from upstream glob.rs) ----

#[test]
fn match_literal() {
    assert_match("a", "a");
}

#[test]
fn match_star_middle() {
    assert_match("a*b", "a_b");
}

#[test]
fn match_star_multi_1() {
    assert_match("a*b*c", "abc");
}

#[test]
fn match_star_multi_2() {
    assert_match("a*b*c", "a_b_c");
}

#[test]
fn match_star_multi_3() {
    assert_match("a*b*c", "a___b___c");
}

#[test]
fn match_star_repeated() {
    assert_match("abc*abc*abc", "abcabcabcabcabcabcabc");
}

#[test]
fn match_star_many() {
    assert_match("a*a*a*a*a*a*a*a*a", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
}

#[test]
fn match_star_class() {
    assert_match("a*b[xyz]c*d", "abxcdbxcddd");
}

#[test]
fn match_ext_dotfile() {
    assert_match("*.rs", ".rs");
}

#[test]
fn match_unicode() {
    assert_match("☃", "☃");
}

// ---- Recursive (**) matches ----

#[test]
fn matchrec_direct() {
    assert_match("some/**/needle.txt", "some/needle.txt");
}

#[test]
fn matchrec_one_level() {
    assert_match("some/**/needle.txt", "some/one/needle.txt");
}

#[test]
fn matchrec_two_levels() {
    assert_match("some/**/needle.txt", "some/one/two/needle.txt");
}

#[test]
fn matchrec_other() {
    assert_match("some/**/needle.txt", "some/other/needle.txt");
}

#[test]
fn matchrec_globstar_anything() {
    assert_match("**", "abcde");
}

#[test]
fn matchrec_globstar_empty() {
    assert_match("**", "");
}

#[test]
fn matchrec_globstar_dotfile() {
    assert_match("**", ".asdf");
}

#[test]
fn matchrec_globstar_deep() {
    assert_match("**", "/x/.asdf");
}

#[test]
fn matchrec_double_globstar_direct() {
    assert_match("some/**/**/needle.txt", "some/needle.txt");
}

#[test]
fn matchrec_double_globstar_one_level() {
    assert_match("some/**/**/needle.txt", "some/one/needle.txt");
}

#[test]
fn matchrec_double_globstar_two_levels() {
    assert_match("some/**/**/needle.txt", "some/one/two/needle.txt");
}

#[test]
fn matchrec_double_globstar_other() {
    assert_match("some/**/**/needle.txt", "some/other/needle.txt");
}

#[test]
fn matchrec_prefix_deep() {
    assert_match("**/test", "one/two/test");
}

#[test]
fn matchrec_prefix_one() {
    assert_match("**/test", "one/test");
}

#[test]
fn matchrec_prefix_bare() {
    assert_match("**/test", "test");
}

#[test]
fn matchrec_abs_deep() {
    assert_match("/**/test", "/one/two/test");
}

#[test]
fn matchrec_abs_one() {
    assert_match("/**/test", "/one/test");
}

#[test]
fn matchrec_abs_bare() {
    assert_match("/**/test", "/test");
}

#[test]
fn matchrec_dotfile_bare() {
    assert_match("**/.*", ".abc");
}

#[test]
fn matchrec_dotfile_nested() {
    assert_match("**/.*", "abc/.abc");
}

#[test]
fn matchrec_prefix_path() {
    assert_match("**/foo/bar", "foo/bar");
}

#[test]
fn matchrec_dotprefix() {
    assert_match(".*/**", ".abc/abc");
}

#[test]
fn matchrec_trailing_slash() {
    assert_match("test/**", "test/");
}

#[test]
fn matchrec_trailing_one() {
    assert_match("test/**", "test/one");
}

#[test]
fn matchrec_trailing_deep() {
    assert_match("test/**", "test/one/two");
}

#[test]
fn matchrec_single_star() {
    assert_match("some/*/needle.txt", "some/one/needle.txt");
}

// ---- Range/class matches ----

#[test]
fn matchrange_digit_low() {
    assert_match("a[0-9]b", "a0b");
}

#[test]
fn matchrange_digit_high() {
    assert_match("a[0-9]b", "a9b");
}

#[test]
fn matchrange_negated() {
    assert_match("a[!0-9]b", "a_b");
}

#[test]
fn matchrange_mixed() {
    assert_match("[a-z123]", "1");
}

#[test]
fn matchrange_mixed_prefix() {
    assert_match("[1a-z23]", "1");
}

#[test]
fn matchrange_mixed_suffix() {
    assert_match("[123a-z]", "1");
}

#[test]
fn matchrange_dash_suffix() {
    assert_match("[abc-]", "-");
}

#[test]
fn matchrange_dash_prefix() {
    assert_match("[-abc]", "-");
}

#[test]
fn matchrange_dash_range() {
    assert_match("[-a-c]", "b");
}

#[test]
fn matchrange_range_dash() {
    assert_match("[a-c-]", "b");
}

#[test]
fn matchrange_dash_only() {
    assert_match("[-]", "-");
}

#[test]
fn matchrange_caret_negation() {
    assert_match("a[^0-9]b", "a_b");
}

// ---- Pattern matches ----
//
// In globset, `*` can cross path separators by default. In glob-set, `*` never
// crosses `/` (use `**` for recursive matching). Tests where `*` must cross `/`
// are marked #[ignore].

#[test]
fn matchpat_hello_exact() {
    assert_match("*hello.txt", "hello.txt");
}

#[test]
fn matchpat_hello_prefix() {
    assert_match("*hello.txt", "gareth_says_hello.txt");
}

#[test]
#[ignore = "glob-set: * does not cross path separators"]
fn matchpat_hello_nested() {
    assert_match("*hello.txt", "some/path/to/hello.txt");
}

#[test]
#[ignore = "glob-set: * does not cross path separators"]
fn matchpat_hello_backslash() {
    assert_match("*hello.txt", "some\\path\\to\\hello.txt");
}

#[test]
#[ignore = "glob-set: * does not cross path separators"]
fn matchpat_hello_absolute() {
    assert_match("*hello.txt", "/an/absolute/path/to/hello.txt");
}

#[test]
fn matchpat_star_path() {
    assert_match("*some/path/to/hello.txt", "some/path/to/hello.txt");
}

#[test]
#[ignore = "glob-set: * does not cross path separators"]
fn matchpat_star_deeper() {
    assert_match("*some/path/to/hello.txt", "a/bigger/some/path/to/hello.txt");
}

// ---- Escape matches ----

#[test]
fn matchescape() {
    assert_match("_[[]_[]]_[?]_[*]_!_", "_[_]_?_*_!_");
}

// ---- Alternates ----

#[test]
fn matchalt_literal_comma() {
    assert_match("a,b", "a,b");
}

#[test]
fn matchalt_comma() {
    assert_match(",", ",");
}

#[test]
fn matchalt_a() {
    assert_match("{a,b}", "a");
}

#[test]
fn matchalt_b() {
    assert_match("{a,b}", "b");
}

#[test]
fn matchalt_globstar_src() {
    assert_match("{**/src/**,foo}", "abc/src/bar");
}

#[test]
fn matchalt_globstar_foo() {
    assert_match("{**/src/**,foo}", "foo");
}

#[test]
fn matchalt_bracket_in_alt() {
    assert_match("{[}],foo}", "}");
}

#[test]
fn matchalt_single() {
    assert_match("{foo}", "foo");
}

#[test]
fn matchalt_empty() {
    assert_match("{}", "");
}

#[test]
fn matchalt_double_comma() {
    assert_match("{,}", "");
}

#[test]
fn matchalt_multi_ext_foo() {
    assert_match("{*.foo,*.bar,*.wat}", "test.foo");
}

#[test]
fn matchalt_multi_ext_bar() {
    assert_match("{*.foo,*.bar,*.wat}", "test.bar");
}

#[test]
fn matchalt_multi_ext_wat() {
    assert_match("{*.foo,*.bar,*.wat}", "test.wat");
}

#[test]
fn matchalt_optional_ext() {
    assert_match("foo{,.txt}", "foo.txt");
}

#[test]
fn matchalt_nested_bc() {
    assert_match("{a,b{c,d}}", "bc");
}

#[test]
fn matchalt_nested_bd() {
    assert_match("{a,b{c,d}}", "bd");
}

#[test]
fn matchalt_nested_a() {
    assert_match("{a,b{c,d}}", "a");
}

// ---- Negative matches (from upstream glob.rs) ----

#[test]
fn nomatch_star_trailing() {
    assert_no_match("a*b*c", "abcd");
}

#[test]
fn nomatch_star_repeat_extra() {
    assert_no_match("abc*abc*abc", "abcabcabcabcabcabcabca");
}

#[test]
fn nomatch_rec_different() {
    assert_no_match("some/**/needle.txt", "some/other/notthis.txt");
}

#[test]
fn nomatch_double_rec_different() {
    assert_no_match("some/**/**/needle.txt", "some/other/notthis.txt");
}

#[test]
fn nomatch_abs_bare() {
    assert_no_match("/**/test", "test");
}

#[test]
fn nomatch_abs_different() {
    assert_no_match("/**/test", "/one/notthis");
}

#[test]
fn nomatch_abs_not() {
    assert_no_match("/**/test", "/notthis");
}

#[test]
fn nomatch_dotfile_mid() {
    assert_no_match("**/.*", "ab.c");
}

#[test]
fn nomatch_dotfile_nested_mid() {
    assert_no_match("**/.*", "abc/ab.c");
}

#[test]
fn nomatch_dotprefix_bare() {
    assert_no_match(".*/**", "a.bc");
}

#[test]
fn nomatch_dotprefix_nested() {
    assert_no_match(".*/**", "abc/a.bc");
}

#[test]
fn nomatch_range_digit_underscore() {
    assert_no_match("a[0-9]b", "a_b");
}

#[test]
fn nomatch_negrange_0() {
    assert_no_match("a[!0-9]b", "a0b");
}

#[test]
fn nomatch_negrange_9() {
    assert_no_match("a[!0-9]b", "a9b");
}

#[test]
fn nomatch_negdash() {
    assert_no_match("[!-]", "-");
}

#[test]
fn nomatch_star_extra() {
    assert_no_match("*hello.txt", "hello.txt-and-then-some");
}

#[test]
fn nomatch_star_different() {
    assert_no_match("*hello.txt", "goodbye.txt");
}

#[test]
fn nomatch_star_path_extra() {
    assert_no_match(
        "*some/path/to/hello.txt",
        "some/path/to/hello.txt-and-then-some",
    );
}

#[test]
fn nomatch_star_different_path() {
    assert_no_match("*some/path/to/hello.txt", "some/other/path/to/hello.txt");
}

#[test]
fn nomatch_literal_nested() {
    assert_no_match("a", "foo/a");
}

#[test]
fn nomatch_dot_slash() {
    assert_no_match("./foo", "foo");
}

#[test]
fn nomatch_globstar_partial() {
    assert_no_match("**/foo", "foofoo");
}

#[test]
fn nomatch_globstar_partial_bar() {
    assert_no_match("**/foo/bar", "foofoo/bar");
}

#[test]
fn nomatch_abs_ext() {
    assert_no_match("/*.c", "mozilla-sha1/sha1.c");
}

#[test]
fn nomatch_caret_range_0() {
    assert_no_match("a[^0-9]b", "a0b");
}

#[test]
fn nomatch_caret_range_9() {
    assert_no_match("a[^0-9]b", "a9b");
}

#[test]
fn nomatch_caret_dash() {
    assert_no_match("[^-]", "-");
}

#[test]
fn nomatch_single_star_direct() {
    assert_no_match("some/*/needle.txt", "some/needle.txt");
}

#[test]
fn nomatch_dotprefix_no_slash() {
    assert_no_match(".*/**", ".abc");
}

#[test]
fn nomatch_trailing_no_slash() {
    assert_no_match("foo/**", "foo");
}

// ---- GlobSet-level tests (from upstream lib.rs) ----

#[test]
fn set_works() {
    // Using `**/*.c` instead of `*.c` because glob-set's `*` does not cross `/`.
    // The upstream test uses `*.c` which matches `src/foo.c` in globset because
    // globset's `*` crosses path separators by default.
    let set = build_set(&["src/**/*.rs", "**/*.c", "src/lib.rs"]);
    let tiny = build_tiny_set(&["src/**/*.rs", "**/*.c", "src/lib.rs"]);

    assert!(set.is_match("foo.c"));
    assert!(tiny.is_match("foo.c"));

    assert!(set.is_match("src/foo.c"));
    assert!(tiny.is_match("src/foo.c"));

    assert!(!set.is_match("foo.rs"));
    assert!(!tiny.is_match("foo.rs"));

    assert!(!set.is_match("tests/foo.rs"));
    assert!(!tiny.is_match("tests/foo.rs"));

    assert!(set.is_match("src/foo.rs"));
    assert!(tiny.is_match("src/foo.rs"));

    assert!(set.is_match("src/grep/src/main.rs"));
    assert!(tiny.is_match("src/grep/src/main.rs"));

    let matches = set.matches("src/lib.rs");
    assert_eq!(2, matches.len());
    assert!(matches.contains(&0));
    assert!(matches.contains(&2));

    let tiny_matches = tiny.matches("src/lib.rs");
    assert_eq!(2, tiny_matches.len());
    assert!(tiny_matches.contains(&0));
    assert!(tiny_matches.contains(&2));
}

// Original upstream test preserved verbatim to document the behavioral difference.
#[test]
#[ignore = "glob-set: * does not cross path separators (use **/*.c instead of *.c)"]
fn set_works_upstream_verbatim() {
    let set = build_set(&["src/**/*.rs", "*.c", "src/lib.rs"]);
    assert!(set.is_match("src/foo.c")); // fails: *.c doesn't cross /
}

#[test]
fn empty_set_works() {
    let set = build_set(&[]);
    let tiny = build_tiny_set(&[]);

    assert!(!set.is_match(""));
    assert!(!tiny.is_match(""));

    assert!(!set.is_match("a"));
    assert!(!tiny.is_match("a"));
}

#[test]
fn default_set_works() {
    let set = GlobSet::default();
    assert!(!set.is_match(""));
    assert!(!set.is_match("a"));

    let tiny = TinyGlobSet::default();
    assert!(!tiny.is_match(""));
    assert!(!tiny.is_match("a"));
}

#[test]
fn set_does_not_remember() {
    let set = build_set(&["*foo*", "*bar*", "*quux*"]);
    let tiny = build_tiny_set(&["*foo*", "*bar*", "*quux*"]);

    let matches = set.matches("ZfooZquuxZ");
    assert_eq!(2, matches.len());
    assert_eq!(0, matches[0]);
    assert_eq!(2, matches[1]);

    let matches = set.matches("nada");
    assert_eq!(0, matches.len());

    let tiny_matches = tiny.matches("ZfooZquuxZ");
    assert_eq!(2, tiny_matches.len());
    assert!(tiny_matches.contains(&0));
    assert!(tiny_matches.contains(&2));

    let tiny_matches = tiny.matches("nada");
    assert_eq!(0, tiny_matches.len());
}

// ---- escape() tests ----
//
// glob-set uses backslash escaping (\*) while globset uses character class escaping ([*]).
// Both are valid; the escaped patterns match the same strings.

#[test]
fn escape_compat() {
    use glob_set::escape;
    assert_eq!("foo", escape("foo"));
    assert_eq!("foo\\*", escape("foo*"));
    assert_eq!("\\[\\]", escape("[]"));
    assert_eq!("\\*\\?", escape("*?"));
    assert_eq!("src/\\*\\*/\\*.rs", escape("src/**/*.rs"));
    assert_eq!("bar\\[ab\\]baz", escape("bar[ab]baz"));
    assert_eq!("bar\\[\\!\\!\\]\\!baz", escape("bar[!!]!baz"));
}

#[test]
fn escape_round_trip() {
    use glob_set::escape;
    // The important thing is that escaped patterns match the original string literally.
    let cases = ["foo*", "[]", "*?", "src/**/*.rs", "bar[ab]baz"];
    for s in cases {
        let escaped = escape(s);
        let glob = Glob::new(&escaped).unwrap();
        let matcher = glob.compile_matcher();
        assert!(
            matcher.is_match(s),
            "escape({s:?}) = {escaped:?} should match the original",
        );
    }
}
