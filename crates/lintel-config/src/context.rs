use std::path::{Path, PathBuf};

use glob_set::GlobSet;

use crate::Config;

/// Pre-loaded configuration context, built once per CLI invocation.
///
/// Encapsulates the merged `lintel.toml` config, the directory it was found in
/// (for resolving `//`-prefixed schema paths), and the compiled ignore set
/// for file exclusion.
pub struct ConfigContext {
    /// The merged configuration from `lintel.toml` (or default if none found).
    pub config: Config,
    /// Directory containing the `lintel.toml` (for resolving relative schema paths).
    pub config_dir: PathBuf,
    /// Path to the `lintel.toml` file itself, if one was found.
    pub config_path: Option<PathBuf>,
    /// Raw ignore patterns (config + CLI) for serialization/display.
    pub ignore_patterns: Vec<String>,
    /// Compiled glob set for efficient file exclusion.
    pub ignore_set: GlobSet,
}

impl ConfigContext {
    /// Load `lintel.toml` and merge CLI excludes.
    ///
    /// Determines the search directory from `globs` (first directory arg),
    /// loads/merges config, and combines config ignore-patterns with CLI excludes.
    pub fn load(globs: &[String], cli_excludes: &[String]) -> Self {
        let search_dir = globs
            .iter()
            .find(|g| Path::new(g).is_dir())
            .map(PathBuf::from);

        Self::load_from_dir(search_dir.as_deref(), cli_excludes)
    }

    /// Load `lintel.toml` from an explicit search directory.
    ///
    /// If `search_dir` is `None`, searches from the current working directory.
    pub fn load_from_dir(search_dir: Option<&Path>, cli_excludes: &[String]) -> Self {
        let start_dir = match search_dir {
            Some(d) => d.to_path_buf(),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };

        let config_path = crate::find_config_path(&start_dir);

        let config = match crate::find_and_load(&start_dir) {
            Ok(Some(cfg)) => cfg,
            Ok(None) => Config::default(),
            Err(e) => {
                eprintln!("warning: failed to load lintel.toml: {e}");
                Config::default()
            }
        };

        let config_dir = config_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf)
            .unwrap_or(start_dir);

        // Collect ignore patterns from config + CLI --exclude.
        let mut ignore_patterns = config
            .files
            .as_ref()
            .map(|f| f.ignore_patterns.clone())
            .unwrap_or_default();
        ignore_patterns.extend(cli_excludes.iter().cloned());

        let ignore_set = build_ignore_set(&ignore_patterns);

        ConfigContext {
            config,
            config_dir,
            config_path,
            ignore_patterns,
            ignore_set,
        }
    }
}

/// Build a `GlobSet` from a list of pattern strings.
///
/// Invalid patterns are silently skipped.
fn build_ignore_set(patterns: &[String]) -> GlobSet {
    let mut builder = glob_set::GlobSetBuilder::new();
    for pat in patterns {
        if let Ok(g) = glob_set::Glob::new(pat) {
            builder.add(g);
        }
    }
    builder.build().unwrap_or_default()
}
