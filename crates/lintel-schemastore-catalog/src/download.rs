use std::collections::HashSet;
use std::path::Path;

use futures::stream::{self, StreamExt};
use tracing::{debug, warn};

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
async fn download_one(client: &reqwest::Client, url: &str, path: &Path) -> anyhow::Result<()> {
    debug!(url = %url, "fetching schema");
    let resp = client.get(url).send().await?.error_for_status()?;
    let text = resp.text().await?;

    // Validate that the response is valid JSON
    let _: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("response from {url} is not valid JSON: {e}"))?;

    tokio::fs::write(path, &text).await?;
    Ok(())
}
