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
    // ── Blank line preservation in arrays ────────────────────────────────
    // The `pass1.json` fixture contains an array with a blank line in the
    // middle. Prettier keeps this as a multi-line array because of the blank
    // line separator. Biome collapses it, producing different (but valid) output.
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
    // ── Single-quoted strings ───────────────────────────────────────────
    // Biome's JSON parser rejects single-quoted strings as invalid JSON/JSONC.
    // Our old custom parser accepted and re-quoted them.
    (
        "json/json",
        "array.json - {\"trailingComma\":\"all\"} format 1",
        "biome: no single-quote string support",
    ),
    (
        "json/json",
        "array.json - {\"trailingComma\":\"es5\"} format 1",
        "biome: no single-quote string support",
    ),
    (
        "json/json",
        "single-quote.json - {\"trailingComma\":\"all\"} format 1",
        "biome: no single-quote string support",
    ),
    (
        "json/json",
        "single-quote.json - {\"trailingComma\":\"es5\"} format 1",
        "biome: no single-quote string support",
    ),
    (
        "json/jsonc/single-quote",
        "test.jsonc - {\"singleQuote\":false} format 1",
        "biome: no single-quote string support",
    ),
    (
        "json/jsonc/single-quote",
        "test.jsonc - {\"singleQuote\":true} format 1",
        "biome: no single-quote string support",
    ),
    // ── Unquoted property keys ──────────────────────────────────────────
    // Biome's JSON parser requires double-quoted property keys; it does not
    // support the unquoted keys used in the quote-props test fixtures.
    (
        "json/jsonc/quote-props",
        "test.jsonc - {\"quoteProps\":\"as-needed\"} format 1",
        "biome: no unquoted property key support",
    ),
    (
        "json/jsonc/quote-props",
        "test.jsonc - {\"quoteProps\":\"consistent\"} format 1",
        "biome: no unquoted property key support",
    ),
    (
        "json/jsonc/quote-props",
        "test.jsonc - {\"quoteProps\":\"preserve\"} format 1",
        "biome: no unquoted property key support",
    ),
    // ── UTF-8 BOM handling ───────────────────────────────────────────────
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
    // ── Escaped solidus ─────────────────────────────────────────────────
    // Biome strips the escaped solidus (\/) to plain (/), while prettier
    // preserves it. Both are valid JSON representations.
    (
        "json/json-test-suite",
        "snippet: y_string_allowed_escapes.json format 1",
        "biome: does not preserve \\/ escape",
    ),
    (
        "json/json-test-suite",
        "snippet: y_string_allowed_escapes.json format 2",
        "biome: does not preserve \\/ escape",
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
