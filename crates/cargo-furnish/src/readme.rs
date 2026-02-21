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
    #[label("expected [![Crates.io][crates-badge]][crates-url] badge")]
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
    #[label("expected [![docs.rs][docs-badge]][docs-url] badge")]
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
    #[label("expected [![License][license-badge]][license-url] badge")]
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
    #[label("expected [![CI][ci-badge]][ci-url] badge")]
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
    repository: &str,
    license_text: &str,
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

    if !content.contains("[crates-badge]") {
        diagnostics.push(Box::new(MissingCratesBadge {
            src: src(),
            span: (0, first_line_len).into(),
            crate_name: crate_name.to_string(),
        }));
    }

    if !content.contains("[docs-badge]") {
        diagnostics.push(Box::new(MissingDocsBadge {
            src: src(),
            span: (0, first_line_len).into(),
            crate_name: crate_name.to_string(),
        }));
    }

    if !content.contains("[ci-badge]") {
        diagnostics.push(Box::new(MissingCiBadge {
            src: src(),
            span: (0, first_line_len).into(),
            crate_name: crate_name.to_string(),
        }));
    }

    if !content.contains("[license-badge]") {
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

    // Check if the README is just the default template with no custom body
    let default = generate_readme(crate_name, description, None, repository, license_text);
    if content == default {
        diagnostics.push(Box::new(DefaultReadme {
            src: src(),
            span: (0, first_line_len).into(),
            crate_name: crate_name.to_string(),
        }));
    }

    diagnostics
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
    out.push_str("[![Crates.io][crates-badge]][crates-url]\n");
    out.push_str("[![docs.rs][docs-badge]][docs-url]\n");
    out.push_str("[![CI][ci-badge]][ci-url]\n");
    out.push_str("[![License][license-badge]][license-url]\n");
    out.push('\n');
    let _ = writeln!(
        out,
        "[crates-badge]: https://img.shields.io/crates/v/{crate_name}.svg"
    );
    let _ = writeln!(out, "[crates-url]: https://crates.io/crates/{crate_name}");
    let _ = writeln!(out, "[docs-badge]: https://docs.rs/{crate_name}/badge.svg");
    let _ = writeln!(out, "[docs-url]: https://docs.rs/{crate_name}");
    let _ = writeln!(
        out,
        "[ci-badge]: {repository}/actions/workflows/ci.yml/badge.svg"
    );
    let _ = writeln!(out, "[ci-url]: {repository}/actions/workflows/ci.yml");
    let _ = writeln!(
        out,
        "[license-badge]: https://img.shields.io/crates/l/{crate_name}.svg"
    );
    let _ = writeln!(out, "[license-url]: {repository}/blob/master/LICENSE");

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
