use std::path::PathBuf;

use anyhow::Result;
use bpaf::Bpaf;

/// Arguments for the `generate` subcommand.
#[derive(Debug, Clone, Bpaf)]
pub struct GenerateArgs {
    /// Path to lintel-catalog.toml config file
    #[bpaf(
        long("config"),
        argument("PATH"),
        fallback(PathBuf::from("lintel-catalog.toml"))
    )]
    pub config: PathBuf,

    /// Build only a specific target (default: all targets)
    #[bpaf(long("target"), argument("NAME"))]
    pub target: Option<String>,

    /// Maximum concurrent downloads
    #[bpaf(long("concurrency"), argument("N"), fallback(20))]
    pub concurrency: usize,

    /// Skip reading from cache (still writes fetched schemas to cache)
    #[bpaf(long("no-cache"), switch)]
    pub no_cache: bool,
}

impl GenerateArgs {
    pub async fn run(self) -> Result<()> {
        crate::generate::run(
            &self.config,
            self.target.as_deref(),
            self.concurrency,
            self.no_cache,
        )
        .await
    }
}
