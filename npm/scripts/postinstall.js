#!/usr/bin/env node

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const https = require("https");
const { createGunzip } = require("zlib");
const { pipeline } = require("stream/promises");
const { createWriteStream } = require("fs");

const PACKAGE = require("../package.json");
const VERSION = PACKAGE.version;
const BASE_URL = `https://github.com/lintel-rs/lintel/releases/download/v${VERSION}`;

function getPlatformTarget() {
  const platform = process.platform;
  const arch = process.arch;

  const targets = {
    "darwin-x64": "x86_64-apple-darwin",
    "darwin-arm64": "aarch64-apple-darwin",
    "linux-x64": "x86_64-unknown-linux-gnu",
    "linux-arm64": "aarch64-unknown-linux-gnu",
    "win32-x64": "x86_64-pc-windows-msvc",
    "win32-arm64": "aarch64-pc-windows-msvc",
  };

  const key = `${platform}-${arch}`;
  const target = targets[key];
  if (!target) {
    console.error(`Unsupported platform: ${key}`);
    console.error(
      "Lintel does not provide a prebuilt binary for this platform.",
    );
    console.error("You can build from source: cargo install lintel");
    process.exit(1);
  }
  return target;
}

function getBinaryName() {
  return process.platform === "win32" ? "lintel.exe" : "lintel";
}

function download(url) {
  return new Promise((resolve, reject) => {
    https
      .get(url, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          return download(res.headers.location).then(resolve, reject);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`Download failed: HTTP ${res.statusCode}`));
        }
        resolve(res);
      })
      .on("error", reject);
  });
}

async function main() {
  const target = getPlatformTarget();
  const binaryName = getBinaryName();
  const ext = process.platform === "win32" ? "zip" : "tar.gz";
  const archiveName = `lintel-${target}.${ext}`;
  const url = `${BASE_URL}/${archiveName}`;
  const binDir = path.join(__dirname, "..", "bin");
  const binPath = path.join(binDir, binaryName);

  // Skip if binary already exists (e.g. from a local build)
  if (fs.existsSync(binPath)) {
    return;
  }

  fs.mkdirSync(binDir, { recursive: true });

  console.log(`Downloading lintel v${VERSION} for ${target}...`);

  try {
    const response = await download(url);

    if (ext === "tar.gz") {
      const tmpFile = path.join(binDir, archiveName);
      await pipeline(response, createWriteStream(tmpFile));
      execSync(`tar -xzf ${archiveName} -C .`, { cwd: binDir });
      fs.unlinkSync(tmpFile);
    } else {
      const tmpFile = path.join(binDir, archiveName);
      await pipeline(response, createWriteStream(tmpFile));
      execSync(`unzip -o ${archiveName} -d .`, { cwd: binDir });
      fs.unlinkSync(tmpFile);
    }

    if (process.platform !== "win32") {
      fs.chmodSync(binPath, 0o755);
    }

    console.log(`Installed lintel v${VERSION}`);
  } catch (err) {
    console.error(`Failed to download lintel: ${err.message}`);
    console.error("You can install manually: cargo install lintel");
    process.exit(1);
  }
}

main();
