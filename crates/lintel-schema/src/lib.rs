use anyhow::{Context, Result, bail};
use bpaf::Bpaf;

#[derive(Debug, Clone, Bpaf)]
#[bpaf(generate(schema_command_inner))]
pub enum SchemaCommand {
    #[bpaf(command("migrate"))]
    /// Migrate a JSON Schema to draft 2020-12
    Migrate(#[bpaf(external(migrate_args))] MigrateArgs),
}

/// Construct the bpaf parser for [`SchemaCommand`].
pub fn schema_command() -> impl bpaf::Parser<SchemaCommand> {
    schema_command_inner()
}

#[derive(Debug, Clone, Bpaf)]
pub struct MigrateArgs {
    /// Schema URL (http://, https://, or file://)
    #[bpaf(positional("URL"))]
    pub url: String,
}

/// # Errors
///
/// Returns an error if the schema cannot be fetched, parsed, or migrated.
pub async fn run(cmd: SchemaCommand) -> Result<()> {
    match cmd {
        SchemaCommand::Migrate(args) => run_migrate(args).await,
    }
}

async fn run_migrate(args: MigrateArgs) -> Result<()> {
    let url = url::Url::parse(&args.url).with_context(|| format!("invalid URL: {}", args.url))?;
    let text = fetch_schema(&url).await?;
    let mut value: serde_json::Value =
        serde_json::from_str(&text).context("failed to parse schema as JSON")?;

    jsonschema_migrate::migrate_to_2020_12(&mut value);

    match serde_json::from_value::<jsonschema_migrate::Schema>(value.clone()) {
        Ok(schema) => {
            let output =
                serde_json::to_string_pretty(&schema).context("failed to serialize schema")?;
            println!("{output}");
            Ok(())
        }
        Err(e) => {
            eprintln!("Error: deserialization failed: {e}");
            eprintln!();
            diagnose_schema_value_errors(&value, "");
            std::process::exit(1);
        }
    }
}

/// Recursively try to deserialize each schema position and report failures.
fn diagnose_schema_value_errors(value: &serde_json::Value, path: &str) {
    let serde_json::Value::Object(obj) = value else {
        return;
    };

    // Try the object itself as a Schema — if it fails, report why
    if let Err(e) = serde_json::from_value::<jsonschema_migrate::Schema>(value.clone()) {
        let err_str = e.to_string();
        // Only report if this is a leaf error (not just "didn't match untagged enum")
        if !err_str.contains("did not match any variant") {
            eprintln!("  {path}: {err_str}");
            return;
        }
    } else {
        // This object deserializes fine — no need to recurse
        return;
    }

    // Check single-schema positions
    for key in [
        "if",
        "then",
        "else",
        "not",
        "additionalProperties",
        "items",
        "contains",
        "propertyNames",
        "unevaluatedItems",
        "unevaluatedProperties",
        "contentSchema",
    ] {
        if let Some(v) = obj.get(key) {
            check_schema_value(v, &format!("{path}/{key}"));
        }
    }

    // Check map-schema positions
    for key in [
        "properties",
        "patternProperties",
        "$defs",
        "definitions",
        "dependentSchemas",
    ] {
        if let Some(serde_json::Value::Object(map)) = obj.get(key) {
            for (k, v) in map {
                check_schema_value(v, &format!("{path}/{key}/{k}"));
            }
        }
    }

    // Check array-schema positions
    for key in ["allOf", "anyOf", "oneOf", "prefixItems"] {
        if let Some(serde_json::Value::Array(arr)) = obj.get(key) {
            for (i, v) in arr.iter().enumerate() {
                check_schema_value(v, &format!("{path}/{key}/{i}"));
            }
        }
    }

    // Also check non-schema fields for type mismatches
    for (key, expected) in [
        ("required", "array of strings"),
        ("enum", "array"),
        ("examples", "array"),
        ("type", "string or array"),
    ] {
        if let Some(v) = obj.get(key) {
            let bad = match key {
                "required" | "enum" | "examples" => !matches!(v, serde_json::Value::Array(_)),
                "type" => !matches!(
                    v,
                    serde_json::Value::String(_) | serde_json::Value::Array(_)
                ),
                _ => false,
            };
            if bad {
                eprintln!(
                    "  {path}/{key}: expected {expected}, got {}",
                    value_type_name(v)
                );
            }
        }
    }
}

fn check_schema_value(value: &serde_json::Value, path: &str) {
    match value {
        serde_json::Value::Bool(_) => {} // Always valid as SchemaValue::Bool
        serde_json::Value::Object(_) => diagnose_schema_value_errors(value, path),
        other => {
            eprintln!(
                "  {path}: expected bool or object, got {}: {}",
                value_type_name(other),
                truncate_json(other)
            );
        }
    }
}

fn value_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn truncate_json(value: &serde_json::Value) -> String {
    let s = value.to_string();
    if s.len() > 120 {
        format!("{}...", &s[..120])
    } else {
        s
    }
}

async fn fetch_schema(url: &url::Url) -> Result<String> {
    match url.scheme() {
        "file" => {
            let path = url
                .to_file_path()
                .map_err(|()| anyhow::anyhow!("invalid file URL: {url}"))?;
            tokio::fs::read_to_string(&path)
                .await
                .with_context(|| format!("failed to read {}", path.display()))
        }
        "http" | "https" => {
            let resp = reqwest::get(url.as_str())
                .await
                .with_context(|| format!("failed to fetch {url}"))?;
            if !resp.status().is_success() {
                bail!("HTTP {} for {url}", resp.status());
            }
            resp.text()
                .await
                .with_context(|| format!("failed to read response body from {url}"))
        }
        scheme => bail!("unsupported URL scheme: {scheme}"),
    }
}
