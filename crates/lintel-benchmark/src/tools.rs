use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use anyhow::Result;

use crate::report::{MIN_RUNS, WARMUP_RUNS};
use crate::runner::{Stats, compute_stats, run_timed, which};

/// A JSON Schema validation tool that can be benchmarked.
pub trait Validator {
    /// Display name for the tool.
    fn name(&self) -> &'static str;

    /// Check if the tool is installed.
    fn is_available(&self) -> bool;

    /// Return a version string for display.
    fn version(&self) -> String;

    /// How to install this tool.
    fn install_hint(&self) -> &'static str;

    /// Validate a single file against a schema URL.
    fn validate_single(&self, schema_url: &str, file: &str) -> Vec<String>;

    /// Validate multiple files, each against the same schema.
    fn validate_multi(&self, schema_url: &str, files: &[&str]) -> Vec<Vec<String>>;

    /// Benchmark validating a single file.
    fn bench_single(&self, schema_url: &str, file: &str) -> Result<Stats> {
        let args = self.validate_single(schema_url, file);
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_timed(self.cmd(), &arg_refs, WARMUP_RUNS, MIN_RUNS)
    }

    /// Benchmark validating per-schema batches (for `SchemaStore`).
    fn bench_per_schema(&self, pairs: &[(PathBuf, Vec<PathBuf>)], runs: usize) -> Stats {
        let str_pairs: Vec<(String, Vec<String>)> = pairs
            .iter()
            .map(|(schema, files)| {
                (
                    schema.to_string_lossy().to_string(),
                    files
                        .iter()
                        .map(|f| f.to_string_lossy().to_string())
                        .collect(),
                )
            })
            .collect();

        let mut durations = Vec::with_capacity(runs);
        for _ in 0..runs {
            let start = Instant::now();
            for (schema, files) in &str_pairs {
                let file_refs: Vec<&str> = files.iter().map(String::as_str).collect();
                let batches = self.validate_multi(schema, &file_refs);
                for args in &batches {
                    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
                    let _ = Command::new(self.cmd())
                        .args(&arg_refs)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                }
            }
            durations.push(start.elapsed());
        }

        compute_stats(&durations)
    }

    /// The command name to invoke.
    fn cmd(&self) -> &'static str;
}

// ---------------------------------------------------------------------------
// check-jsonschema
// ---------------------------------------------------------------------------

pub struct CheckJsonschema;

impl Validator for CheckJsonschema {
    fn name(&self) -> &'static str {
        "check-jsonschema"
    }

    fn is_available(&self) -> bool {
        which("check-jsonschema")
    }

    fn version(&self) -> String {
        tool_version("check-jsonschema", "--version")
    }

    fn install_hint(&self) -> &'static str {
        "nix profile install nixpkgs#check-jsonschema"
    }

    fn validate_single(&self, schema_url: &str, file: &str) -> Vec<String> {
        vec![
            "--schemafile".to_string(),
            schema_url.to_string(),
            file.to_string(),
        ]
    }

    fn validate_multi(&self, schema_url: &str, files: &[&str]) -> Vec<Vec<String>> {
        let mut args = vec!["--schemafile".to_string(), schema_url.to_string()];
        args.extend(files.iter().map(|f| (*f).to_string()));
        vec![args]
    }

    fn cmd(&self) -> &'static str {
        "check-jsonschema"
    }
}

// ---------------------------------------------------------------------------
// ajv-cli
// ---------------------------------------------------------------------------

pub struct AjvCli;

impl Validator for AjvCli {
    fn name(&self) -> &'static str {
        "ajv-cli"
    }

    fn is_available(&self) -> bool {
        which("ajv")
    }

    fn version(&self) -> String {
        npm_package_version("ajv-cli")
    }

    fn install_hint(&self) -> &'static str {
        "bun add --global ajv-cli"
    }

    fn validate_single(&self, schema_url: &str, file: &str) -> Vec<String> {
        vec![
            "validate".to_string(),
            "-s".to_string(),
            schema_url.to_string(),
            "-d".to_string(),
            file.to_string(),
        ]
    }

    fn validate_multi(&self, schema_url: &str, files: &[&str]) -> Vec<Vec<String>> {
        let mut args = vec![
            "validate".to_string(),
            "-s".to_string(),
            schema_url.to_string(),
        ];
        for f in files {
            args.push("-d".to_string());
            args.push((*f).to_string());
        }
        vec![args]
    }

    fn cmd(&self) -> &'static str {
        "ajv"
    }
}

// ---------------------------------------------------------------------------
// pajv
// ---------------------------------------------------------------------------

pub struct Pajv;

impl Validator for Pajv {
    fn name(&self) -> &'static str {
        "pajv"
    }

    fn is_available(&self) -> bool {
        which("pajv")
    }

    fn version(&self) -> String {
        npm_package_version("pajv")
    }

    fn install_hint(&self) -> &'static str {
        "bun add --global pajv"
    }

    fn validate_single(&self, schema_url: &str, file: &str) -> Vec<String> {
        vec![
            "validate".to_string(),
            "-s".to_string(),
            schema_url.to_string(),
            "-d".to_string(),
            file.to_string(),
        ]
    }

    fn validate_multi(&self, schema_url: &str, files: &[&str]) -> Vec<Vec<String>> {
        let mut args = vec![
            "validate".to_string(),
            "-s".to_string(),
            schema_url.to_string(),
        ];
        for f in files {
            args.push("-d".to_string());
            args.push((*f).to_string());
        }
        vec![args]
    }

    fn cmd(&self) -> &'static str {
        "pajv"
    }
}

// ---------------------------------------------------------------------------
// jv (santhosh-tekuri/jsonschema)
// ---------------------------------------------------------------------------

pub struct Jv;

impl Validator for Jv {
    fn name(&self) -> &'static str {
        "jv"
    }

    fn is_available(&self) -> bool {
        which("jv")
    }

    fn version(&self) -> String {
        tool_version("jv", "--version")
    }

    fn install_hint(&self) -> &'static str {
        "nix profile install nixpkgs#jsonschema"
    }

    fn validate_single(&self, schema_url: &str, file: &str) -> Vec<String> {
        vec![schema_url.to_string(), file.to_string()]
    }

    fn validate_multi(&self, schema_url: &str, files: &[&str]) -> Vec<Vec<String>> {
        let mut args = vec![schema_url.to_string()];
        args.extend(files.iter().map(|f| (*f).to_string()));
        vec![args]
    }

    fn cmd(&self) -> &'static str {
        "jv"
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Return all known validator tools.
pub fn all_tools() -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(CheckJsonschema),
        Box::new(AjvCli),
        Box::new(Pajv),
        Box::new(Jv),
    ]
}

/// Return only the tools that are installed.
pub fn available_tools() -> Vec<Box<dyn Validator>> {
    all_tools()
        .into_iter()
        .filter(|t| t.is_available())
        .collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tool_version(cmd: &str, flag: &str) -> String {
    Command::new(cmd)
        .arg(flag)
        .output()
        .ok()
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if out.is_empty() {
                String::from_utf8_lossy(&o.stderr).trim().to_string()
            } else {
                out
            }
        })
        .unwrap_or_default()
}

fn npm_package_version(pkg: &str) -> String {
    Command::new("bun")
        .args(["pm", "ls", "-g"])
        .output()
        .ok()
        .and_then(|o| {
            let out = String::from_utf8_lossy(&o.stdout).to_string();
            out.lines()
                .find(|l| l.contains(pkg))
                .and_then(|l| l.split('@').next_back())
                .map(String::from)
        })
        .unwrap_or_else(|| String::from("installed"))
}

/// Install a tool using the appropriate method.
pub fn install(tool: &dyn Validator) -> bool {
    let hint = tool.install_hint();
    let parts: Vec<&str> = hint.split_whitespace().collect();
    if parts.is_empty() {
        return false;
    }
    println!("  Installing {} ...", tool.name());
    let status = Command::new(parts[0]).args(&parts[1..]).status();
    match status {
        Ok(s) if s.success() => {
            println!("  {}: installed", tool.name());
            true
        }
        _ => {
            println!("  {}: failed (try: {})", tool.name(), hint);
            false
        }
    }
}
