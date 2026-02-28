use std::path::{Path, PathBuf};

use crate::parsers;

/// Walk `root` respecting `.gitignore`, returning files with known config extensions.
///
/// Applies `excludes` glob patterns to filter results.
///
/// # Errors
///
/// Returns an error if the directory walk encounters an I/O error.
pub fn discover_files(root: &str, excludes: &[String]) -> Result<Vec<PathBuf>, anyhow::Error> {
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false) // don't skip dotfiles (e.g. .eslintrc.json)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    let mut files = Vec::new();
    for entry in walker {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if parsers::detect_format(path).is_none() {
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

fn is_excluded(path: &Path, excludes: &[String]) -> bool {
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

    #[test]
    fn discovers_known_extensions() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(tmp.path().join("a.json"), "{}")?;
        fs::write(tmp.path().join("b.yaml"), "key: val")?;
        fs::write(tmp.path().join("c.yml"), "key: val")?;
        fs::write(tmp.path().join("d.json5"), "{}")?;
        fs::write(tmp.path().join("e.jsonc"), "{}")?;
        fs::write(tmp.path().join("f.txt"), "nope")?;
        fs::write(tmp.path().join("g.nix"), "{ }")?;
        fs::write(tmp.path().join("h.jsonl"), "{\"a\":1}\n")?;

        let root = tmp.path().to_str().expect("temp dir should be valid UTF-8");
        let files = discover_files(root, &[])?;
        assert_eq!(files.len(), 6);
        assert!(files.iter().all(|f| parsers::detect_format(f).is_some()));
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
        let files = discover_files(root, &["**/vendor/**".to_string()])?;
        assert_eq!(files.len(), 1);
        Ok(())
    }

    #[test]
    fn discovers_dotfiles() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        fs::write(tmp.path().join(".eslintrc.json"), "{}")?;

        let root = tmp.path().to_str().expect("temp dir should be valid UTF-8");
        let files = discover_files(root, &[])?;
        assert_eq!(files.len(), 1);
        Ok(())
    }
}
