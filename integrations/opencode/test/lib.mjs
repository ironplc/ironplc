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
///
/// OpenCode's `run` resolves the project directory from `process.env.PWD ??
/// process.cwd()` — it prefers PWD. `spawnSync`'s `cwd` option changes the
/// child's actual working directory but does NOT rewrite the inherited PWD, so
/// a stale PWD (e.g. the repo dir the test was launched from) would make
/// OpenCode create its session there instead of in `cwd`, missing the inline
/// provider config and failing with ProviderModelNotFoundError. Keep PWD in
/// sync with cwd so OpenCode anchors to the workspace we set up.
export function runOpencode(args, { cwd, timeoutMs = 120000, env = {} } = {}) {
  return spawnSync(opencodeBin(), args, {
    cwd,
    timeout: timeoutMs,
    encoding: "utf8",
    env: { ...process.env, ...(cwd ? { PWD: cwd } : {}), ...env },
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

// --- Recording MCP server: shared config + assertions ------------------------
//
// Both the mock and real-agent lanes point OpenCode at a thin wrapper
// (`test/record-mcp.mjs`) around `ironplcmcp` that tees the JSON-RPC stream to
// `${recordLog}.in` (OpenCode -> server), `.out` (server -> OpenCode) and
// `.err`. Asserting on that recording keeps the tests decoupled from OpenCode's
// human-facing output format.

/// The `mcp` config fragment that launches the recording wrapper. `wrapper` is
/// the path to `record-mcp.mjs`; `bin` is the real `ironplcmcp` it wraps.
export function recordingMcpConfig({ bin, wrapper, recordLog }) {
  return {
    ironplc: {
      type: "local",
      command: ["node", wrapper],
      enabled: true,
      environment: { IRONPLCMCP_BIN: bin, IRONPLCMCP_RECORD_LOG: recordLog },
    },
  };
}

/// Remove any traffic recorded by a previous attempt.
export function resetRecordLogs(recordLog) {
  for (const suffix of [".in", ".out", ".err"]) {
    try {
      fs.rmSync(`${recordLog}${suffix}`, { force: true });
    } catch {
      /* ignore */
    }
  }
}

/// The tool was invoked when the recording wrapper saw a `tools/call` for it.
/// OpenCode exposes the tool to the model as `ironplc_check`, but the JSON-RPC
/// `tools/call` it sends to the server uses the server-side name `check`.
export function toolWasInvoked(recordLog) {
  try {
    const text = fs.readFileSync(`${recordLog}.in`, "utf8");
    return /"method"\s*:\s*"tools\/call"[\s\S]*?"name"\s*:\s*"check"/.test(text);
  } catch {
    return false;
  }
}

/// The compiler responded when the server emitted a check result (which always
/// carries an `ok` field and, for a broken program, diagnostics).
///
/// The result travels as an MCP `tools/call` response whose payload is nested:
/// the compiler's JSON is serialized into a `content[].text` string, so its
/// quotes arrive escaped (`\"diagnostics\"`). Parse the JSON-RPC envelope and
/// inspect the decoded text rather than regexing the escaped bytes.
export function compilerResponded(recordLog) {
  let text;
  try {
    text = fs.readFileSync(`${recordLog}.out`, "utf8");
  } catch {
    return false;
  }
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    let message;
    try {
      message = JSON.parse(trimmed);
    } catch {
      continue; // a framed/partial line we cannot parse on its own
    }
    const content = message?.result?.content;
    if (!Array.isArray(content)) continue;
    for (const block of content) {
      if (block?.type !== "text" || typeof block.text !== "string") continue;
      if (/"diagnostics"/.test(block.text) || /"ok"\s*:/.test(block.text)) {
        return true;
      }
    }
  }
  return false;
}

/// Parse the newline-delimited JSON-RPC messages OpenCode sent to the server and
/// return the `tools/call` request for `toolName`, or null if it never called
/// it. Used to inspect the arguments OpenCode actually serialized.
export function invokedToolCall(recordLog, toolName = "check") {
  let text;
  try {
    text = fs.readFileSync(`${recordLog}.in`, "utf8");
  } catch {
    return null;
  }
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    let message;
    try {
      message = JSON.parse(trimmed);
    } catch {
      continue; // a framed/partial line we cannot parse on its own
    }
    if (message.method === "tools/call" && message.params?.name === toolName) {
      return message;
    }
  }
  return null;
}

/// Validate that the `check` call OpenCode sent carries well-formed arguments.
/// This is the real regression guard: the defect that motivated changing the
/// tool arguments was OpenCode failing to serialize a particular parameter
/// shape, so we assert the shape survived the round-trip intact.
export function validateCheckArguments(recordLog) {
  const call = invokedToolCall(recordLog, "check");
  if (!call) return { ok: false, reason: "no `check` tools/call was recorded" };
  const args = call.params?.arguments;
  if (!args || typeof args !== "object") {
    return { ok: false, reason: "arguments are missing or not an object" };
  }
  const { sources, options } = args;
  if (!Array.isArray(sources) || sources.length === 0) {
    return { ok: false, reason: "`sources` is not a non-empty array" };
  }
  const badSource = sources.find(
    (s) => typeof s?.name !== "string" || typeof s?.content !== "string",
  );
  if (badSource) {
    return { ok: false, reason: "a source is missing a string `name`/`content`" };
  }
  if (!options || typeof options !== "object") {
    return { ok: false, reason: "`options` is not an object" };
  }
  return { ok: true };
}
