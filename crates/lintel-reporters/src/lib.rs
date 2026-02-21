pub mod reporter;
pub mod reporters;

use core::time::Duration;
use std::time::Instant;

use anyhow::Result;
use bpaf::{Bpaf, Parser};

use lintel_check::retriever::CacheStatus;
use lintel_check::validate::{self, CheckedFile};
use lintel_check::validation_cache::ValidationCacheStatus;

fn schema_cache_ttl() -> impl bpaf::Parser<Option<Duration>> {
    bpaf::long("schema-cache-ttl")
        .help("Schema cache TTL (e.g. \"12h\", \"30m\", \"1d\"); default 12h")
        .argument::<String>("DURATION")
        .parse(|s: String| {
            humantime::parse_duration(&s).map_err(|e| format!("invalid duration '{s}': {e}"))
        })
        .optional()
}

pub use reporter::Reporter;
pub use reporters::github::GithubReporter;
pub use reporters::pretty::PrettyReporter;
pub use reporters::text::TextReporter;

// -----------------------------------------------------------------------
// ReporterKind — CLI-parseable enum
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReporterKind {
    Pretty,
    Text,
    Github,
}

impl core::str::FromStr for ReporterKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pretty" => Ok(Self::Pretty),
            "text" => Ok(Self::Text),
            "github" => Ok(Self::Github),
            _ => Err(format!(
                "unknown reporter '{s}', expected: pretty, text, github"
            )),
        }
    }
}

impl core::fmt::Display for ReporterKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Pretty => write!(f, "pretty"),
            Self::Text => write!(f, "text"),
            Self::Github => write!(f, "github"),
        }
    }
}

// -----------------------------------------------------------------------
// ValidateArgs — shared CLI struct
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Bpaf)]
#[allow(clippy::struct_excessive_bools)]
pub struct ValidateArgs {
    #[bpaf(long("exclude"), argument("PATTERN"))]
    pub exclude: Vec<String>,

    #[bpaf(long("cache-dir"), argument("DIR"))]
    pub cache_dir: Option<String>,

    /// Bypass schema cache reads (still writes fetched schemas to cache)
    #[bpaf(long("force-schema-fetch"), switch)]
    pub force_schema_fetch: bool,

    /// Bypass validation cache reads (still writes results to cache)
    #[bpaf(long("force-validation"), switch)]
    pub force_validation: bool,

    /// Bypass all cache reads (combines --force-schema-fetch and --force-validation)
    #[bpaf(long("force"), switch)]
    pub force: bool,

    #[bpaf(long("no-catalog"), switch)]
    pub no_catalog: bool,

    #[bpaf(external(schema_cache_ttl))]
    pub schema_cache_ttl: Option<Duration>,

    #[bpaf(positional("PATH"))]
    pub globs: Vec<String>,
}

impl From<&ValidateArgs> for validate::ValidateArgs {
    fn from(args: &ValidateArgs) -> Self {
        // When a single directory is passed as an arg, use it as the config
        // search directory so that `lintel.toml` inside that directory is found.
        let config_dir = args
            .globs
            .iter()
            .find(|g| std::path::Path::new(g).is_dir())
            .map(std::path::PathBuf::from);

        validate::ValidateArgs {
            globs: args.globs.clone(),
            exclude: args.exclude.clone(),
            cache_dir: args.cache_dir.clone(),
            force_schema_fetch: args.force_schema_fetch || args.force,
            force_validation: args.force_validation || args.force,
            no_catalog: args.no_catalog,
            config_dir,
            schema_cache_ttl: Some(
                args.schema_cache_ttl
                    .unwrap_or(lintel_check::retriever::DEFAULT_SCHEMA_CACHE_TTL),
            ),
        }
    }
}

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

/// Format a verbose line for a checked file, including cache status tags.
pub fn format_checked_verbose(file: &CheckedFile) -> String {
    let schema_tag = match file.cache_status {
        Some(CacheStatus::Hit) => " [cached]",
        Some(CacheStatus::Miss | CacheStatus::Disabled) => " [fetched]",
        None => "",
    };
    let validation_tag = match file.validation_cache_status {
        Some(ValidationCacheStatus::Hit) => " [validated:cached]",
        Some(ValidationCacheStatus::Miss) => " [validated]",
        None => "",
    };
    format!(
        "  {} ({}){schema_tag}{validation_tag}",
        file.path, file.schema
    )
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
            let cli_excludes = core::mem::take(&mut args.exclude);
            args.exclude = cfg.exclude;
            args.exclude.extend(cli_excludes);
        }
        Err(e) => {
            eprintln!("warning: failed to load lintel.toml: {e}");
        }
    }
}

/// Create a reporter from the kind and verbose flag.
pub fn make_reporter(kind: ReporterKind, verbose: bool) -> Box<dyn Reporter> {
    match kind {
        ReporterKind::Pretty => Box::new(PrettyReporter { verbose }),
        ReporterKind::Text => Box::new(TextReporter { verbose }),
        ReporterKind::Github => Box::new(GithubReporter { verbose }),
    }
}

// -----------------------------------------------------------------------
// Run function — shared between check/ci commands
// -----------------------------------------------------------------------

/// Run validation and report results via the given reporter.
///
/// Returns `true` if there were validation errors, `false` if clean.
///
/// # Errors
///
/// Returns an error if file collection or schema validation encounters an I/O error.
pub async fn run(args: &mut ValidateArgs, reporter: &mut dyn Reporter) -> Result<bool> {
    merge_config(args);

    let lib_args = validate::ValidateArgs::from(&*args);
    let start = Instant::now();
    let result = validate::run_with(&lib_args, None, |file| {
        reporter.on_file_checked(file);
    })
    .await?;
    let had_errors = result.has_errors();
    let elapsed = start.elapsed();

    reporter.report(result, elapsed);

    Ok(had_errors)
}
