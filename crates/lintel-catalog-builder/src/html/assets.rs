use std::path::Path;

use anyhow::{Context, Result};

const STYLE_CSS: &str = include_str!("templates/style.css");
const APP_JS: &str = include_str!("templates/app.js");

/// Write `style.css` and `app.js` to the output directory.
pub async fn write_assets(output_dir: &Path) -> Result<()> {
    let css_path = output_dir.join("style.css");
    tokio::fs::write(&css_path, STYLE_CSS)
        .await
        .with_context(|| alloc::format!("failed to write {}", css_path.display()))?;

    let js_path = output_dir.join("app.js");
    tokio::fs::write(&js_path, APP_JS)
        .await
        .with_context(|| alloc::format!("failed to write {}", js_path.display()))?;

    Ok(())
}
