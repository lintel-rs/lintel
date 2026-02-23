mod snapshot_parser;

use prettier_rs::{Format, format_str};
use std::collections::HashSet;
use std::path::Path;

/// Known test failures with explanations.
///
/// Each entry is a `(dir, test_name, reason)` tuple. Tests listed here are
/// expected to fail and won't cause CI failures. If a listed test starts
/// passing, the test harness will flag it as a stale exclusion so we can
/// remove it.
///
/// These mirror the known failures in the individual formatter crates
/// (prettier-yaml, prettier-json5, prettier-jsonc) since prettier-rs
/// delegates to them.
const KNOWN_FAILURES: &[(&str, &str, &str)] = &[
    // ── Flow sequence with nested sequences ──────────────────────────────
    // Spec example 7.14 uses a flow sequence with an embedded nested flow
    // sequence (`[nested]`) and a trailing comma. Our flow sequence formatter
    // doesn't handle the nested sequence boundary correctly, duplicating the
    // nested content.
    (
        "yaml/spec",
        "spec-example-7-14-flow-sequence-entries.yml - {\"useTabs\":true} format 1",
        "flow sequence with nested sequence formatting",
    ),
    // ── Explicit key (?) in flow sequences ───────────────────────────────
    // Spec example 7.20 uses the `?` (explicit key) indicator inside a flow
    // sequence (`[? foo bar : baz]`). Our formatter doesn't preserve the
    // explicit key indicator in flow contexts.
    (
        "yaml/spec",
        "spec-example-7-20-single-pair-explicit-entry.yml - {\"useTabs\":true} format 1",
        "explicit key (?) in flow sequence not supported",
    ),
    // ── Explicit key in flow mapping with document markers ───────────────
    // Spec example 9.4 uses explicit keys (`?`) inside a flow mapping combined
    // with document start/end markers (`---`/`...`). Our formatter doesn't
    // handle explicit keys in flow mappings, causing the output structure to
    // diverge from prettier's.
    (
        "yaml/spec",
        "spec-example-9-4-explicit-documents.yml - {\"useTabs\":true} format 1",
        "explicit key (?) in flow mapping not supported",
    ),
    // ── %TAG directive parsing ───────────────────────────────────────────
    // Spec example 9.5 uses the `%TAG` directive. Our YAML parser does not
    // support `%TAG` directives, causing a parse error.
    (
        "yaml/spec",
        "spec-example-9-5-directives-documents.yml - {\"proseWrap\":\"always\"} format 1",
        "%TAG directive not supported",
    ),
    (
        "yaml/spec",
        "spec-example-9-5-directives-documents.yml - {\"useTabs\":true} format 1",
        "%TAG directive not supported",
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
            "json" => Format::Json,
            "json5" => Format::Json5,
            "jsonc" => Format::Jsonc,
            "yaml" => Format::Yaml,
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

// YAML fixtures
#[test]
fn yaml_alias() {
    run_fixture_dir("yaml/alias");
}
#[test]
fn yaml_ansible() {
    run_fixture_dir("yaml/ansible");
}
#[test]
fn yaml_block_folded() {
    run_fixture_dir("yaml/block-folded");
}
#[test]
fn yaml_block_literal() {
    run_fixture_dir("yaml/block-literal");
}
#[test]
fn yaml_comment() {
    run_fixture_dir("yaml/comment");
}
#[test]
fn yaml_directive() {
    run_fixture_dir("yaml/directive");
}
#[test]
fn yaml_document() {
    run_fixture_dir("yaml/document");
}
#[test]
fn yaml_flow_mapping() {
    run_fixture_dir("yaml/flow-mapping");
}
#[test]
fn yaml_flow_sequence() {
    run_fixture_dir("yaml/flow-sequence");
}
#[test]
fn yaml_home_assistant() {
    run_fixture_dir("yaml/home-assistant");
}
#[test]
fn yaml_mapping() {
    run_fixture_dir("yaml/mapping");
}
#[test]
fn yaml_plain() {
    run_fixture_dir("yaml/plain");
}
#[test]
fn yaml_prettier_ignore() {
    run_fixture_dir("yaml/prettier-ignore");
}
#[test]
fn yaml_quote() {
    run_fixture_dir("yaml/quote");
}
#[test]
fn yaml_root() {
    run_fixture_dir("yaml/root");
}
#[test]
fn yaml_sequence() {
    run_fixture_dir("yaml/sequence");
}
#[test]
fn yaml_spec() {
    run_fixture_dir("yaml/spec");
}
