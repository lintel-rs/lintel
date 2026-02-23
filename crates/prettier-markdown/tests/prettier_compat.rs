use prettier_markdown::format_markdown;
use prettier_test_harness::{FixtureConfig, run_fixture_dir};
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

fn run(dir: &str) {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let config = FixtureConfig {
        fixtures_dir: &fixtures,
        snap_subpath: "__snapshots__/format.test.js.snap",
        known_failures: KNOWN_FAILURES,
    };
    run_fixture_dir(&config, dir, |case| {
        if case.parser != "markdown" {
            return None;
        }
        Some(format_markdown(&case.input, &case.options).map_err(|e| e.to_string()))
    });
}

// Markdown fixtures
#[test]
fn markdown_heading() {
    run("markdown/heading");
}

#[test]
fn markdown_heading_setext() {
    run("markdown/heading/setext");
}

#[test]
fn markdown_paragraph() {
    run("markdown/paragraph");
}

#[test]
fn markdown_code() {
    run("markdown/code");
}

#[test]
fn markdown_code_angular() {
    run("markdown/code/angular");
}

#[test]
fn markdown_fenced_code_block() {
    run("markdown/fenced-code-block");
}

#[test]
fn markdown_list() {
    run("markdown/list");
}

#[test]
fn markdown_list_task_list() {
    run("markdown/list/task-list");
}

#[test]
fn markdown_blockquote() {
    run("markdown/blockquote");
}

#[test]
fn markdown_emphasis() {
    run("markdown/emphasis");
}

#[test]
fn markdown_strong() {
    run("markdown/strong");
}

#[test]
fn markdown_link() {
    run("markdown/link");
}

#[test]
fn markdown_image() {
    run("markdown/image");
}

#[test]
fn markdown_yaml() {
    run("markdown/yaml");
}

#[test]
fn markdown_front_matter() {
    run("markdown/front-matter");
}

#[test]
fn markdown_thematic_break() {
    run("markdown/thematicBreak");
}

#[test]
fn markdown_break() {
    run("markdown/break");
}

#[test]
fn markdown_break_list_item() {
    run("markdown/break/list-item");
}

#[test]
fn markdown_table() {
    run("markdown/table");
}

#[test]
fn markdown_table_empty() {
    run("markdown/table/empty-table");
}

#[test]
fn markdown_ignore() {
    run("markdown/ignore");
}
