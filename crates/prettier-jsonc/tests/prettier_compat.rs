mod snapshot_parser;

use prettier_jsonc::{JsonFormat, format_str};
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
        let format = match case.parser.as_str() {
            "json" => JsonFormat::Json,
            "json5" => JsonFormat::Json5,
            "jsonc" => JsonFormat::Jsonc,
            _ => {
                skipped += 1;
                continue;
            }
        };

        match format_str(&case.input, format, &case.options) {
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
                // Show a unified-style diff
                for diff in diff_lines(expected, actual) {
                    eprintln!("  {diff}");
                }
            }
        }
        // Always print all failure names for analysis
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

    // Limit output to avoid overwhelming logs
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
fn debug_pass1_format1() {
    use prettier_jsonc::JsonFormat;
    let fixtures_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let snap_path = fixtures_dir.join("json/json/format.test.js.snap");
    let content = std::fs::read_to_string(&snap_path).expect("read snapshot");
    let cases = snapshot_parser::parse_snapshot(&content);
    for case in &cases {
        let is_pass1 = case.name.starts_with("pass1.json") && case.name.contains("all");
        if !is_pass1 {
            continue;
        }
        let format = match case.parser.as_str() {
            "json" => JsonFormat::Json,
            "json5" => JsonFormat::Json5,
            "jsonc" => JsonFormat::Jsonc,
            _ => continue,
        };
        let result =
            prettier_jsonc::format_str(&case.input, format, &case.options).expect("format pass1");
        if result != case.expected {
            let exp_chars: Vec<char> = case.expected.chars().collect();
            let act_chars: Vec<char> = result.chars().collect();
            for (i, (e, a)) in exp_chars.iter().zip(act_chars.iter()).enumerate() {
                if e != a {
                    let ctx_s = i.saturating_sub(40);
                    let ctx_e = (i + 120).min(exp_chars.len()).min(act_chars.len());
                    eprintln!("[{}] First diff at char {i}:", case.name);
                    eprintln!(
                        "  Expected: {:?}",
                        &case.expected[ctx_s..ctx_e.min(case.expected.len())]
                    );
                    eprintln!("  Actual:   {:?}", &result[ctx_s..ctx_e.min(result.len())]);
                    break;
                }
            }
            if exp_chars.len() != act_chars.len() {
                eprintln!(
                    "Length diff: expected {} chars, got {} chars",
                    exp_chars.len(),
                    act_chars.len()
                );
            }
        }
    }
}
