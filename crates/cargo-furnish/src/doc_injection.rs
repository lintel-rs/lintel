use std::path::Path;

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

const DOC_LINE: &str = "#![doc = include_str!(\"../README.md\")]";

#[derive(Debug, Error, Diagnostic)]
#[error("file has doc comments that would be replaced")]
#[diagnostic(
    code(furnish::doc_comment_exists),
    severity(Warning),
    help("cargo furnish update --force {crate_name}")
)]
pub struct DocExistsError {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("existing doc comment")]
    pub span: SourceSpan,
    pub crate_name: String,
}

#[derive(Debug, Error, Diagnostic)]
#[error("missing #![doc = include_str!(\"../README.md\")] as first line")]
#[diagnostic(
    code(furnish::missing_doc_include),
    help("autofixable with cargo furnish check --fix")
)]
pub struct DocMissingError {
    #[source_code]
    pub src: NamedSource<String>,
    #[label("should start with #![doc = include_str!(\"../README.md\")]")]
    pub span: SourceSpan,
}

/// Check if the crate's source file has the doc include. Returns diagnostics.
pub fn check_doc_include(
    crate_dir: &Path,
    crate_name: &str,
) -> Vec<Box<dyn Diagnostic + Send + Sync>> {
    let lib_rs = crate_dir.join("src/lib.rs");
    let main_rs = crate_dir.join("src/main.rs");

    let target = if lib_rs.exists() {
        lib_rs
    } else if main_rs.exists() {
        main_rs
    } else {
        return Vec::new();
    };

    let Ok(content) = std::fs::read_to_string(&target) else {
        return Vec::new();
    };

    let file_name = target.display().to_string();

    if content
        .lines()
        .next()
        .is_some_and(|line| line.trim() == DOC_LINE)
    {
        return Vec::new();
    }

    // Check if there are //! doc comments
    let has_doc_comments = content.lines().any(|line| line.starts_with("//!"));

    if has_doc_comments {
        // Find the first //! line for the span
        let mut offset = 0;
        for line in content.lines() {
            if line.starts_with("//!") {
                return vec![Box::new(DocExistsError {
                    src: NamedSource::new(file_name, content.clone()),
                    span: (offset, line.len()).into(),
                    crate_name: crate_name.to_string(),
                })];
            }
            offset += line.len() + 1;
        }
    }

    // No doc include and no doc comments — just missing
    let span_len = content.lines().next().map_or(0, str::len).max(1);
    vec![Box::new(DocMissingError {
        src: NamedSource::new(file_name, content),
        span: (0, span_len).into(),
    })]
}

/// Fix: ensure the target source file has the doc include as its first line.
pub fn fix_doc_include(crate_dir: &Path, crate_name: &str, force: bool) -> miette::Result<()> {
    let lib_rs = crate_dir.join("src/lib.rs");
    let main_rs = crate_dir.join("src/main.rs");

    let target = if lib_rs.exists() {
        lib_rs
    } else if main_rs.exists() {
        main_rs
    } else {
        eprintln!("  skipped doc injection (no src/lib.rs or src/main.rs)");
        return Ok(());
    };

    let content = std::fs::read_to_string(&target)
        .map_err(|e| miette::miette!("failed to read {}: {e}", target.display()))?;

    // Already has the doc include — idempotent skip
    if content
        .lines()
        .next()
        .is_some_and(|line| line.trim() == DOC_LINE)
    {
        return Ok(());
    }

    let has_doc_comments = content.lines().any(|line| line.starts_with("//!"));

    if has_doc_comments && !force {
        let mut offset = 0;
        for line in content.lines() {
            if line.starts_with("//!") {
                return Err(DocExistsError {
                    src: NamedSource::new(target.display().to_string(), content.clone()),
                    span: (offset, line.len()).into(),
                    crate_name: crate_name.to_string(),
                }
                .into());
            }
            offset += line.len() + 1;
        }
    }

    // Strip leading //! lines, prepend the doc include
    let mut new_lines: Vec<&str> = Vec::new();
    let mut past_doc_comments = false;
    for line in content.lines() {
        if !past_doc_comments && line.starts_with("//!") {
            continue;
        }
        past_doc_comments = true;
        new_lines.push(line);
    }

    while new_lines.first().is_some_and(|l| l.is_empty()) {
        new_lines.remove(0);
    }

    let mut result = String::from(DOC_LINE);
    result.push('\n');
    if !new_lines.is_empty() {
        result.push('\n');
        for line in &new_lines {
            result.push_str(line);
            result.push('\n');
        }
    }

    std::fs::write(&target, result)
        .map_err(|e| miette::miette!("failed to write {}: {e}", target.display()))?;
    eprintln!("  fixed {}", target.display());
    Ok(())
}
