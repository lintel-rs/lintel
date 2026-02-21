use std::collections::HashSet;
use std::path::Path;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use lintel_schema_cache::{HttpClient, SchemaCache};
use tracing::{debug, warn};

/// Maximum schema file size we'll download (10 MiB). Schemas larger than this
/// are skipped and the catalog retains the original upstream URL.
const MAX_SCHEMA_SIZE: u64 = 10 * 1024 * 1024;

/// An item to download: a URL and the destination path.
pub struct DownloadItem {
    pub url: String,
    pub dest: std::path::PathBuf,
}

/// Download a single schema via the cache, validate it is parseable JSON, and
/// write to disk. Returns the JSON text on success (needed for `$ref` scanning).
///
/// Schemas whose serialized output exceeds [`MAX_SCHEMA_SIZE`] are skipped so
/// that very large files don't bloat the output.
pub async fn download_one<C: HttpClient>(
    cache: &SchemaCache<C>,
    url: &str,
    path: &Path,
) -> Result<String> {
    debug!(url = %url, "fetching schema");
    let (value, _status) = cache.fetch(url).await.map_err(|e| anyhow::anyhow!("{e}"))?;

    let text = serde_json::to_string_pretty(&value)?;

    if text.len() as u64 > MAX_SCHEMA_SIZE {
        anyhow::bail!(
            "schema too large ({} MiB, limit {} MiB)",
            text.len() / (1024 * 1024),
            MAX_SCHEMA_SIZE / (1024 * 1024),
        );
    }

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(path, &text).await?;
    Ok(text)
}

/// Download a batch of items concurrently. Returns the set of URLs that were
/// successfully downloaded. Failed downloads are logged as warnings and skipped.
pub async fn download_batch<C: HttpClient>(
    cache: &SchemaCache<C>,
    items: &[DownloadItem],
    concurrency: usize,
) -> Result<HashSet<String>> {
    let total = items.len();
    let downloaded: HashSet<String> = stream::iter(items.iter().enumerate())
        .map(|(i, item)| {
            let cache = cache.clone();
            let url = item.url.clone();
            let dest = item.dest.clone();
            async move {
                match download_one(&cache, &url, &dest).await {
                    Ok(_text) => {
                        debug!(url = %url, progress = format!("{}/{total}", i + 1), "downloaded");
                        Some(url)
                    }
                    Err(e) => {
                        warn!(url = %url, error = %e, "failed to download schema, skipping");
                        None
                    }
                }
            }
        })
        .buffer_unordered(concurrency)
        .filter_map(|r| async { r })
        .collect()
        .await;

    Ok(downloaded)
}
