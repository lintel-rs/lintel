mod snapshot_parser;

use prettier_jsonc::{JsonFormat, format_str};
use std::collections::HashSet;
use std::path::Path;

/// Known test failures with explanations.
///
/// Each entry is a `(dir, test_name, reason)` tuple. Tests listed here are
/// expected to fail and won't cause CI failures. If a listed test starts
/// passing, the test harness will flag it as a stale exclusion so we can
/// remove it.
const KNOWN_FAILURES: &[(&str, &str, &str)] = &[
    // ── JSON5/JSON6 syntax in json/json fixtures ─────────────────────────
    // These fixtures contain JSON5-only syntax (unquoted keys, hex numbers,
    // positive number prefix `+`, etc.) that is not valid JSONC. The JSONC
    // parser correctly rejects them. They are tested by prettier-json5 instead.
    (
        "json/json",
        "json5.json - {\"trailingComma\":\"all\"} format 1",
        "JSON5 syntax not valid JSONC",
    ),
    (
        "json/json",
        "json5.json - {\"trailingComma\":\"es5\"} format 1",
        "JSON5 syntax not valid JSONC",
    ),
    (
        "json/json",
        "json6.json - {\"trailingComma\":\"all\"} format 1",
        "JSON6 syntax not valid JSONC",
    ),
    (
        "json/json",
        "json6.json - {\"trailingComma\":\"es5\"} format 1",
        "JSON6 syntax not valid JSONC",
    ),
    (
        "json/json",
        "positive-number.json - {\"trailingComma\":\"all\"} format 1",
        "positive number prefix (+) not valid JSONC",
    ),
    (
        "json/json",
        "positive-number.json - {\"trailingComma\":\"es5\"} format 1",
        "positive number prefix (+) not valid JSONC",
    ),
    (
        "json/json",
        "propertyKey.json - {\"trailingComma\":\"all\"} format 1",
        "unquoted property keys not valid JSONC",
    ),
    (
        "json/json",
        "propertyKey.json - {\"trailingComma\":\"es5\"} format 1",
        "unquoted property keys not valid JSONC",
    ),
    // ── Blank line preservation in arrays ────────────────────────────────
    // The `pass1.json` fixture contains an array with a blank line in the
    // middle. Prettier keeps this as a multi-line array because of the blank
    // line separator. Our formatter collapses it, producing different (but
    // valid) output.
    (
        "json/json",
        "pass1.json - {\"trailingComma\":\"all\"} format 1",
        "blank line preservation in arrays",
    ),
    (
        "json/json",
        "pass1.json - {\"trailingComma\":\"es5\"} format 1",
        "blank line preservation in arrays",
    ),
    // ── Exponent leading zeros ───────────────────────────────────────────
    // Our number parser preserves leading zeros in huge exponents
    // (e.g. `0.4e00669...`) while prettier strips them (`0.4e669...`).
    // This is implementation-defined for numbers outside IEEE 754 range.
    (
        "json/json-test-suite",
        "snippet: i_number_huge_exp.json format 1",
        "leading zeros in huge exponent preserved",
    ),
    (
        "json/json-test-suite",
        "snippet: i_number_huge_exp.json format 2",
        "leading zeros in huge exponent preserved",
    ),
    // ── Lone/invalid surrogate pairs ─────────────────────────────────────
    // Our JSONC parser rejects lone surrogates (U+D800–U+DFFF) as invalid
    // Unicode, while prettier's parser accepts them and passes them through.
    // These are implementation-defined ("i_" prefix) in the JSON test suite
    // — both behaviors are valid.
    (
        "json/json-test-suite",
        "snippet: i_object_key_lone_2nd_surrogate.json format 1",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_object_key_lone_2nd_surrogate.json format 2",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_1st_surrogate_but_2nd_missing.json format 1",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_1st_surrogate_but_2nd_missing.json format 2",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_1st_valid_surrogate_2nd_invalid.json format 1",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_1st_valid_surrogate_2nd_invalid.json format 2",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_incomplete_surrogate_and_escape_valid.json format 1",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_incomplete_surrogate_and_escape_valid.json format 2",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_incomplete_surrogate_pair.json format 1",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_incomplete_surrogate_pair.json format 2",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_incomplete_surrogates_escape_valid.json format 1",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_incomplete_surrogates_escape_valid.json format 2",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_invalid_lonely_surrogate.json format 1",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_invalid_lonely_surrogate.json format 2",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_invalid_surrogate.json format 1",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_invalid_surrogate.json format 2",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_inverted_surrogates_U+1D11E.json format 1",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_inverted_surrogates_U+1D11E.json format 2",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_lone_second_surrogate.json format 1",
        "lone surrogate rejected by parser",
    ),
    (
        "json/json-test-suite",
        "snippet: i_string_lone_second_surrogate.json format 2",
        "lone surrogate rejected by parser",
    ),
    // ── UTF-8 BOM handling ───────────────────────────────────────────────
    // Our JSONC parser does not strip the UTF-8 byte order mark (U+FEFF)
    // before parsing, causing a parse error on BOM-prefixed input.
    (
        "json/json-test-suite",
        "snippet: i_structure_UTF-8_BOM_empty_object.json format 1",
        "UTF-8 BOM not handled",
    ),
    (
        "json/json-test-suite",
        "snippet: i_structure_UTF-8_BOM_empty_object.json format 2",
        "UTF-8 BOM not handled",
    ),
    // ── Trailing decimal zero stripped ────────────────────────────────────
    // Our number formatter normalizes `1.0` to `1` and `1.0e28` to `1e28`,
    // stripping the trailing `.0` when it has no fractional significance.
    // Prettier preserves the original notation.
    (
        "json/json-test-suite",
        "snippet: number_1.0.json format 1",
        "trailing .0 stripped from numbers",
    ),
    (
        "json/json-test-suite",
        "snippet: number_1.0.json format 2",
        "trailing .0 stripped from numbers",
    ),
    (
        "json/json-test-suite",
        "snippet: y_object_extreme_numbers.json format 1",
        "trailing .0 stripped from numbers",
    ),
    (
        "json/json-test-suite",
        "snippet: y_object_extreme_numbers.json format 2",
        "trailing .0 stripped from numbers",
    ),
    // ── Invalid Unicode codepoints ───────────────────────────────────────
    // These fixtures contain escaped codepoints (e.g. \uFDD0) that are valid
    // JSON but are Unicode noncharacters. Our parser rejects them while
    // prettier passes them through.
    (
        "json/json-test-suite",
        "snippet: string_1_escaped_invalid_codepoint.json format 1",
        "invalid Unicode codepoint rejected",
    ),
    (
        "json/json-test-suite",
        "snippet: string_1_escaped_invalid_codepoint.json format 2",
        "invalid Unicode codepoint rejected",
    ),
    (
        "json/json-test-suite",
        "snippet: string_2_escaped_invalid_codepoints.json format 1",
        "invalid Unicode codepoint rejected",
    ),
    (
        "json/json-test-suite",
        "snippet: string_2_escaped_invalid_codepoints.json format 2",
        "invalid Unicode codepoint rejected",
    ),
    (
        "json/json-test-suite",
        "snippet: string_3_escaped_invalid_codepoints.json format 1",
        "invalid Unicode codepoint rejected",
    ),
    (
        "json/json-test-suite",
        "snippet: string_3_escaped_invalid_codepoints.json format 2",
        "invalid Unicode codepoint rejected",
    ),
    // ── Single-entry object collapsing ───────────────────────────────────
    // Our formatter collapses single-entry objects to one line
    // (`{ "a": "b" }`) while prettier keeps them expanded when the input
    // has newlines between the braces.
    (
        "json/json-test-suite",
        "snippet: y_object_with_newlines.json format 1",
        "single-entry object collapsing",
    ),
    (
        "json/json-test-suite",
        "snippet: y_object_with_newlines.json format 2",
        "single-entry object collapsing",
    ),
    // ── Surrogate pair decoding ──────────────────────────────────────────
    // These fixtures contain valid surrogate pairs (e.g. \uD834\uDD1E for
    // U+1D11E) that prettier preserves as escape sequences. Our parser
    // decodes them into the actual Unicode character, which is valid but
    // produces different output.
    (
        "json/json-test-suite",
        "snippet: y_string_accepted_surrogate_pair.json format 1",
        "surrogate pair decoded to literal character",
    ),
    (
        "json/json-test-suite",
        "snippet: y_string_accepted_surrogate_pair.json format 2",
        "surrogate pair decoded to literal character",
    ),
    (
        "json/json-test-suite",
        "snippet: y_string_accepted_surrogate_pairs.json format 1",
        "surrogate pair decoded to literal character",
    ),
    (
        "json/json-test-suite",
        "snippet: y_string_accepted_surrogate_pairs.json format 2",
        "surrogate pair decoded to literal character",
    ),
    (
        "json/json-test-suite",
        "snippet: y_string_last_surrogates_1_and_2.json format 1",
        "surrogate pair decoded to literal character",
    ),
    (
        "json/json-test-suite",
        "snippet: y_string_last_surrogates_1_and_2.json format 2",
        "surrogate pair decoded to literal character",
    ),
    (
        "json/json-test-suite",
        "snippet: y_string_surrogates_U+1D11E_MUSICAL_SYMBOL_G_CLEF.json format 1",
        "surrogate pair decoded to literal character",
    ),
    (
        "json/json-test-suite",
        "snippet: y_string_surrogates_U+1D11E_MUSICAL_SYMBOL_G_CLEF.json format 2",
        "surrogate pair decoded to literal character",
    ),
    // ── Non-BMP character encoding ───────────────────────────────────────
    // These fixtures contain non-BMP Unicode characters encoded as surrogate
    // pairs in JSON. Our parser decodes them to literal UTF-8 characters
    // while prettier preserves the \uXXXX escape sequences.
    (
        "json/json-test-suite",
        "snippet: y_string_unicode_U+1FFFE_nonchar.json format 1",
        "non-BMP character decoded to literal UTF-8",
    ),
    (
        "json/json-test-suite",
        "snippet: y_string_unicode_U+1FFFE_nonchar.json format 2",
        "non-BMP character decoded to literal UTF-8",
    ),
    (
        "json/json-test-suite",
        "snippet: y_string_unicode_U+10FFFE_nonchar.json format 1",
        "non-BMP character decoded to literal UTF-8",
    ),
    (
        "json/json-test-suite",
        "snippet: y_string_unicode_U+10FFFE_nonchar.json format 2",
        "non-BMP character decoded to literal UTF-8",
    ),
];

fn run_fixture_dir(dir: &str) {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let snap_path = fixtures_dir.join(dir).join("format.test.js.snap");

    let content = std::fs::read_to_string(&snap_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", snap_path.display()));

    let cases = snapshot_parser::parse_snapshot(&content);
    assert!(!cases.is_empty(), "No test cases found in {dir}");

    let known: HashSet<&str> = KNOWN_FAILURES
        .iter()
        .filter(|(d, _, _)| *d == dir)
        .map(|(_, name, _)| *name)
        .collect();

    let mut counts = (0usize, 0usize, 0usize, 0usize); // passed, failed, skipped, expected
    let mut unexpected: Vec<(String, String, String)> = Vec::new();
    let mut stale: Vec<String> = Vec::new();

    for case in &cases {
        let format = match case.parser.as_str() {
            "json" => JsonFormat::Json,
            "jsonc" => JsonFormat::Jsonc,
            // json5 entries are tested in prettier-json5 crate
            _ => {
                counts.2 += 1;
                continue;
            }
        };

        let is_known = known.contains(case.name.as_str());
        let result = format_str(&case.input, format, &case.options);
        let matches = result.as_ref().is_ok_and(|a| *a == case.expected);

        if matches {
            counts.0 += 1;
            if is_known {
                stale.push(case.name.clone());
            }
        } else if is_known {
            counts.3 += 1;
        } else {
            counts.1 += 1;
            if unexpected.len() < 20 {
                let actual = result.unwrap_or_else(|e| format!("ERROR: {e}"));
                unexpected.push((case.name.clone(), case.expected.clone(), actual));
            }
        }
    }

    let (passed, failed, skipped, expected_failures) = counts;
    let total = passed + failed + expected_failures;
    eprintln!(
        "{dir}: {passed}/{total} passed ({expected_failures} known, {failed} unexpected, {skipped} skipped)"
    );
    print_failures(dir, &unexpected, &stale);

    assert!(
        unexpected.is_empty(),
        "{dir}: {failed} unexpected failure(s) — add to KNOWN_FAILURES or fix the formatter"
    );
    assert!(
        stale.is_empty(),
        "{dir}: {} stale exclusion(s) — remove from KNOWN_FAILURES",
        stale.len()
    );
}

fn print_failures(dir: &str, unexpected: &[(String, String, String)], stale: &[String]) {
    if !unexpected.is_empty() {
        eprintln!("\n  Unexpected failures in {dir}:");
        for (name, expected, actual) in unexpected {
            eprintln!("\n  --- {name} ---");
            if actual.starts_with("ERROR:") {
                eprintln!("  {actual}");
            } else {
                for diff in diff_lines(expected, actual) {
                    eprintln!("  {diff}");
                }
            }
        }
        eprintln!();
    }
    if !stale.is_empty() {
        eprintln!("\n  Stale exclusions in {dir} (tests now pass, remove from KNOWN_FAILURES):");
        for name in stale {
            eprintln!("  - {name}");
        }
        eprintln!();
    }
}

/// Simple line-by-line diff for readable failure output.
fn diff_lines(expected: &str, actual: &str) -> Vec<String> {
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    let mut output = Vec::new();
    let max = expected_lines.len().max(actual_lines.len());

    for i in 0..max {
        let exp = expected_lines.get(i).copied();
        let act = actual_lines.get(i).copied();
        match (exp, act) {
            (Some(e), Some(a)) if e == a => {
                output.push(format!(" {e}"));
            }
            (Some(e), Some(a)) => {
                output.push(format!("-{e}"));
                output.push(format!("+{a}"));
            }
            (Some(e), None) => {
                output.push(format!("-{e}"));
            }
            (None, Some(a)) => {
                output.push(format!("+{a}"));
            }
            (None, None) => {}
        }
    }

    if output.len() > 30 {
        output.truncate(30);
        output.push("  ... (diff truncated)".to_string());
    }

    output
}

// JSON fixtures
#[test]
fn json_json() {
    run_fixture_dir("json/json");
}
#[test]
fn json_json5_trailing_commas() {
    run_fixture_dir("json/json5-trailing-commas");
}
#[test]
fn json_jsonc_quote_props() {
    run_fixture_dir("json/jsonc/quote-props");
}
#[test]
fn json_jsonc_single_quote() {
    run_fixture_dir("json/jsonc/single-quote");
}
#[test]
fn json_jsonc_trailing_comma() {
    run_fixture_dir("json/jsonc/trailing-comma");
}
#[test]
fn json_jsonc_empty() {
    run_fixture_dir("json/jsonc/empty");
}
#[test]
fn json_with_comment() {
    run_fixture_dir("json/with-comment");
}
#[test]
fn json_test_suite() {
    run_fixture_dir("json/json-test-suite");
}
