//! Shared file discovery for Lintel.
//!
//! Provides directory walking that respects `.gitignore`, exclude glob patterns,
//! and caller-provided file filters.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Walk a directory tree, respecting `.gitignore`, filtering by predicate, applying excludes.
///
/// Hidden files (e.g. `.eslintrc.json`) are included, but the `.git` directory is skipped.
///
/// # Errors
///
/// Returns an error if the directory walk encounters an I/O error.
pub fn discover_files(
    root: &str,
    excludes: &[String],
    filter: impl Fn(&Path) -> bool,
) -> Result<Vec<PathBuf>> {
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false) // don't skip dotfiles (e.g. .eslintrc.json)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build();

    let mut files = Vec::new();
    for entry in walker {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !filter(path) {
            continue;
        }
        if is_excluded(path, excludes) {
            continue;
        }
        files.push(path.to_path_buf());
    }

    files.sort();
    Ok(files)
}

/// Resolve globs/directories into file lists. Empty globs = auto-discover from `"."`.
///
/// # Errors
///
/// Returns an error if a glob pattern is invalid or a directory cannot be walked.
pub fn collect_files(
    globs: &[String],
    excludes: &[String],
    filter: impl Fn(&Path) -> bool,
) -> Result<Vec<PathBuf>> {
    if globs.is_empty() {
        return discover_files(".", excludes, filter);
    }

    let mut result = Vec::new();
    for pattern in globs {
        let path = Path::new(pattern);
        if path.is_dir() {
            result.extend(discover_files(pattern, excludes, &filter)?);
        } else {
            for entry in
                glob::glob(pattern).with_context(|| format!("invalid glob pattern: {pattern}"))?
            {
                let path = entry?;
                if path.is_file() && !is_excluded(&path, excludes) {
                    result.push(path);
                }
            }
        }
    }
    result.sort();
    result.dedup();
    Ok(result)
}

/// Check if a path matches any exclude glob pattern.
pub fn is_excluded(path: &Path, excludes: &[String]) -> bool {
    let path_str = match path.to_str() {
        Some(s) => s.strip_prefix("./").unwrap_or(s),
        None => return false,
    };
    excludes
        .iter()
        .any(|pattern| glob_match::glob_match(pattern, path_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn always_true(_path: &Path) -> bool {
        true
    }

    fn json_only(path: &Path) -> bool {
        path.extension().and_then(|e| e.to_str()) == Some("json")
    }

    #[test]
    fn discovers_all_files_with_true_filter() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(tmp.path().join("a.json"), "{}")?;
        fs::write(tmp.path().join("b.yaml"), "key: val")?;
        fs::write(tmp.path().join("c.txt"), "nope")?;

        let root = tmp.path().to_str().expect("temp dir should be valid UTF-8");
        let files = discover_files(root, &[], always_true)?;
        assert_eq!(files.len(), 3);
        Ok(())
    }

    #[test]
    fn discovers_files_with_extension_filter() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(tmp.path().join("a.json"), "{}")?;
        fs::write(tmp.path().join("b.yaml"), "key: val")?;
        fs::write(tmp.path().join("c.txt"), "nope")?;

        let root = tmp.path().to_str().expect("temp dir should be valid UTF-8");
        let files = discover_files(root, &[], json_only)?;
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("a.json"));
        Ok(())
    }

    #[test]
    fn respects_exclude_patterns() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let sub = tmp.path().join("vendor");
        fs::create_dir_all(&sub)?;
        fs::write(tmp.path().join("a.json"), "{}")?;
        fs::write(sub.join("b.json"), "{}")?;

        let root = tmp.path().to_str().expect("temp dir should be valid UTF-8");
        let files = discover_files(root, &["**/vendor/**".to_string()], json_only)?;
        assert_eq!(files.len(), 1);
        Ok(())
    }

    #[test]
    fn discovers_dotfiles() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(tmp.path().join(".eslintrc.json"), "{}")?;

        let root = tmp.path().to_str().expect("temp dir should be valid UTF-8");
        let files = discover_files(root, &[], json_only)?;
        assert_eq!(files.len(), 1);
        Ok(())
    }

    #[test]
    fn skips_git_directory() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let git_dir = tmp.path().join(".git");
        fs::create_dir_all(&git_dir)?;
        fs::write(git_dir.join("config.json"), "{}")?;
        fs::write(tmp.path().join("real.json"), "{}")?;

        let root = tmp.path().to_str().expect("temp dir should be valid UTF-8");
        let files = discover_files(root, &[], json_only)?;
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("real.json"));
        Ok(())
    }

    #[test]
    fn is_excluded_strips_dot_slash() {
        let path = Path::new("./vendor/file.json");
        assert!(is_excluded(path, &["**/vendor/**".to_string()]));
    }

    #[test]
    fn is_excluded_no_match() {
        let path = Path::new("src/main.json");
        assert!(!is_excluded(path, &["**/vendor/**".to_string()]));
    }
}
