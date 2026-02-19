use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::report::{MIN_RUNS, Report, WARMUP_RUNS};
use crate::runner::{run_timed, run_timed_with_setup};
use crate::tools::{Validator, available_tools};

const SCHEMASTORE_REPO: &str = "https://github.com/SchemaStore/schemastore.git";
const PKG_JSON_SCHEMA: &str = "https://json.schemastore.org/package.json";
const TSCONFIG_SCHEMA: &str = "https://json.schemastore.org/tsconfig.json";
const GH_WORKFLOW_SCHEMA: &str = "https://json.schemastore.org/github-workflow.json";

#[derive(Debug, Clone)]
pub enum Filter {
    Single,
    Multi,
    Repo,
}

impl std::str::FromStr for Filter {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "single" => Ok(Self::Single),
            "multi" => Ok(Self::Multi),
            "repo" => Ok(Self::Repo),
            _ => Err(format!(
                "unknown filter '{s}', expected: single, multi, repo"
            )),
        }
    }
}

pub fn run(filter: Option<&Filter>) -> Result<()> {
    let lintel_bin = find_lintel()?;
    let lintel = lintel_bin.to_string_lossy().to_string();

    println!("Lintel benchmark");
    println!("  binary: {lintel}");

    let version_output = Command::new(&lintel).arg("version").output()?;
    let version = String::from_utf8_lossy(&version_output.stdout)
        .trim()
        .to_string();
    if !version.is_empty() {
        println!("  version: {version}");
    }
    println!();

    let tools = available_tools();
    println!("Comparing against:");
    if tools.is_empty() {
        println!("  (none found — run `lintel-benchmark setup` to install)");
    } else {
        for tool in &tools {
            println!("  {:<20} {}", tool.name(), tool.version());
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .context("cannot find repo root")?;
    let fixtures = repo_root.join("benchmarks").join("fixtures");
    if !fixtures.exists() {
        bail!("Fixtures directory not found at {}", fixtures.display());
    }
    let schemastore_dir = repo_root.join("benchmarks").join(".schemastore");
    let output_path = repo_root.join("benchmarks").join("README.md");

    let mut report = Report::new(&version);

    match filter {
        Some(Filter::Single) => {
            bench_single_file(&lintel, &fixtures, &tools, &mut report)?;
        }
        Some(Filter::Multi) => {
            bench_multi_file(&lintel, &fixtures, &tools, &mut report)?;
        }
        Some(Filter::Repo) => {
            ensure_schemastore(&schemastore_dir)?;
            bench_schemastore(&lintel, &schemastore_dir, &tools, &mut report)?;
        }
        None => {
            bench_single_file(&lintel, &fixtures, &tools, &mut report)?;
            bench_multi_file(&lintel, &fixtures, &tools, &mut report)?;
            ensure_schemastore(&schemastore_dir)?;
            bench_schemastore(&lintel, &schemastore_dir, &tools, &mut report)?;
        }
    }

    report.write_markdown(&output_path)?;

    println!();
    println!("=== Done ===");
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_lintel() -> Result<PathBuf> {
    let self_path = std::env::current_exe()?;
    let target_dir = self_path
        .parent()
        .and_then(|p| p.parent())
        .context("cannot find target directory")?;
    let release_bin = target_dir.join("release").join("lintel");
    if release_bin.exists() {
        return Ok(release_bin);
    }

    let output = Command::new("which").arg("lintel").output()?;
    if output.status.success() {
        let path = String::from_utf8(output.stdout)?.trim().to_string();
        return Ok(PathBuf::from(path));
    }

    bail!("Cannot find lintel binary. Build with: cargo build --release --package lintel");
}

fn ensure_schemastore(dir: &Path) -> Result<()> {
    if dir.exists() {
        println!("  SchemaStore already cloned at {}", dir.display());
        return Ok(());
    }
    println!("  Cloning SchemaStore (shallow)...");
    let status = Command::new("git")
        .args(["clone", "--depth", "1", SCHEMASTORE_REPO])
        .arg(dir)
        .status()?;
    if !status.success() {
        bail!("Failed to clone SchemaStore");
    }
    Ok(())
}

fn prime_lintel(lintel: &str, target: &str) {
    let _ = Command::new(lintel)
        .args(["check", target])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Collect (schema, test-files) pairs from `SchemaStore` layout.
/// Each `src/test/<name>/` dir is paired with `src/schemas/json/<name>.json`.
fn collect_schema_test_pairs(schemastore_src: &Path) -> Vec<(PathBuf, Vec<PathBuf>)> {
    let schemas_dir = schemastore_src.join("schemas").join("json");
    let test_dir = schemastore_src.join("test");
    let mut pairs = Vec::new();

    let Ok(entries) = std::fs::read_dir(&test_dir) else {
        return pairs;
    };

    for entry in entries.filter_map(Result::ok) {
        let test_subdir = entry.path();
        if !test_subdir.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let schema = schemas_dir.join(format!("{}.json", name.to_string_lossy()));
        if !schema.exists() {
            continue;
        }

        let files: Vec<PathBuf> = walkdir::WalkDir::new(&test_subdir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(walkdir::DirEntry::into_path)
            .collect();

        if !files.is_empty() {
            pairs.push((schema, files));
        }
    }

    pairs
}

// ---------------------------------------------------------------------------
// Single file benchmarks
// ---------------------------------------------------------------------------

struct SingleFileBench<'a> {
    section_title: &'a str,
    schema_url: &'a str,
    file_path: String,
}

fn bench_single_file(
    lintel: &str,
    fixtures: &Path,
    tools: &[Box<dyn Validator>],
    report: &mut Report,
) -> Result<()> {
    let benches = [
        SingleFileBench {
            section_title: "Single file: package.json",
            schema_url: PKG_JSON_SCHEMA,
            file_path: fixtures.join("package.json").to_string_lossy().to_string(),
        },
        SingleFileBench {
            section_title: "Single file: tsconfig.json",
            schema_url: TSCONFIG_SCHEMA,
            file_path: fixtures.join("tsconfig.json").to_string_lossy().to_string(),
        },
        SingleFileBench {
            section_title: "Single file: github-ci.yml (YAML)",
            schema_url: GH_WORKFLOW_SCHEMA,
            file_path: fixtures.join("github-ci.yml").to_string_lossy().to_string(),
        },
    ];

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("lintel");

    for (i, bench) in benches.iter().enumerate() {
        report.section(bench.section_title);
        prime_lintel(lintel, &bench.file_path);

        // lintel with various cache modes
        report.add(
            "lintel check (warm cache)",
            run_timed(lintel, &["check", &bench.file_path], WARMUP_RUNS, MIN_RUNS)?,
        );

        // Only run --force-validation on first file to keep benchmark concise
        if i == 0 {
            report.add(
                "lintel check --force-validation",
                run_timed(
                    lintel,
                    &["check", "--force-validation", &bench.file_path],
                    WARMUP_RUNS,
                    MIN_RUNS,
                )?,
            );
        }

        report.add(
            "lintel check --force (no cache)",
            run_timed(
                lintel,
                &["check", "--force", &bench.file_path],
                WARMUP_RUNS,
                MIN_RUNS,
            )?,
        );

        // Cold start only on first file
        if i == 0 {
            let cache_clone = cache_dir.clone();
            report.add(
                "lintel check (cold start, empty disk cache)",
                run_timed_with_setup(
                    || {
                        if cache_clone.exists() {
                            std::fs::remove_dir_all(&cache_clone)?;
                        }
                        Ok(())
                    },
                    lintel,
                    &["check", &bench.file_path],
                    5,
                )?,
            );
        }

        // Other tools
        for tool in tools {
            report.add(
                tool.name(),
                tool.bench_single(bench.schema_url, &bench.file_path)?,
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Multi-file benchmarks
// ---------------------------------------------------------------------------

fn bench_multi_file(
    lintel: &str,
    fixtures: &Path,
    tools: &[Box<dyn Validator>],
    report: &mut Report,
) -> Result<()> {
    let pkg_str = fixtures.join("package.json").to_string_lossy().to_string();
    let tsconfig_str = fixtures.join("tsconfig.json").to_string_lossy().to_string();
    let ci_str = fixtures.join("github-ci.yml").to_string_lossy().to_string();

    report.section("Multiple files (package.json + tsconfig.json + github-ci.yml)");

    prime_lintel(lintel, &pkg_str);
    prime_lintel(lintel, &tsconfig_str);
    prime_lintel(lintel, &ci_str);

    let fixtures_str = fixtures.to_string_lossy().to_string();
    report.add(
        "lintel check <dir> (warm cache)",
        run_timed(lintel, &["check", &fixtures_str], WARMUP_RUNS, MIN_RUNS)?,
    );
    report.add(
        "lintel check <dir> --force-validation",
        run_timed(
            lintel,
            &["check", "--force-validation", &fixtures_str],
            WARMUP_RUNS,
            MIN_RUNS,
        )?,
    );
    report.add(
        "lintel check <dir> --force (no cache)",
        run_timed(
            lintel,
            &["check", "--force", &fixtures_str],
            WARMUP_RUNS,
            MIN_RUNS,
        )?,
    );

    // Other tools: validate all 3 files with their respective schemas
    let file_pairs = [
        (PKG_JSON_SCHEMA, pkg_str.as_str()),
        (TSCONFIG_SCHEMA, tsconfig_str.as_str()),
        (GH_WORKFLOW_SCHEMA, ci_str.as_str()),
    ];

    for tool in tools {
        let pairs: Vec<(PathBuf, Vec<PathBuf>)> = file_pairs
            .iter()
            .map(|(schema, file)| (PathBuf::from(schema), vec![PathBuf::from(file)]))
            .collect();
        report.add(
            &format!("{} (3 invocations)", tool.name()),
            tool.bench_per_schema(&pairs, MIN_RUNS),
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// SchemaStore benchmarks
// ---------------------------------------------------------------------------

fn bench_schemastore(
    lintel: &str,
    schemastore_dir: &Path,
    tools: &[Box<dyn Validator>],
    report: &mut Report,
) -> Result<()> {
    let src_dir = schemastore_dir.join("src");
    let src_str = src_dir.to_string_lossy().to_string();

    let pairs = collect_schema_test_pairs(&src_dir);
    let total_files: usize = pairs.iter().map(|(_, files)| files.len()).sum();

    report.section(&format!(
        "SchemaStore repo ({schemas} schemas, {total_files} test files)",
        schemas = pairs.len(),
    ));

    prime_lintel(lintel, &src_str);

    report.add(
        "lintel check <src> (warm cache)",
        run_timed(lintel, &["check", &src_str], 1, 5)?,
    );
    report.add(
        "lintel check <src> --force-validation",
        run_timed(lintel, &["check", "--force-validation", &src_str], 1, 5)?,
    );
    report.add(
        "lintel check <src> --force (no cache)",
        run_timed(lintel, &["check", "--force", &src_str], 0, 3)?,
    );

    for tool in tools {
        report.add(
            &format!("{} ({} schemas x test files)", tool.name(), pairs.len()),
            tool.bench_per_schema(&pairs, 3),
        );
    }

    Ok(())
}
