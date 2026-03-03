//! Shared file discovery for Lintel.
//!
//! Provides directory walking that respects `.gitignore`, ignore-pattern glob
//! exclusion (oxlint-style), and caller-provided file filters.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use glob_set::GlobSet;

/// Check if a path is excluded by the ignore set.
///
/// Returns `true` if the path matches any pattern in `ignore_set`.
/// An empty set excludes nothing.
pub fn is_excluded(path: &Path, ignore_set: &GlobSet) -> bool {
    if ignore_set.is_empty() {
        return false;
    }
    let Some(path_str) = path.to_str() else {
        return false;
    };
    let path_str = path_str.strip_prefix("./").unwrap_or(path_str);
    ignore_set.is_match(path_str)
}

/// Walk a directory tree, respecting `.gitignore`, filtering by predicate,
/// applying ignore patterns.
///
/// Hidden files (e.g. `.eslintrc.json`) are included, but the `.git` directory is skipped.
///
/// # Errors
///
/// Returns an error if the directory walk encounters an I/O error.
pub fn discover_files(
    root: &str,
    ignore_set: &GlobSet,
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
        if is_excluded(path, ignore_set) {
            continue;
        }
        files.push(path.to_path_buf());
    }

    files.sort();
    Ok(files)
}

/// Resolve globs/directories into file lists.
///
/// When `globs` is empty, discovers from `"."`. Files matching `ignore_set`
/// are excluded.
///
/// # Errors
///
/// Returns an error if a glob pattern is invalid or a directory cannot be walked.
pub fn collect_files(
    globs: &[String],
    ignore_set: &GlobSet,
    filter: impl Fn(&Path) -> bool,
) -> Result<Vec<PathBuf>> {
    if globs.is_empty() {
        return discover_files(".", ignore_set, filter);
    }

    let mut result = Vec::new();
    for pattern in globs {
        let path = Path::new(pattern);
        if path.is_dir() {
            result.extend(discover_files(pattern, ignore_set, &filter)?);
        } else {
            for entry in
                glob::glob(pattern).with_context(|| format!("invalid glob pattern: {pattern}"))?
            {
                let path = entry?;
                if path.is_file() && filter(&path) && !is_excluded(&path, ignore_set) {
                    result.push(path);
                }
            }
        }
    }
    result.sort();
    result.dedup();
    Ok(result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    fn always_true(_path: &Path) -> bool {
        true
    }

    fn json_only(path: &Path) -> bool {
        path.extension().and_then(|e| e.to_str()) == Some("json")
    }

    fn build_ignore_set(patterns: &[&str]) -> GlobSet {
        let mut builder = glob_set::GlobSetBuilder::new();
        for p in patterns {
            builder.add(glob_set::Glob::new(p).unwrap());
        }
        builder.build().unwrap()
    }

    #[test]
    fn discovers_all_files_with_true_filter() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(tmp.path().join("a.json"), "{}")?;
        fs::write(tmp.path().join("b.yaml"), "key: val")?;
        fs::write(tmp.path().join("c.txt"), "nope")?;

        let root = tmp.path().to_str().expect("temp dir should be valid UTF-8");
        let files = discover_files(root, &GlobSet::default(), always_true)?;
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
        let files = discover_files(root, &GlobSet::default(), json_only)?;
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("a.json"));
        Ok(())
    }

    #[test]
    fn respects_ignore_patterns() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let sub = tmp.path().join("vendor");
        fs::create_dir_all(&sub)?;
        fs::write(tmp.path().join("a.json"), "{}")?;
        fs::write(sub.join("b.json"), "{}")?;

        let root = tmp.path().to_str().expect("temp dir should be valid UTF-8");
        let ignore_set = build_ignore_set(&["**/vendor/**"]);
        let files = discover_files(root, &ignore_set, json_only)?;
        assert_eq!(files.len(), 1);
        Ok(())
    }

    #[test]
    fn discovers_dotfiles() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(tmp.path().join(".eslintrc.json"), "{}")?;

        let root = tmp.path().to_str().expect("temp dir should be valid UTF-8");
        let files = discover_files(root, &GlobSet::default(), json_only)?;
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
        let files = discover_files(root, &GlobSet::default(), json_only)?;
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("real.json"));
        Ok(())
    }

    // --- is_excluded ---

    #[test]
    fn is_excluded_empty_set_excludes_nothing() {
        let path = Path::new("anything.json");
        assert!(!is_excluded(path, &GlobSet::default()));
    }

    #[test]
    fn is_excluded_matching_pattern() {
        let set = build_ignore_set(&["vendor/**"]);
        assert!(is_excluded(Path::new("vendor/file.json"), &set));
    }

    #[test]
    fn is_excluded_no_match() {
        let set = build_ignore_set(&["vendor/**"]);
        assert!(!is_excluded(Path::new("src/main.json"), &set));
    }

    #[test]
    fn is_excluded_strips_dot_slash() {
        let set = build_ignore_set(&["vendor/**"]);
        assert!(is_excluded(Path::new("./vendor/file.json"), &set));
    }

    #[test]
    fn is_excluded_multiple_patterns() {
        let set = build_ignore_set(&["vendor/**", "testdata/**"]);
        assert!(is_excluded(Path::new("vendor/file.json"), &set));
        assert!(is_excluded(Path::new("testdata/file.json"), &set));
        assert!(!is_excluded(Path::new("src/main.json"), &set));
    }
}
