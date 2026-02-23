mod snapshot_parser;

use prettier_json5::format_json5;
use std::collections::HashSet;
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
        // Only handle json5 parser entries
        if case.parser != "json5" {
            counts.2 += 1;
            continue;
        }

        let is_known = known.contains(case.name.as_str());
        let result = format_json5(&case.input, &case.options);
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

// JSON5 entries from json/json snapshot
#[test]
fn json5_from_json() {
    run_fixture_dir("json/json");
}

// JSON5 trailing commas
#[test]
fn json5_trailing_commas() {
    run_fixture_dir("json/json5-trailing-commas");
}

// JSON test suite (RFC 8259 compliance)
#[test]
fn json_test_suite() {
    run_fixture_dir("json/json-test-suite");
}
