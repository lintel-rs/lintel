use prettier_rs::{Format, format_str};
use prettier_test_harness::{FixtureConfig, run_fixture_dir};
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

fn run(dir: &str) {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let config = FixtureConfig {
        fixtures_dir: &fixtures,
        snap_subpath: "format.test.js.snap",
        known_failures: KNOWN_FAILURES,
    };
    run_fixture_dir(&config, dir, |case| {
        let format = match case.parser.as_str() {
            "json" => Format::Json,
            "json5" => Format::Json5,
            "jsonc" => Format::Jsonc,
            "yaml" => Format::Yaml,
            _ => return None,
        };
        Some(format_str(&case.input, format, &case.options).map_err(|e| e.to_string()))
    });
}

#[test]
fn yaml_alias() {
    run("yaml/alias");
}
#[test]
fn yaml_ansible() {
    run("yaml/ansible");
}
#[test]
fn yaml_block_folded() {
    run("yaml/block-folded");
}
#[test]
fn yaml_block_literal() {
    run("yaml/block-literal");
}
#[test]
fn yaml_comment() {
    run("yaml/comment");
}
#[test]
fn yaml_directive() {
    run("yaml/directive");
}
#[test]
fn yaml_document() {
    run("yaml/document");
}
#[test]
fn yaml_flow_mapping() {
    run("yaml/flow-mapping");
}
#[test]
fn yaml_flow_sequence() {
    run("yaml/flow-sequence");
}
#[test]
fn yaml_home_assistant() {
    run("yaml/home-assistant");
}
#[test]
fn yaml_mapping() {
    run("yaml/mapping");
}
#[test]
fn yaml_plain() {
    run("yaml/plain");
}
#[test]
fn yaml_prettier_ignore() {
    run("yaml/prettier-ignore");
}
#[test]
fn yaml_quote() {
    run("yaml/quote");
}
#[test]
fn yaml_root() {
    run("yaml/root");
}
#[test]
fn yaml_sequence() {
    run("yaml/sequence");
}
#[test]
fn yaml_spec() {
    run("yaml/spec");
}
