pub mod check;
pub mod ci;
pub mod completions;
pub mod convert;
pub mod init;

use lintel_check::retriever::CacheStatus;
use lintel_check::validate::CheckedFile;

use crate::ValidateArgs;

/// Format a verbose line for a checked file, including cache status tag.
pub fn format_checked_verbose(file: &CheckedFile) -> String {
    match file.cache_status {
        Some(CacheStatus::Hit) => format!("  {} ({}) [cached]", file.path, file.schema),
        Some(CacheStatus::Miss | CacheStatus::Disabled) => {
            format!("  {} ({}) [fetched]", file.path, file.schema)
        }
        None => format!("  {} ({})", file.path, file.schema),
    }
}

/// Load `lintel.toml` and merge its excludes into the args.
///
/// Config excludes are prepended so they have the same priority as CLI excludes.
/// When a directory arg is passed (e.g. `lintel check some/dir`), we search
/// for `lintel.toml` starting from that directory rather than cwd.
pub fn merge_config(args: &mut ValidateArgs) {
    let search_dir = args
        .globs
        .iter()
        .find(|g| std::path::Path::new(g).is_dir())
        .map(std::path::PathBuf::from);

    let cfg_result = match &search_dir {
        Some(dir) => lintel_check::config::find_and_load(dir).map(Option::unwrap_or_default),
        None => lintel_check::config::load(),
    };

    match cfg_result {
        Ok(cfg) => {
            // Config excludes first, then CLI excludes.
            let cli_excludes = std::mem::take(&mut args.exclude);
            args.exclude = cfg.exclude;
            args.exclude.extend(cli_excludes);
        }
        Err(e) => {
            eprintln!("warning: failed to load lintel.toml: {e}");
        }
    }
}
