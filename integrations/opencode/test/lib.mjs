// Shared helpers for the OpenCode integration tests.

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const testDir = path.dirname(fileURLToPath(import.meta.url));
export const packageDir = path.resolve(testDir, "..");
export const repoRoot = path.resolve(packageDir, "..", "..");

/// Locate the OpenCode CLI: prefer the version pinned in this package, then PATH.
export function opencodeBin() {
  const local = path.join(packageDir, "node_modules", ".bin", "opencode");
  return fs.existsSync(local) ? local : "opencode";
}

/// Locate the ironplcmcp binary: honor IRONPLCMCP_BIN, then the cargo target
/// dirs, then PATH.
export function ironplcmcpBin() {
  if (process.env.IRONPLCMCP_BIN) return process.env.IRONPLCMCP_BIN;
  const candidates = [
    path.join(repoRoot, "compiler", "target", "release", "ironplcmcp"),
    path.join(repoRoot, "compiler", "target", "debug", "ironplcmcp"),
  ];
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) return candidate;
  }
  return "ironplcmcp";
}

/// Create an isolated working directory holding a generated `opencode.json`.
export function makeWorkspace(prefix, config) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  fs.writeFileSync(path.join(dir, "opencode.json"), JSON.stringify(config, null, 2));
  return dir;
}

/// Run an OpenCode command synchronously and return its result.
export function runOpencode(args, { cwd, timeoutMs = 120000, env = {} } = {}) {
  return spawnSync(opencodeBin(), args, {
    cwd,
    timeout: timeoutMs,
    encoding: "utf8",
    env: { ...process.env, ...env },
  });
}

/// Strip ANSI escape sequences so we can match against OpenCode's TUI output.
export function stripAnsi(text) {
  // eslint-disable-next-line no-control-regex
  return (text || "").replace(/\x1b\[[0-9;]*m/g, "");
}

/// Locate OpenCode's own server log directory. OpenCode writes detailed logs
/// (the ones referenced by "Check server logs for details") under its XDG data
/// dir. Honor an explicit override, then try the usual locations.
export function opencodeLogDir() {
  if (process.env.OPENCODE_LOG_DIR) return process.env.OPENCODE_LOG_DIR;
  const dataHome =
    process.env.XDG_DATA_HOME || path.join(os.homedir(), ".local", "share");
  const candidates = [
    path.join(dataHome, "opencode", "log"),
    path.join(os.homedir(), ".opencode", "log"),
  ];
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) return candidate;
  }
  return null;
}

/// Return the contents of the most recently modified OpenCode server log files,
/// newest first, capped to `maxFiles`. Returns "" when no log dir is found.
export function readRecentOpencodeLogs(maxFiles = 2) {
  const dir = opencodeLogDir();
  if (!dir) return "";
  let entries;
  try {
    entries = fs
      .readdirSync(dir)
      .map((name) => path.join(dir, name))
      .filter((file) => fs.statSync(file).isFile())
      .sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs)
      .slice(0, maxFiles);
  } catch {
    return "";
  }
  return entries
    .map((file) => `==== ${file} ====\n${fs.readFileSync(file, "utf8")}`)
    .join("\n\n");
}

/// Read a file, returning a placeholder string when it is missing or empty.
export function readOrPlaceholder(file) {
  try {
    const text = fs.readFileSync(file, "utf8");
    return text.length ? text : "(empty)";
  } catch {
    return "(missing)";
  }
}
