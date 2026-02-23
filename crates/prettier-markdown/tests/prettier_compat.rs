mod snapshot_parser;

use prettier_markdown::format_markdown;
use std::collections::HashSet;
use std::path::Path;

/// Known test failures with explanations.
///
/// Each entry is a `(dir, test_name, reason)` tuple. Tests listed here are
/// expected to fail and won't cause CI failures. If a listed test starts
/// passing, the test harness will flag it as a stale exclusion so we can
/// remove it.
const KNOWN_FAILURES: &[(&str, &str, &str)] = &[
    // ── Embedded CSS formatting ──────────────────────────────────────────
    // These tests expect prettier's CSS formatter to reformat CSS code blocks.
    // We don't have a CSS formatter, so the code blocks are passed through as-is.
    (
        "markdown/code",
        "mdn-auth-api.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS/JS formatter",
    ),
    (
        "markdown/code",
        "mdn-background-1.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-background-2.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-background-3.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-background-4.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-background-5.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-background-6.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-background-7.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-background-8.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-background-9.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-filter-1.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-filter-2.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-font-face-1.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-font-face-2.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-grid-auto-columns.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-import.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-mask-image.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-padding-1.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-padding-2.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-transform.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    (
        "markdown/code",
        "mdn-unicode-range.md - {\"proseWrap\":\"always\"} format 1",
        "no CSS formatter",
    ),
    // ── Embedded JS/TS formatting ────────────────────────────────────────
    // These tests expect prettier's JavaScript/TypeScript formatter to reformat
    // JS/TS code blocks. We don't have JS/TS formatters.
    (
        "markdown/code",
        "0-indent-js.md - {\"proseWrap\":\"always\"} format 1",
        "no JS formatter",
    ),
    (
        "markdown/code",
        "format.md - {\"proseWrap\":\"always\"} format 1",
        "no JS formatter",
    ),
    (
        "markdown/code",
        "leading-trailing-newlines.md - {\"proseWrap\":\"always\"} format 1",
        "no JS formatter",
    ),
    (
        "markdown/code",
        "ts-trailing-comma.md - {\"proseWrap\":\"always\"} format 1",
        "no TS formatter",
    ),
    // ── Embedded Angular formatting ──────────────────────────────────────
    // These tests expect prettier's Angular formatter for angular-html and
    // angular-ts code blocks. We don't have an Angular formatter.
    (
        "markdown/code/angular",
        "angular-html.md format 1",
        "no Angular formatter",
    ),
    (
        "markdown/code/angular",
        "angular-ts.md format 1",
        "no Angular formatter",
    ),
    // ── JSON formatting differences ──────────────────────────────────────
    // Our JSON formatter collapses single-entry objects to one line (e.g.,
    // `{ "browser": true }`) while prettier's JSON formatter always expands
    // them. The formatting is valid but differs from prettier's output.
    (
        "markdown/blockquote",
        "code.md - {\"proseWrap\":\"always\"} format 1",
        "JSON object collapsing differs from prettier",
    ),
    (
        "markdown/blockquote",
        "code.md - {\"proseWrap\":\"never\"} format 1",
        "JSON object collapsing differs from prettier",
    ),
    (
        "markdown/blockquote",
        "code.md - {\"proseWrap\":\"preserve\"} format 1",
        "JSON object collapsing differs from prettier",
    ),
    // ── prettier-ignore in blockquotes ───────────────────────────────────
    // These tests exercise `<!-- prettier-ignore -->` inside blockquotes combined
    // with `// prettier-ignore` inside JS code blocks. We support the markdown-level
    // prettier-ignore but the JS code block test expects prettier's JS formatter to
    // honor `// prettier-ignore` and add a semicolon — we don't have a JS formatter.
    (
        "markdown/blockquote",
        "ignore-code.md - {\"proseWrap\":\"always\"} format 1",
        "no JS formatter for // prettier-ignore",
    ),
    (
        "markdown/blockquote",
        "ignore-code.md - {\"proseWrap\":\"never\"} format 1",
        "no JS formatter for // prettier-ignore",
    ),
    (
        "markdown/blockquote",
        "ignore-code.md - {\"proseWrap\":\"preserve\"} format 1",
        "no JS formatter for // prettier-ignore",
    ),
    // ── Blockquote interruption ──────────────────────────────────────────
    // This test covers blockquote interruption of other block-level elements,
    // which requires tracking whether a blockquote lazily continues or starts
    // a new context. Our blockquote handler doesn't distinguish these cases.
    (
        "markdown/blockquote",
        "interrupt-others.md - {\"proseWrap\":\"preserve\"} format 1",
        "blockquote lazy continuation",
    ),
    // ── Setext headings ──────────────────────────────────────────────────
    // These tests involve link reference definitions (`[foo]: url`) which comrak
    // resolves and removes from the AST. We can't reproduce them in output
    // because the definitions aren't preserved as AST nodes.
    (
        "markdown/heading/setext",
        "definition-before.md format 1",
        "link reference definitions not in AST",
    ),
    (
        "markdown/heading/setext",
        "snippet: #1 format 1",
        "link reference definitions not in AST",
    ),
    // ── Link escaping ────────────────────────────────────────────────────
    // These tests cover backslash escaping of special characters inside link
    // URLs and titles. Our link formatter doesn't replicate prettier's exact
    // escaping strategy for characters like `(`, `)`, and `"` in URLs.
    (
        "markdown/link",
        "escape-in-link.md - {\"proseWrap\":\"always\",\"singleQuote\":true} format 1",
        "link escape differences",
    ),
    (
        "markdown/link",
        "escape-in-link.md - {\"proseWrap\":\"always\"} format 1",
        "link escape differences",
    ),
    // ── Image alt text wrapping ──────────────────────────────────────────
    // This test expects image alt text to be wrapped at print_width. Our
    // formatter treats image alt text as atomic (not wrappable).
    (
        "markdown/image",
        "alt.md - {\"proseWrap\":\"always\"} format 1",
        "image alt text wrapping",
    ),
    // ── Hard break wrapping ──────────────────────────────────────────────
    // This test expects hard breaks (trailing `\` or `  `) to interact with
    // prose wrapping in a specific way. Our formatter doesn't re-wrap text
    // around hard breaks.
    (
        "markdown/break",
        "wrap.md - {\"proseWrap\":\"always\"} format 1",
        "hard break + prose wrap interaction",
    ),
];

fn run_fixture_dir(dir: &str) {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let snap_path = fixtures_dir
        .join(dir)
        .join("__snapshots__/format.test.js.snap");

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
        if case.parser.as_str() != "markdown" {
            counts.2 += 1;
            continue;
        }
        let is_known = known.contains(case.name.as_str());
        let result = format_markdown(&case.input, &case.options);
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

#[test]
fn markdown_ignore() {
    run_fixture_dir("markdown/ignore");
}
