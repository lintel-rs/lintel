use std::path::{Path, PathBuf};

use crate::parsers;

/// Walk `root` respecting `.gitignore`, returning files with known config extensions.
///
/// Applies `excludes` glob patterns to filter results.
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
    fn discovers_known_extensions() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.json"), "{}").unwrap();
        fs::write(tmp.path().join("b.yaml"), "key: val").unwrap();
        fs::write(tmp.path().join("c.yml"), "key: val").unwrap();
        fs::write(tmp.path().join("d.json5"), "{}").unwrap();
        fs::write(tmp.path().join("e.jsonc"), "{}").unwrap();
        fs::write(tmp.path().join("f.txt"), "nope").unwrap();
        fs::write(tmp.path().join("g.nix"), "{ }").unwrap();

        let files = discover_files(tmp.path().to_str().unwrap(), &[]).unwrap();
        assert_eq!(files.len(), 5);
        assert!(files.iter().all(|f| parsers::detect_format(f).is_some()));
    }

    #[test]
    fn respects_exclude_patterns() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("vendor");
        fs::create_dir_all(&sub).unwrap();
        fs::write(tmp.path().join("a.json"), "{}").unwrap();
        fs::write(sub.join("b.json"), "{}").unwrap();

        let files =
            discover_files(tmp.path().to_str().unwrap(), &["**/vendor/**".to_string()]).unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn discovers_dotfiles() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(".eslintrc.json"), "{}").unwrap();

        let files = discover_files(tmp.path().to_str().unwrap(), &[]).unwrap();
        assert_eq!(files.len(), 1);
    }
}
