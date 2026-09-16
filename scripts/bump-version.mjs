#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = dirname(dirname(fileURLToPath(import.meta.url)));
const args = new Set(process.argv.slice(2));
const isPostCommit = args.has("--post-commit");
const dryRun = args.has("--dry-run");
const shouldStage = args.has("--stage") || isPostCommit;
const shouldAmend = args.has("--amend") || isPostCommit;
const bumpPart = process.env.YEUTRE_BUMP_PART || "patch";

const versionFiles = [
  "package.json",
  "package-lock.json",
  "src-tauri/tauri.conf.json",
  "src-tauri/Cargo.toml",
];

function log(message) {
  console.log(`[version-bump] ${message}`);
}

function git(args, options = {}) {
  return execFileSync("git", args, {
    cwd: rootDir,
    encoding: "utf8",
    stdio: options.stdio || ["ignore", "pipe", "pipe"],
    env: { ...process.env, ...options.env },
  });
}

function readJson(relativePath) {
  return JSON.parse(readFileSync(join(rootDir, relativePath), "utf8"));
}

function writeJson(relativePath, value) {
  writeFileSync(join(rootDir, relativePath), `${JSON.stringify(value, null, 2)}\n`);
}

function nextVersion(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!match) {
    throw new Error(`Version "${version}" must use x.y.z format.`);
  }

  const parts = match.slice(1).map(Number);
  if (bumpPart === "major") {
    parts[0] += 1;
    parts[1] = 0;
    parts[2] = 0;
  } else if (bumpPart === "minor") {
    parts[1] += 1;
    parts[2] = 0;
  } else if (bumpPart === "patch") {
    parts[2] += 1;
  } else {
    throw new Error(`Unsupported YEUTRE_BUMP_PART="${bumpPart}". Use patch, minor, or major.`);
  }

  return parts.join(".");
}

function dirtyVersionFiles() {
  const output = git(["status", "--porcelain", "--", ...versionFiles]).trim();
  return output
    ? output.split("\n").map((line) => line.trim()).filter(Boolean)
    : [];
}

function updateCargoVersion(next) {
  const cargoPath = join(rootDir, "src-tauri/Cargo.toml");
  const content = readFileSync(cargoPath, "utf8");
  const updated = content.replace(
    /^(\[package\][\s\S]*?^version\s*=\s*")([^"]+)(")/m,
    `$1${next}$3`,
  );
  if (updated === content) {
    throw new Error("Could not find [package] version in src-tauri/Cargo.toml.");
  }
  writeFileSync(cargoPath, updated);
}

function updateVersions(next) {
  const packageJson = readJson("package.json");
  packageJson.version = next;
  writeJson("package.json", packageJson);

  if (existsSync(join(rootDir, "package-lock.json"))) {
    const packageLock = readJson("package-lock.json");
    packageLock.version = next;
    if (packageLock.packages?.[""]) {
      packageLock.packages[""].version = next;
    }
    writeJson("package-lock.json", packageLock);
  }

  const tauriConfig = readJson("src-tauri/tauri.conf.json");
  tauriConfig.version = next;
  writeJson("src-tauri/tauri.conf.json", tauriConfig);

  updateCargoVersion(next);
}

function main() {
  if (process.env.YEUTRE_SKIP_VERSION_BUMP === "1") {
    log("Skipped because YEUTRE_SKIP_VERSION_BUMP=1.");
    return;
  }

  const packageJson = readJson("package.json");
  const current = packageJson.version;
  const next = nextVersion(current);

  if (dryRun) {
    log(`Would bump ${current} -> ${next}.`);
    log(`Files: ${versionFiles.join(", ")}`);
    return;
  }

  const dirty = dirtyVersionFiles();
  if (dirty.length > 0) {
    log("Skipped because version files already have uncommitted changes:");
    dirty.forEach((line) => log(`  ${line}`));
    return;
  }

  updateVersions(next);
  log(`Bumped ${current} -> ${next}.`);

  if (shouldStage) {
    git(["add", "--", ...versionFiles], { stdio: "inherit" });
  }

  if (shouldAmend) {
    git(["commit", "--amend", "--no-edit", "--no-verify"], {
      stdio: "inherit",
      env: { YEUTRE_SKIP_VERSION_BUMP: "1" },
    });
  }
}

try {
  main();
} catch (error) {
  console.error(`[version-bump] ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
