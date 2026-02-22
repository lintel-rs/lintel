mod snapshot_parser;

use prettier_json5::format_json5;
use std::path::Path;

fn run_fixture_dir(dir: &str) {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let snap_path = fixtures_dir.join(dir).join("format.test.js.snap");

    let content = std::fs::read_to_string(&snap_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", snap_path.display()));

    let cases = snapshot_parser::parse_snapshot(&content);
    assert!(!cases.is_empty(), "No test cases found in {dir}");

    let strict = std::env::var("PRETTIER_STRICT").is_ok();
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut failures: Vec<(String, String, String)> = Vec::new();
    let mut all_failure_names: Vec<String> = Vec::new();

    for case in &cases {
        // Only handle json5 parser entries
        if case.parser != "json5" {
            skipped += 1;
            continue;
        }

        match format_json5(&case.input, &case.options) {
            Ok(actual) => {
                if actual == case.expected {
                    passed += 1;
                } else {
                    failed += 1;
                    all_failure_names.push(case.name.clone());
                    if failures.len() < 50 {
                        failures.push((case.name.clone(), case.expected.clone(), actual));
                    }
                }
            }
            Err(e) => {
                failed += 1;
                all_failure_names.push(case.name.clone());
                if failures.len() < 20 {
                    failures.push((
                        case.name.clone(),
                        case.expected.clone(),
                        format!("ERROR: {e}"),
                    ));
                }
            }
        }
    }

    let total = passed + failed;
    eprintln!("{dir}: {passed}/{total} passed ({failed} failed, {skipped} skipped)");

    if !failures.is_empty() {
        let max_diffs = 50;
        eprintln!("\n  First {max_diffs} failure(s) in {dir}:");
        for (name, expected, actual) in failures.iter().take(max_diffs) {
            eprintln!("\n  --- {name} ---");
            if actual.starts_with("ERROR:") {
                eprintln!("  {actual}");
            } else {
                for diff in diff_lines(expected, actual) {
                    eprintln!("  {diff}");
                }
            }
        }
        eprintln!("\n  All {dir} failure names ({}):", all_failure_names.len());
        for name in &all_failure_names {
            eprintln!("  - {name}");
        }
        eprintln!();
    }

    assert!(
        !(strict && failed > 0),
        "{dir}: {failed}/{total} tests failed (PRETTIER_STRICT=1)"
    );
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
