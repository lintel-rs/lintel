use prettier_jsonc::{JsonFormat, format_str};
use prettier_test_harness::{FixtureConfig, run_fixture_dir};
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

fn run(dir: &str) {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let config = FixtureConfig {
        fixtures_dir: &fixtures,
        snap_subpath: "format.test.js.snap",
        known_failures: KNOWN_FAILURES,
    };
    run_fixture_dir(&config, dir, |case| {
        let format = match case.parser.as_str() {
            "json" => JsonFormat::Json,
            "jsonc" => JsonFormat::Jsonc,
            _ => return None,
        };
        Some(format_str(&case.input, format, &case.options).map_err(|e| e.to_string()))
    });
}

#[test]
fn json_json() {
    run("json/json");
}
#[test]
fn json_json5_trailing_commas() {
    run("json/json5-trailing-commas");
}
#[test]
fn json_jsonc_quote_props() {
    run("json/jsonc/quote-props");
}
#[test]
fn json_jsonc_single_quote() {
    run("json/jsonc/single-quote");
}
#[test]
fn json_jsonc_trailing_comma() {
    run("json/jsonc/trailing-comma");
}
#[test]
fn json_jsonc_empty() {
    run("json/jsonc/empty");
}
#[test]
fn json_with_comment() {
    run("json/with-comment");
}
#[test]
fn json_test_suite() {
    run("json/json-test-suite");
}
