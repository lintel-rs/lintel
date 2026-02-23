mod snapshot_parser;

use prettier_markdown::format_markdown;
use std::path::Path;

fn run_fixture_dir(dir: &str) {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let snap_path = fixtures_dir
        .join(dir)
        .join("__snapshots__/format.test.js.snap");

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
        if case.parser.as_str() != "markdown" {
            skipped += 1;
            continue;
        }

        match format_markdown(&case.input, &case.options) {
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

// Markdown fixtures
#[test]
fn markdown_heading() {
    run_fixture_dir("markdown/heading");
}

#[test]
fn markdown_heading_setext() {
    run_fixture_dir("markdown/heading/setext");
}

#[test]
fn markdown_paragraph() {
    run_fixture_dir("markdown/paragraph");
}

#[test]
fn markdown_code() {
    run_fixture_dir("markdown/code");
}

#[test]
fn markdown_code_angular() {
    run_fixture_dir("markdown/code/angular");
}

#[test]
fn markdown_fenced_code_block() {
    run_fixture_dir("markdown/fenced-code-block");
}

#[test]
fn markdown_list() {
    run_fixture_dir("markdown/list");
}

#[test]
fn markdown_list_task_list() {
    run_fixture_dir("markdown/list/task-list");
}

#[test]
fn markdown_blockquote() {
    run_fixture_dir("markdown/blockquote");
}

#[test]
fn markdown_emphasis() {
    run_fixture_dir("markdown/emphasis");
}

#[test]
fn markdown_strong() {
    run_fixture_dir("markdown/strong");
}

#[test]
fn markdown_link() {
    run_fixture_dir("markdown/link");
}

#[test]
fn markdown_image() {
    run_fixture_dir("markdown/image");
}

#[test]
fn markdown_yaml() {
    run_fixture_dir("markdown/yaml");
}

#[test]
fn markdown_front_matter() {
    run_fixture_dir("markdown/front-matter");
}

#[test]
fn markdown_thematic_break() {
    run_fixture_dir("markdown/thematicBreak");
}

#[test]
fn markdown_break() {
    run_fixture_dir("markdown/break");
}

#[test]
fn markdown_break_list_item() {
    run_fixture_dir("markdown/break/list-item");
}

#[test]
fn markdown_table() {
    run_fixture_dir("markdown/table");
}

#[test]
fn markdown_table_empty() {
    run_fixture_dir("markdown/table/empty-table");
}
