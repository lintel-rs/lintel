use prettier_yaml::{ProseWrap, YamlFormatOptions};

pub struct TestCase {
    pub name: String,
    pub options: YamlFormatOptions,
    pub input: String,
    pub expected: String,
}

/// Parse a Jest snapshot file into individual test cases.
///
/// Only returns YAML test cases (skips non-yaml parsers).
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

fn parse_entry(name: &str, body: &str) -> Option<TestCase> {
    let lines: Vec<&str> = body.lines().collect();

    let options_idx = lines.iter().position(|l| is_section_marker(l, "options"))?;
    let input_idx = lines.iter().position(|l| is_section_marker(l, "input"))?;
    let output_idx = lines.iter().position(|l| is_section_marker(l, "output"))?;
    let end_idx = lines.iter().rposition(|l| is_end_marker(l))?;

    if end_idx <= output_idx {
        return None;
    }

    let option_lines = &lines[options_idx + 1..input_idx];
    let (parser, options, skip) = parse_options(option_lines);

    // Only handle YAML; skip unsupported parsers/options
    if skip || parser != "yaml" {
        return None;
    }

    let mut input = unescape_template_literal(&lines[input_idx + 1..output_idx].join("\n"));
    let mut expected = unescape_template_literal(&lines[output_idx + 1..end_idx].join("\n"));
    // Ensure trailing newline — prettier always outputs one, and the snapshot
    // format strips the final newline. Don't add to empty content.
    if !input.is_empty() && !input.ends_with('\n') {
        input.push('\n');
    }
    if !expected.is_empty() && !expected.ends_with('\n') {
        expected.push('\n');
    }

    Some(TestCase {
        name: name.to_string(),
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

fn parse_options(lines: &[&str]) -> (String, YamlFormatOptions, bool) {
    let mut options = YamlFormatOptions::default();
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
            "singleQuote" => {
                options.single_quote = value == "true";
            }
            "bracketSpacing" => {
                options.bracket_spacing = value == "true";
            }
            "useTabs" => {
                options.use_tabs = value == "true";
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
