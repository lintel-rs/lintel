use prettier_json5::format_json5;
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
    // middle (`" s p a c e d ": [1, 2, 3,\n\n4, 5, 6, 7]`). Prettier keeps
    // this as a multi-line array because of the blank line separator. Our
    // formatter collapses it, producing different (but valid) output.
    (
        "json/json",
        "pass1.json - {\"trailingComma\":\"all\"} format 2",
        "blank line preservation in arrays",
    ),
    (
        "json/json",
        "pass1.json - {\"trailingComma\":\"es5\"} format 2",
        "blank line preservation in arrays",
    ),
    // ── Exponent leading zeros ───────────────────────────────────────────
    // Our number parser preserves leading zeros in huge exponents
    // (e.g. `0.4e00669...`) while prettier strips them (`0.4e669...`).
    // This is an implementation-defined behavior for numbers outside the
    // normal IEEE 754 range.
    (
        "json/json-test-suite",
        "snippet: i_number_huge_exp.json format 3",
        "leading zeros in huge exponent preserved",
    ),
    // ── UTF-8 BOM handling ───────────────────────────────────────────────
    // Our JSON5 parser does not strip the UTF-8 byte order mark (U+FEFF)
    // before parsing, causing a parse error on BOM-prefixed input.
    (
        "json/json-test-suite",
        "snippet: i_structure_UTF-8_BOM_empty_object.json format 3",
        "UTF-8 BOM not handled",
    ),
    // ── Unicode NFC/NFD key normalization ────────────────────────────────
    // These fixtures have object keys using both NFC and NFD forms of "é".
    // After NFC normalization the keys become identical, and our formatter
    // quotes the duplicate differently than prettier does.
    (
        "json/json-test-suite",
        "snippet: object_key_nfc_nfd.json format 3",
        "NFC/NFD key quoting differs",
    ),
    (
        "json/json-test-suite",
        "snippet: object_key_nfd_nfc.json format 3",
        "NFC/NFD key quoting differs",
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
        if case.parser != "json5" {
            return None;
        }
        Some(format_json5(&case.input, &case.options).map_err(|e| e.to_string()))
    });
}

#[test]
fn json5_from_json() {
    run("json/json");
}

#[test]
fn json5_trailing_commas() {
    run("json/json5-trailing-commas");
}

#[test]
fn json_test_suite() {
    run("json/json-test-suite");
}
