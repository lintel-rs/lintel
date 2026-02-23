use core::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashSet;
use std::path::Path;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use lintel_schema_cache::{CacheStatus, SchemaCache};
use tracing::{info, warn};

/// Maximum schema file size we'll download (10 MiB). Schemas larger than this
/// are skipped and the catalog retains the original upstream URL.
const MAX_SCHEMA_SIZE: u64 = 10 * 1024 * 1024;

/// An item to download: a URL and the destination path.
pub struct DownloadItem {
    pub url: String,
    pub dest: std::path::PathBuf,
}

/// Download a single schema via the cache, validate it is parseable JSON, and
/// write to disk. Returns the JSON text and cache status on success.
///
/// Schemas whose serialized output exceeds [`MAX_SCHEMA_SIZE`] are skipped so
/// that very large files don't bloat the output.
pub async fn download_one(
    cache: &SchemaCache,
    url: &str,
    path: &Path,
) -> Result<(String, CacheStatus)> {
    let (value, status) = cache.fetch(url).await.map_err(|e| anyhow::anyhow!("{e}"))?;

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
    Ok((text, status))
}

/// Download a batch of items concurrently. Returns the set of URLs that were
/// successfully downloaded. Failed downloads are logged as warnings and skipped.
pub async fn download_batch(
    cache: &SchemaCache,
    items: &[DownloadItem],
    concurrency: usize,
) -> Result<HashSet<String>> {
    let total = items.len();
    let completed = AtomicUsize::new(0);
    let downloaded: HashSet<String> = stream::iter(items.iter())
        .map(|item| {
            let cache = cache.clone();
            let url = item.url.clone();
            let dest = item.dest.clone();
            let completed = &completed;
            async move {
                match download_one(&cache, &url, &dest).await {
                    Ok((_text, status)) => {
                        let n = completed.fetch_add(1, Ordering::Relaxed) + 1;
                        info!(
                            url = %url,
                            status = %status,
                            progress = format!("{n}/{total}"),
                            "downloaded"
                        );
                        Some(url)
                    }
                    Err(e) => {
                        completed.fetch_add(1, Ordering::Relaxed);
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
