#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { chmodSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = dirname(dirname(fileURLToPath(import.meta.url)));
const hooksDir = join(rootDir, ".githooks");
const postCommitHook = join(hooksDir, "post-commit");

if (!existsSync(postCommitHook)) {
  console.error("[git-hooks] Missing .githooks/post-commit.");
  process.exit(1);
}

if (process.platform !== "win32") {
  chmodSync(postCommitHook, 0o755);
}

execFileSync("git", ["config", "core.hooksPath", ".githooks"], {
  cwd: rootDir,
  stdio: "inherit",
});

console.log("[git-hooks] Enabled .githooks for this repository.");
