use std::collections::HashSet;
use std::path::Path;

use futures_util::stream::{self, StreamExt};
use tracing::{debug, warn};

/// Maximum schema file size we'll download (10 MiB). Schemas larger than this
/// are skipped and the catalog retains the original upstream URL.
const MAX_SCHEMA_SIZE: u64 = 10 * 1024 * 1024;

/// Download a set of schemas concurrently and write them to `output_dir`.
///
/// Returns the set of filenames that were successfully written.
/// Failed downloads are logged as warnings and skipped.
pub async fn download_schemas(
    client: &reqwest::Client,
    urls_and_filenames: &[(String, String)],
    output_dir: &Path,
    concurrency: usize,
) -> anyhow::Result<HashSet<String>> {
    let total = urls_and_filenames.len();
    let downloaded: HashSet<String> = stream::iter(urls_and_filenames.iter().enumerate())
        .map(|(i, (url, filename))| {
            let client = client.clone();
            let output_path = output_dir.join(filename);
            let url = url.clone();
            let filename = filename.clone();
            async move {
                match download_one(&client, &url, &output_path).await {
                    Ok(()) => {
                        debug!(filename = %filename, progress = format!("{}/{total}", i + 1), "downloaded");
                        Some(filename)
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

/// Download a single schema, validate it is parseable JSON, and write to disk.
///
/// Schemas whose `Content-Length` exceeds [`MAX_SCHEMA_SIZE`] are skipped so
/// that very large files (e.g. 50+ MiB data-model schemas) don't bloat the
/// mirror repository. The catalog will retain the original upstream URL for
/// any skipped schema.
async fn download_one(client: &reqwest::Client, url: &str, path: &Path) -> anyhow::Result<()> {
    debug!(url = %url, "fetching schema");
    let resp = client.get(url).send().await?.error_for_status()?;

    // Check Content-Length before reading the body.
    if let Some(len) = resp.content_length()
        && len > MAX_SCHEMA_SIZE
    {
        anyhow::bail!(
            "schema too large ({} MiB, limit {} MiB)",
            len / (1024 * 1024),
            MAX_SCHEMA_SIZE / (1024 * 1024),
        );
    }

    let text = resp.text().await?;

    // Guard against servers that omit Content-Length.
    if text.len() as u64 > MAX_SCHEMA_SIZE {
        anyhow::bail!(
            "schema too large ({} MiB, limit {} MiB)",
            text.len() / (1024 * 1024),
            MAX_SCHEMA_SIZE / (1024 * 1024),
        );
    }

    // Validate that the response is valid JSON
    let _: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("response from {url} is not valid JSON: {e}"))?;

    tokio::fs::write(path, &text).await?;
    Ok(())
}
