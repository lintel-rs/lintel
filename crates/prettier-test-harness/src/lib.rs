use core::fmt::Display;
use prettier_config::{PrettierConfig, ProseWrap, QuoteProps, TrailingComma};
use std::collections::HashSet;
use std::path::Path;

/// A single test case extracted from a Jest snapshot file.
pub struct TestCase {
    pub name: String,
    pub parser: String,
    pub options: PrettierConfig,
    pub input: String,
    pub expected: String,
}

/// Parse a Jest snapshot file into individual test cases.
///
/// Returns all test cases with their parser name. Skips entries with the
/// `objectWrap` option (not yet supported). Callers should filter by parser
/// name as needed.
pub fn parse_snapshot(content: &str) -> Vec<TestCase> {
    let mut cases = Vec::new();
    let mut pos = 0;

    while let Some(start_offset) = content[pos..].find("exports[`") {
        let name_start = pos + start_offset + "exports[`".len();

        let Some(name_end_offset) = content[name_start..].find("`] = `\n") else {
            break;
        };
        let name = &content[name_start..name_start + name_end_offset];

        let body_start = name_start + name_end_offset + "`] = `\n".len();

        let Some(body_end_offset) = content[body_start..].find("\n`;") else {
            break;
        };
        let body_end = body_start + body_end_offset;

        let body = &content[body_start..body_end];

        if let Some(case) = parse_entry(name, body) {
            cases.push(case);
        }

        pos = body_end + "\n`;".len();
    }

    cases
}

/// Configuration for running a fixture directory.
pub struct FixtureConfig<'a> {
    /// Root path to the test fixtures (typically
    /// `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")`).
    pub fixtures_dir: &'a Path,
    /// Relative path from `dir` to the snapshot file
    /// (e.g. `"format.test.js.snap"` or `"__snapshots__/format.test.js.snap"`).
    pub snap_subpath: &'a str,
    /// Slice of `(dir, test_name, reason)` tuples for expected failures.
    /// Only entries matching the current `dir` are used.
    pub known_failures: &'a [(&'a str, &'a str, &'a str)],
}

/// Run a fixture directory against a formatter, tracking known failures.
///
/// The `format_fn` closure receives a `&TestCase` and returns:
/// - `None` to skip the test (e.g. wrong parser)
/// - `Some(Ok(output))` to compare against expected output
/// - `Some(Err(e))` for a formatter error (counts as failure)
///
/// # Panics
///
/// Panics if there are unexpected failures (not in `known_failures`) or
/// stale exclusions (in `known_failures` but now passing).
pub fn run_fixture_dir<F, E>(config: &FixtureConfig<'_>, dir: &str, format_fn: F)
where
    F: Fn(&TestCase) -> Option<Result<String, E>>,
    E: Display,
{
    let snap_path = config.fixtures_dir.join(dir).join(config.snap_subpath);

    let content = std::fs::read_to_string(&snap_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", snap_path.display()));

    let cases = parse_snapshot(&content);
    assert!(!cases.is_empty(), "No test cases found in {dir}");

    let known: HashSet<&str> = config
        .known_failures
        .iter()
        .filter(|(d, _, _)| *d == dir)
        .map(|(_, name, _)| *name)
        .collect();

    let mut counts = Counts::default();
    let mut unexpected: Vec<(String, String, String)> = Vec::new();
    let mut stale: Vec<String> = Vec::new();

    for case in &cases {
        let Some(result) = format_fn(case) else {
            counts.skipped += 1;
            continue;
        };

        let is_known = known.contains(case.name.as_str());
        let matches = result.as_ref().is_ok_and(|a| *a == case.expected);

        if matches {
            counts.passed += 1;
            if is_known {
                stale.push(case.name.clone());
            }
        } else if is_known {
            counts.expected += 1;
        } else {
            counts.failed += 1;
            if unexpected.len() < 20 {
                let actual = result
                    .map_err(|e| format!("ERROR: {e}"))
                    .unwrap_or_else(|e| e);
                unexpected.push((case.name.clone(), case.expected.clone(), actual));
            }
        }
    }

    let total = counts.passed + counts.failed + counts.expected;
    eprintln!(
        "{dir}: {}/{total} passed ({} known, {} unexpected, {} skipped)",
        counts.passed, counts.expected, counts.failed, counts.skipped
    );
    print_failures(dir, &unexpected, &stale);

    assert!(
        unexpected.is_empty(),
        "{dir}: {} unexpected failure(s) — add to KNOWN_FAILURES or fix the formatter",
        counts.failed
    );
    assert!(
        stale.is_empty(),
        "{dir}: {} stale exclusion(s) — remove from KNOWN_FAILURES",
        stale.len()
    );
}

#[derive(Default)]
struct Counts {
    passed: usize,
    failed: usize,
    skipped: usize,
    expected: usize,
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

fn parse_entry(name: &str, body: &str) -> Option<TestCase> {
    let lines: Vec<&str> = body.lines().collect();

    let options_idx = lines.iter().position(|l| is_section_marker(l, "options"))?;
    let input_idx = lines.iter().position(|l| is_section_marker(l, "input"))?;
    let output_idx = lines.iter().position(|l| is_section_marker(l, "output"))?;
    // Use rposition to find the last all-= marker (the closing one after output)
    let end_idx = lines.iter().rposition(|l| is_end_marker(l))?;

    if end_idx <= output_idx {
        return None;
    }

    let option_lines = &lines[options_idx + 1..input_idx];
    let (parser, options, skip) = parse_options(option_lines);

    if skip {
        return None;
    }

    let mut input = unescape_template_literal(&lines[input_idx + 1..output_idx].join("\n"));
    let mut expected = unescape_template_literal(&lines[output_idx + 1..end_idx].join("\n"));
    // Ensure trailing newline — formatters always output one, and the snapshot
    // format strips the final newline. Don't add to empty content.
    if !input.is_empty() && !input.ends_with('\n') {
        input.push('\n');
    }
    if !expected.is_empty() && !expected.ends_with('\n') {
        expected.push('\n');
    }

    Some(TestCase {
        name: name.to_string(),
        parser,
        options,
        input,
        expected,
    })
}

fn is_section_marker(line: &str, keyword: &str) -> bool {
    line.contains(keyword) && line.chars().filter(|&c| c == '=').count() > 20
}

fn is_end_marker(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() > 20 && trimmed.chars().all(|c| c == '=')
}

fn parse_options(lines: &[&str]) -> (String, PrettierConfig, bool) {
    let mut options = PrettierConfig::default();
    let mut parser = String::new();
    let mut skip = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Remove trailing column indicator " |"
        let trimmed = trimmed.strip_suffix(" |").unwrap_or(trimmed).trim();
        // Remove "(default)" annotation
        let trimmed = if let Some(s) = trimmed.strip_suffix("(default)") {
            s.trim()
        } else {
            trimmed
        };

        let Some((key, value)) = trimmed.split_once(": ") else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        match key {
            "parsers" => {
                let inner = value
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .trim_matches('"');
                parser = inner.to_string();
            }
            "tabWidth" => {
                if let Ok(v) = value.parse::<usize>() {
                    options.tab_width = v;
                }
            }
            "printWidth" => {
                if let Ok(v) = value.parse::<usize>() {
                    options.print_width = v;
                }
            }
            "trailingComma" => {
                options.trailing_comma = match value.trim_matches('"') {
                    "es5" => TrailingComma::Es5,
                    "none" => TrailingComma::None,
                    _ => TrailingComma::All,
                };
            }
            "quoteProps" => {
                options.quote_props = match value.trim_matches('"') {
                    "consistent" => QuoteProps::Consistent,
                    "preserve" => QuoteProps::Preserve,
                    _ => QuoteProps::AsNeeded,
                };
            }
            "singleQuote" => {
                options.single_quote = value == "true";
            }
            "useTabs" => {
                options.use_tabs = value == "true";
            }
            "bracketSpacing" => {
                options.bracket_spacing = value == "true";
            }
            "proseWrap" => {
                options.prose_wrap = match value.trim_matches('"') {
                    "always" => ProseWrap::Always,
                    "never" => ProseWrap::Never,
                    _ => ProseWrap::Preserve,
                };
            }
            "objectWrap" => {
                skip = true;
            }
            _ => {}
        }
    }

    (parser, options, skip)
}

/// Unescape Jest snapshot template literal content.
///
/// In JavaScript template literals: `\\` -> `\`, `` \` `` -> `` ` ``, `\$` -> `$`.
fn unescape_template_literal(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\\') | None => result.push('\\'),
                Some('`') => result.push('`'),
                Some('$') => result.push('$'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_entry() {
        let content = r#"// Jest Snapshot v1, https://jestjs.io/docs/snapshot-testing

exports[`test.json format 1`] = `
====================================options=====================================
parsers: ["json"]
                                                      printWidth: 80 (default) |
=====================================input======================================
{"a": 1}

=====================================output=====================================
{ "a": 1 }

================================================================================
`;
"#;
        let cases = parse_snapshot(content);
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].name, "test.json format 1");
        assert_eq!(cases[0].parser, "json");
        assert_eq!(cases[0].input, "{\"a\": 1}\n");
        assert_eq!(cases[0].expected, "{ \"a\": 1 }\n");
    }

    #[test]
    fn test_skip_object_wrap() {
        let content = r#"// Jest Snapshot v1

exports[`test.json - {"objectWrap":"collapse"} format 1`] = `
====================================options=====================================
objectWrap: "collapse"
parsers: ["json"]
                                                      printWidth: 80 (default) |
=====================================input======================================
{}

=====================================output=====================================
{}

================================================================================
`;
"#;
        let cases = parse_snapshot(content);
        assert_eq!(cases.len(), 0);
    }

    #[test]
    fn test_parse_options() {
        let content = r#"// Jest Snapshot v1

exports[`test.jsonc - {"trailingComma":"es5"} format 1`] = `
====================================options=====================================
parsers: ["jsonc"]
trailingComma: "es5"
                                                      printWidth: 80 (default) |
=====================================input======================================
[1]

=====================================output=====================================
[1]

================================================================================
`;
"#;
        let cases = parse_snapshot(content);
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].parser, "jsonc");
        assert_eq!(cases[0].options.trailing_comma, TrailingComma::Es5);
    }

    #[test]
    fn test_unescape_backslash() {
        assert_eq!(unescape_template_literal(r"a\\b"), r"a\b");
        assert_eq!(unescape_template_literal(r"a\`b"), "a`b");
        assert_eq!(unescape_template_literal(r"a\${b}"), "a${b}");
        // Non-special escapes are preserved
        assert_eq!(unescape_template_literal(r"a\nb"), r"a\nb");
    }
}
