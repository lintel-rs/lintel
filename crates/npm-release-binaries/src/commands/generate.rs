use alloc::collections::BTreeMap;
use core::fmt::Write as _;
use std::fs;
use std::io::{Cursor, Read as _};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;

use crate::config::{self, PackageConfig, ResolvedTarget};
use crate::metadata::{self, ResolvedMetadata};

pub struct Options<'a> {
    pub pkg_key: &'a str,
    pub pkg_config: &'a PackageConfig,
    pub version: &'a str,
    pub artifacts_dir: Option<&'a Path>,
    pub output_dir: &'a Path,
    pub skip_artifact_copy: bool,
}

pub fn run(opts: &Options<'_>) -> miette::Result<()> {
    let metadata = metadata::resolve(opts.pkg_key, opts.pkg_config)?;

    // When skipping artifact copy, use a dummy dir so resolve_targets
    // doesn't need archive-base-url — the resolved paths are never accessed.
    let effective_dir = if opts.skip_artifact_copy {
        Some(Path::new("skip"))
    } else {
        opts.artifacts_dir
    };
    let targets =
        config::resolve_targets(opts.pkg_key, opts.pkg_config, opts.version, effective_dir)?;

    generate_main_package(&metadata, &targets, opts)?;

    for target in &targets {
        generate_platform_package(&metadata, target, opts)
            .map_err(|e| miette::miette!("target {}: {e}", target.key))?;
    }

    eprintln!("Generated packages in {}", opts.output_dir.display());
    Ok(())
}

fn generate_main_package(
    metadata: &ResolvedMetadata,
    targets: &[ResolvedTarget],
    opts: &Options<'_>,
) -> miette::Result<()> {
    let version = opts.version;
    let pkg_dir = opts.output_dir.join(&metadata.name);
    let bin_dir = pkg_dir.join("bin");
    fs::create_dir_all(&bin_dir)
        .map_err(|e| miette::miette!("failed to create {}: {e}", bin_dir.display()))?;

    // package.json — insertion order preserved by serde_json::Map
    let optional_deps: BTreeMap<&str, &str> = targets
        .iter()
        .map(|t| (t.package_name.as_str(), version))
        .collect();

    let mut pkg = serde_json::Map::new();
    pkg.insert("name".into(), json!(metadata.name));
    pkg.insert("version".into(), json!(version));
    pkg.insert("description".into(), json!(metadata.description));
    pkg.insert("license".into(), json!(metadata.license));
    pkg.insert(
        "repository".into(),
        json!({ "type": "git", "url": metadata.repository }),
    );
    pkg.insert("engines".into(), json!({ "node": ">=14.21.3" }));
    pkg.insert("homepage".into(), json!(metadata.homepage));
    pkg.insert("keywords".into(), json!(metadata.keywords));
    pkg.insert(
        "bin".into(),
        json!({ &metadata.bin: format!("bin/{}", metadata.bin) }),
    );
    pkg.insert("files".into(), json!(["bin/", "README.md"]));
    pkg.insert("optionalDependencies".into(), json!(optional_deps));
    let package_json = serde_json::Value::Object(pkg);

    let json_str = serde_json::to_string_pretty(&package_json)
        .map_err(|e| miette::miette!("failed to serialize package.json: {e}"))?;
    fs::write(pkg_dir.join("package.json"), format!("{json_str}\n"))
        .map_err(|e| miette::miette!("failed to write package.json: {e}"))?;

    // README.md
    if let Some(readme_path) = opts.pkg_config.readme.as_deref() {
        fs::copy(readme_path, pkg_dir.join("README.md")).map_err(|e| {
            miette::miette!("failed to copy README from {}: {e}", readme_path.display())
        })?;
    } else {
        let readme_content = generate_main_readme(metadata, targets);
        fs::write(pkg_dir.join("README.md"), readme_content)
            .map_err(|e| miette::miette!("failed to write README.md: {e}"))?;
    }

    // bin wrapper
    let wrapper = generate_bin_wrapper(metadata, targets);
    let wrapper_path = bin_dir.join(&metadata.bin);
    fs::write(&wrapper_path, wrapper)
        .map_err(|e| miette::miette!("failed to write bin/{}: {e}", metadata.bin))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&wrapper_path, fs::Permissions::from_mode(0o755)).map_err(|e| {
            miette::miette!("failed to set permissions on bin/{}: {e}", metadata.bin)
        })?;
    }

    Ok(())
}

fn generate_platform_package(
    metadata: &ResolvedMetadata,
    target: &ResolvedTarget,
    opts: &Options<'_>,
) -> miette::Result<()> {
    let version = opts.version;
    let pkg_path = package_dir(opts.output_dir, &target.package_name);
    fs::create_dir_all(&pkg_path)
        .map_err(|e| miette::miette!("failed to create {}: {e}", pkg_path.display()))?;

    if !opts.skip_artifact_copy {
        let binary_data = extract_binary(&target.archive, &target.binary_name)?;
        let dest_path = pkg_path.join(&target.binary_name);
        fs::write(&dest_path, &binary_data)
            .map_err(|e| miette::miette!("failed to write binary: {e}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&dest_path, fs::Permissions::from_mode(0o755))
                .map_err(|e| miette::miette!("failed to set permissions: {e}"))?;
        }
    }

    // package.json — insertion order preserved by serde_json::Map
    let mut pkg = serde_json::Map::new();
    pkg.insert("name".into(), json!(target.package_name));
    pkg.insert("version".into(), json!(version));
    pkg.insert("license".into(), json!(metadata.license));
    pkg.insert(
        "repository".into(),
        json!({ "type": "git", "url": metadata.repository }),
    );
    pkg.insert("engines".into(), json!({ "node": ">=14.21.3" }));
    pkg.insert("homepage".into(), json!(metadata.homepage));
    pkg.insert("keywords".into(), json!(metadata.keywords));
    pkg.insert("os".into(), json!([target.os]));
    pkg.insert("cpu".into(), json!([target.cpu]));
    if let Some(ref libc) = target.libc {
        pkg.insert("libc".into(), json!([libc]));
    }
    let package_json = serde_json::Value::Object(pkg);

    let json_str = serde_json::to_string_pretty(&package_json)
        .map_err(|e| miette::miette!("failed to serialize package.json: {e}"))?;
    fs::write(pkg_path.join("package.json"), format!("{json_str}\n"))
        .map_err(|e| miette::miette!("failed to write package.json: {e}"))?;

    // README.md
    let readme = format!(
        "# {}\n\
         \n\
         Platform-specific binary for [{}]({}) ({} {}).\n\
         \n\
         This package is installed automatically by `{}` — you don't need to install it directly.\n\
         \n\
         ## License\n\
         \n\
         {}\n",
        target.package_name,
        metadata.name,
        metadata.homepage,
        target.os,
        target.cpu,
        metadata.name,
        metadata.license,
    );
    fs::write(pkg_path.join("README.md"), readme)
        .map_err(|e| miette::miette!("failed to write README.md: {e}"))?;

    Ok(())
}

fn get_archive_data(archive: &str) -> miette::Result<Vec<u8>> {
    if archive.starts_with("http://") || archive.starts_with("https://") {
        eprintln!("Downloading {archive}");
        let output = Command::new("curl")
            .args(["-fsSL", archive])
            .output()
            .map_err(|e| miette::miette!("failed to run curl: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(miette::miette!("download failed for {archive}: {stderr}"));
        }
        Ok(output.stdout)
    } else {
        let path = Path::new(archive);
        fs::read(path).map_err(|e| miette::miette!("failed to read {}: {e}", path.display()))
    }
}

#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn extract_binary(archive: &str, binary_name: &str) -> miette::Result<Vec<u8>> {
    let data = get_archive_data(archive)?;

    if archive.ends_with(".tar.gz") || archive.ends_with(".tgz") {
        extract_from_tar_gz(&data, binary_name)
    } else if archive.ends_with(".zip") {
        extract_from_zip(&data, binary_name)
    } else {
        Err(miette::miette!("unsupported archive format: {archive}"))
    }
}

fn extract_from_tar_gz(data: &[u8], binary_name: &str) -> miette::Result<Vec<u8>> {
    let decoder = flate2::read::GzDecoder::new(Cursor::new(data));
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .map_err(|e| miette::miette!("failed to read tar entries: {e}"))?
    {
        let mut entry = entry.map_err(|e| miette::miette!("failed to read tar entry: {e}"))?;
        let path = entry
            .path()
            .map_err(|e| miette::miette!("failed to read entry path: {e}"))?;

        if path.file_name().and_then(|n| n.to_str()) == Some(binary_name) {
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| miette::miette!("failed to read binary from archive: {e}"))?;
            return Ok(buf);
        }
    }

    Err(miette::miette!(
        "binary '{binary_name}' not found in archive"
    ))
}

fn extract_from_zip(data: &[u8], binary_name: &str) -> miette::Result<Vec<u8>> {
    let cursor = Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| miette::miette!("failed to read zip: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| miette::miette!("failed to read zip entry: {e}"))?;

        let path = entry.name().to_string();
        let file_name = Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if file_name == binary_name {
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| miette::miette!("failed to read binary from zip: {e}"))?;
            return Ok(buf);
        }
    }

    Err(miette::miette!(
        "binary '{binary_name}' not found in archive"
    ))
}

fn package_dir(output_dir: &Path, package_name: &str) -> PathBuf {
    if let Some(rest) = package_name.strip_prefix('@') {
        let (scope, name) = rest.split_once('/').expect("scoped package must have /");
        output_dir.join(format!("@{scope}")).join(name)
    } else {
        output_dir.join(package_name)
    }
}

fn generate_main_readme(metadata: &ResolvedMetadata, targets: &[ResolvedTarget]) -> String {
    let mut rows = String::new();
    for target in targets {
        let os_display = match target.os.as_str() {
            "darwin" => "macOS",
            "linux" => "Linux",
            "win32" => "Windows",
            other => other,
        };
        let _ = writeln!(
            rows,
            "| {os_display} | {} | {} |",
            target.cpu, target.package_name
        );
    }

    format!(
        "# {}\n\
         \n\
         {}\n\
         \n\
         ## Install\n\
         \n\
         ```sh\n\
         npm install {}\n\
         ```\n\
         \n\
         ## Supported platforms\n\
         \n\
         | OS | Architecture | Package |\n\
         |---|---|---|\n\
         {}\
         \n\
         ## License\n\
         \n\
         {}\n",
        metadata.name, metadata.description, metadata.name, rows, metadata.license,
    )
}

fn has_musl_targets(targets: &[ResolvedTarget]) -> bool {
    targets.iter().any(|t| t.libc.as_deref() == Some("musl"))
}

fn generate_bin_wrapper(metadata: &ResolvedMetadata, targets: &[ResolvedTarget]) -> String {
    let mut platform_entries = String::new();
    for target in targets {
        let _ = writeln!(
            platform_entries,
            "  \"{}\": \"{}/{}\",",
            target.key, target.package_name, target.binary_name
        );
    }

    let bin = &metadata.bin;
    let name = &metadata.name;
    let repo = &metadata.repository;

    let musl_detection = if has_musl_targets(targets) {
        r#"
function isMusl() {
  let output;
  try {
    output = require("child_process").execSync("ldd --version", {
      stdio: ["pipe", "pipe", "pipe"],
    });
  } catch (err) {
    output = err.stderr;
  }
  return output && output.indexOf("musl") > -1;
}
"#
    } else {
        ""
    };

    let key_expr = if has_musl_targets(targets) {
        r#"let key = `${process.platform}-${process.arch}`;
  if (process.platform === "linux" && isMusl()) {
    key += "-musl";
  }"#
    } else {
        "const key = `${process.platform}-${process.arch}`;"
    };

    format!(
        r#"#!/usr/bin/env node

const PLATFORMS = {{
{platform_entries}}};
{musl_detection}
function getBinaryPath() {{
  {key_expr}
  const pkg = PLATFORMS[key];
  if (!pkg) {{
    throw new Error(
      `{name} doesn't ship with prebuilt binaries for your platform yet (${{key}}). ` +
      `You can still use it by cloning the repo from {repo} ` +
      `and following the instructions there to build from source.`
    );
  }}
  try {{
    return require.resolve(pkg);
  }} catch (e) {{
    throw new Error(
      `Could not find package for ${{key}}. Make sure optional dependencies are installed.\n` +
      `Expected package: ${{pkg}}\n` +
      `Try: npm install {name}`
    );
  }}
}}

let binPath;
try {{
  binPath = getBinaryPath();
}} catch (e) {{
  console.error(e.message);
  process.exit(1);
}}

const result = require("child_process").spawnSync(binPath, process.argv.slice(2), {{
  stdio: "inherit",
}});

if (result.error) {{
  console.error(`Failed to run {bin}: ${{result.error.message}}`);
  process.exit(1);
}}

process.exit(result.status ?? 1);
"#
    )
}
