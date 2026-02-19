use std::fs;
use std::path::Path;

use anyhow::{bail, Result};

/// Run the `init` command: generate a `lintel.toml` with sensible defaults.
pub fn run() -> Result<()> {
    let config_path = Path::new("lintel.toml");
    if config_path.exists() {
        bail!("lintel.toml already exists");
    }

    let content = r#"# Lintel configuration
# https://github.com/lintel-rs/lintel

# Uncomment to stop parent config inheritance:
# root = true

# Exclude patterns (glob)
exclude = []

# Custom schema mappings: glob pattern -> schema URL
# [schemas]
# "my-config.json" = "https://example.com/schema.json"

# Additional schema catalog registries (SchemaStore format)
# registries = []

# Schema URI rewrite rules
# [rewrite]
# "https://old-domain.com/" = "https://new-domain.com/"

# Per-file overrides
# [[override]]
# files = ["vendor/**"]
# validate_formats = false
"#;

    fs::write(config_path, content)?;
    eprintln!("Created lintel.toml");
    Ok(())
}
