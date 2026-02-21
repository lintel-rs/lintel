use core::fmt::Write;
use std::path::Path;

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
#[error("README.md already exists")]
#[diagnostic(
    code(furnish::readme_exists),
    severity(Warning),
    help(
        "cargo furnish update --force --description \"...\" {crate_name}\n\nExisting README contents:\n\n{existing_contents}"
    )
)]
pub struct ReadmeExistsError {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("this file already exists")]
    pub span: SourceSpan,
    pub crate_name: String,
    pub existing_contents: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("README.md is missing")]
#[diagnostic(
    code(furnish::missing_readme),
    help("cargo furnish update --description \"...\" {crate_name}")
)]
pub struct ReadmeMissingError {
    pub crate_name: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("README.md is missing the crates.io badge")]
#[diagnostic(
    code(furnish::missing_crates_badge),
    severity(Warning),
    help("cargo furnish update --force {crate_name}")
)]
pub struct MissingCratesBadge {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("expected [![Crates.io](...)](...) badge")]
    pub span: SourceSpan,
    pub crate_name: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("README.md is missing the docs.rs badge")]
#[diagnostic(
    code(furnish::missing_docs_badge),
    severity(Warning),
    help("cargo furnish update --force {crate_name}")
)]
pub struct MissingDocsBadge {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("expected [![docs.rs](...)](...) badge")]
    pub span: SourceSpan,
    pub crate_name: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("README.md is missing the license badge")]
#[diagnostic(
    code(furnish::missing_license_badge),
    severity(Warning),
    help("cargo furnish update --force {crate_name}")
)]
pub struct MissingLicenseBadge {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("expected [![License](...)](...) badge")]
    pub span: SourceSpan,
    pub crate_name: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("README.md is missing ## License section")]
#[diagnostic(
    code(furnish::missing_license_section),
    severity(Warning),
    help("cargo furnish update --force {crate_name}")
)]
pub struct MissingLicenseSection {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("expected ## License section")]
    pub span: SourceSpan,
    pub crate_name: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("README.md is missing the GitHub CI badge")]
#[diagnostic(
    code(furnish::missing_ci_badge),
    severity(Warning),
    help("cargo furnish update --force {crate_name}")
)]
pub struct MissingCiBadge {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("expected [![CI](...)](...) badge")]
    pub span: SourceSpan,
    pub crate_name: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("README.md is the default template with no custom content")]
#[diagnostic(
    code(furnish::default_readme),
    severity(Warning),
    help(
        "cargo furnish update --readme \"...\" {crate_name}\n\n\
         A good README should include:\n  \
         - A short description of what the crate does\n  \
         - A quick-start example showing basic usage\n  \
         - Links to relevant documentation or related crates\n\n\
         The --readme flag accepts markdown that is inserted between\n\
         the description and the License section."
    )
)]
pub struct DefaultReadme {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("this README has no content beyond the auto-generated template")]
    pub span: SourceSpan,
    pub crate_name: String,
}

/// Check if README exists and is well-formed. Returns diagnostics.
pub fn check_readme(
    crate_dir: &Path,
    crate_name: &str,
    description: Option<&str>,
) -> Vec<Box<dyn Diagnostic + Send + Sync>> {
    let readme_path = crate_dir.join("README.md");
    let mut diagnostics: Vec<Box<dyn Diagnostic + Send + Sync>> = Vec::new();

    if !readme_path.exists() {
        diagnostics.push(Box::new(ReadmeMissingError {
            crate_name: crate_name.to_string(),
        }));
        return diagnostics;
    }

    let Ok(content) = std::fs::read_to_string(&readme_path) else {
        return diagnostics;
    };

    let file_name = readme_path.display().to_string();
    let src = || NamedSource::new(file_name.clone(), content.clone());

    // Point diagnostics at the first line of the file
    let first_line_len = content.lines().next().map_or(1, |l| l.len().max(1));

    if !content.contains("img.shields.io/crates/v/") {
        diagnostics.push(Box::new(MissingCratesBadge {
            src: src(),
            span: (0, first_line_len).into(),
            crate_name: crate_name.to_string(),
        }));
    }

    if !content.contains("docs.rs/") {
        diagnostics.push(Box::new(MissingDocsBadge {
            src: src(),
            span: (0, first_line_len).into(),
            crate_name: crate_name.to_string(),
        }));
    }

    if !content.contains("actions/workflows/") {
        diagnostics.push(Box::new(MissingCiBadge {
            src: src(),
            span: (0, first_line_len).into(),
            crate_name: crate_name.to_string(),
        }));
    }

    if !content.contains("img.shields.io/crates/l/") {
        diagnostics.push(Box::new(MissingLicenseBadge {
            src: src(),
            span: (0, first_line_len).into(),
            crate_name: crate_name.to_string(),
        }));
    }

    if !content.contains("## License") {
        // Point at the last line of the file
        let last_line_span = last_line_span(&content);
        diagnostics.push(Box::new(MissingLicenseSection {
            src: src(),
            span: last_line_span,
            crate_name: crate_name.to_string(),
        }));
    }

    // Check if the README is just the default template with no custom body.
    // Extract the region between the last badge-ref line and "## License",
    // strip the one-line description if present — anything left is custom content.
    if is_default_readme(&content, description) {
        diagnostics.push(Box::new(DefaultReadme {
            src: src(),
            span: (0, first_line_len).into(),
            crate_name: crate_name.to_string(),
        }));
    }

    diagnostics
}

/// Check whether the README body region contains only the description (or nothing).
///
/// The "body region" is everything between the last badge line
/// (a line starting with `[![`) and the `## License` heading. If stripping
/// the description from that region leaves only whitespace, the README is default.
fn is_default_readme(content: &str, description: Option<&str>) -> bool {
    let lines: Vec<&str> = content.lines().collect();

    // Last inline badge line (starts with `[![`)
    let last_badge_idx = lines.iter().rposition(|l| l.starts_with("[!["));
    let license_idx = lines.iter().position(|l| l.starts_with("## License"));

    let (Some(badge_end), Some(lic_start)) = (last_badge_idx, license_idx) else {
        return false;
    };

    if badge_end >= lic_start {
        return false;
    }

    // Collect non-empty lines in the body region
    let body: String = lines[badge_end + 1..lic_start]
        .iter()
        .copied()
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if body.is_empty() {
        return true;
    }

    // If the only content is the description line, it's still default
    if let Some(desc) = description {
        body.trim() == desc.trim()
    } else {
        false
    }
}

/// Get the span of the last non-empty line in the content.
fn last_line_span(content: &str) -> SourceSpan {
    let mut last_offset = 0;
    let mut last_len = 1;
    let mut offset = 0;
    for line in content.lines() {
        if !line.is_empty() {
            last_offset = offset;
            last_len = line.len();
        }
        offset += line.len() + 1; // +1 for newline
    }
    (last_offset, last_len).into()
}

/// Generate README content from the template.
pub fn generate_readme(
    crate_name: &str,
    description: Option<&str>,
    body: Option<&str>,
    repository: &str,
    license_text: &str,
) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "# {crate_name}");
    out.push('\n');
    let _ = writeln!(
        out,
        "[![Crates.io](https://img.shields.io/crates/v/{crate_name}.svg)](https://crates.io/crates/{crate_name})"
    );
    let _ = writeln!(
        out,
        "[![docs.rs](https://docs.rs/{crate_name}/badge.svg)](https://docs.rs/{crate_name})"
    );
    let _ = writeln!(
        out,
        "[![CI]({repository}/actions/workflows/ci.yml/badge.svg)]({repository}/actions/workflows/ci.yml)"
    );
    let _ = writeln!(
        out,
        "[![License](https://img.shields.io/crates/l/{crate_name}.svg)]({repository}/blob/master/LICENSE)"
    );

    if let Some(desc) = description {
        out.push('\n');
        out.push_str(desc);
        out.push('\n');
    }

    if let Some(body_text) = body {
        out.push('\n');
        out.push_str(body_text);
        out.push('\n');
    }

    let _ = write!(out, "\n## License\n\n{license_text}\n");

    out
}

/// Write the README, checking for existing file when `--force` is not set.
pub fn fix_readme(
    crate_dir: &Path,
    crate_name: &str,
    description: Option<&str>,
    body: Option<&str>,
    repository: &str,
    license_text: &str,
    force: bool,
) -> miette::Result<()> {
    let readme_path = crate_dir.join("README.md");

    if readme_path.exists() && !force {
        let existing_contents = std::fs::read_to_string(&readme_path)
            .unwrap_or_else(|e| format!("<failed to read: {e}>"));
        let src_len = existing_contents.len();
        return Err(ReadmeExistsError {
            src: NamedSource::new(readme_path.display().to_string(), existing_contents.clone()),
            span: (0, src_len.min(1)).into(),
            crate_name: crate_name.to_string(),
            existing_contents,
        }
        .into());
    }

    let content = generate_readme(crate_name, description, body, repository, license_text);
    std::fs::write(&readme_path, content)
        .map_err(|e| miette::miette!("failed to write {}: {e}", readme_path.display()))?;
    eprintln!("  fixed {}", readme_path.display());
    Ok(())
}
