#!/usr/bin/env node

// This script runs at `npm install` time. It resolves the correct
// platform-specific binary from the optionalDependencies and copies
// (or symlinks) it into ./bin/grep4ai so the `"bin"` field works.

"use strict";

const fs = require("fs");
const path = require("path");
const os = require("os");

// Map Node.js platform/arch to our package names
const PLATFORMS = {
  "win32-x64": "@grep4ai/win32-x64",
  "linux-x64": "@grep4ai/linux-x64",
  "linux-arm64": "@grep4ai/linux-arm64",
  "darwin-x64": "@grep4ai/darwin-x64",
  "darwin-arm64": "@grep4ai/darwin-arm64",
};

function getPlatformPackage() {
  const platform = os.platform();
  const arch = os.arch();
  const key = `${platform}-${arch}`;
  const pkg = PLATFORMS[key];

  if (!pkg) {
    console.error(
      `grep4ai: Unsupported platform ${platform}-${arch}.\n` +
      `Supported: ${Object.keys(PLATFORMS).join(", ")}\n` +
      `You can build from source: cargo install grep4ai`
    );
    process.exit(1);
  }

  return pkg;
}

function getBinaryName() {
  return os.platform() === "win32" ? "grep4ai.exe" : "grep4ai";
}

function main() {
  const pkgName = getPlatformPackage();
  const binaryName = getBinaryName();

  // Try to resolve the platform-specific package
  let pkgDir;
  try {
    // The platform package exports its binary path
    pkgDir = path.dirname(require.resolve(`${pkgName}/package.json`));
  } catch (e) {
    console.error(
      `grep4ai: Could not find platform package ${pkgName}.\n` +
      `This usually means the optional dependency was not installed.\n` +
      `Try: npm install ${pkgName}\n` +
      `Or build from source: cargo install grep4ai`
    );
    process.exit(1);
  }

  const sourceBinary = path.join(pkgDir, "bin", binaryName);
  const targetDir = path.join(__dirname, "bin");
  const targetBinary = path.join(targetDir, binaryName);

  if (!fs.existsSync(sourceBinary)) {
    console.error(
      `grep4ai: Binary not found at ${sourceBinary}\n` +
      `The platform package may be corrupt. Try reinstalling.`
    );
    process.exit(1);
  }

  // Ensure bin directory exists
  if (!fs.existsSync(targetDir)) {
    fs.mkdirSync(targetDir, { recursive: true });
  }

  // Copy the binary
  fs.copyFileSync(sourceBinary, targetBinary);

  // Make executable on Unix
  if (os.platform() !== "win32") {
    fs.chmodSync(targetBinary, 0o755);
  }

  console.log(`grep4ai: Installed ${pkgName} binary successfully.`);
}

main();
