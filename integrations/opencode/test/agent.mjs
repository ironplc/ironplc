// Layer 3 — real-agent end-to-end test.
//
// Drives `opencode run` with a local, key-free Ollama model and the IronPLC MCP
// server configured. Asserts that the agent actually invokes the `check` tool
// and that the IronPLC compiler returns diagnostics for a deliberately broken
// program. Tool invocation is observed through a recording wrapper around
// `ironplcmcp` (see record-mcp.mjs), so the assertion does not depend on
// OpenCode's output format.
//
// The model is intentionally small (CPU-friendly) and therefore not perfectly
// reliable at choosing tools, so the prompt is highly directive and the run is
// retried a few times.
//
// Configuration (environment variables):
//   OPENCODE_E2E_MODEL     provider/model, default "ollama/llama3.2:3b"
//   OLLAMA_BASE_URL        default "http://localhost:11434/v1"
//   OPENCODE_E2E_ATTEMPTS  retry budget, default 3
//   IRONPLCMCP_BIN         path to ironplcmcp (see lib.mjs for fallbacks)

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  ironplcmcpBin,
  makeWorkspace,
  readOrPlaceholder,
  readRecentOpencodeLogs,
  runOpencode,
  testDir,
} from "./lib.mjs";

const model = process.env.OPENCODE_E2E_MODEL || "ollama/llama3.2:3b";
const [providerId, ...modelParts] = model.split("/");
const modelId = modelParts.join("/");
const ollamaBaseUrl = process.env.OLLAMA_BASE_URL || "http://localhost:11434/v1";
const attempts = Number(process.env.OPENCODE_E2E_ATTEMPTS || 3);

const bin = ironplcmcpBin();
const wrapper = path.join(testDir, "record-mcp.mjs");

// One workspace per attempt would also work; we reset the log between attempts.
const tmpBase = process.env.RUNNER_TEMP || os.tmpdir();
const recordLog = path.join(
  fs.mkdtempSync(path.join(tmpBase, "opencode-agent-")),
  "mcp-record",
);

const workspace = makeWorkspace("opencode-agentws-", {
  $schema: "https://opencode.ai/config.json",
  provider: {
    [providerId]: {
      npm: "@ai-sdk/openai-compatible",
      name: providerId,
      options: { baseURL: ollamaBaseUrl, apiKey: "ollama-local" },
      models: { [modelId]: { name: modelId, tools: true } },
    },
  },
  mcp: {
    ironplc: {
      type: "local",
      command: ["node", wrapper],
      enabled: true,
      environment: { IRONPLCMCP_BIN: bin, IRONPLCMCP_RECORD_LOG: recordLog },
    },
  },
});

const program =
  "FUNCTION_BLOCK fb\\nVAR x : BOOL := notabool; END_VAR\\nEND_FUNCTION_BLOCK";
const prompt = [
  "You have access to MCP tools from the IronPLC compiler, including a tool named `ironplc_check`.",
  "Call the `ironplc_check` tool exactly once with these arguments to validate an IEC 61131-3 program:",
  `  sources = [{ "name": "main.st", "content": "${program}" }]`,
  '  options = { "dialect": "iec61131-3-ed2" }',
  "After the tool returns, briefly report the diagnostics it produced.",
].join("\n");

/// The check tool was invoked when the recording wrapper saw a tools/call for
/// it. OpenCode exposes the tool to the model as `ironplc_check`, but the
/// JSON-RPC `tools/call` it sends to the server uses the server-side name `check`.
function toolWasInvoked() {
  const file = `${recordLog}.in`;
  if (!fs.existsSync(file)) return false;
  const text = fs.readFileSync(file, "utf8");
  return /"method"\s*:\s*"tools\/call"[\s\S]*?"name"\s*:\s*"check"/.test(text);
}

/// The compiler responded when the server emitted a check result (which always
/// carries an `ok` field and, for our broken program, diagnostics).
function compilerResponded() {
  const file = `${recordLog}.out`;
  if (!fs.existsSync(file)) return false;
  const text = fs.readFileSync(file, "utf8");
  return /"diagnostics"/.test(text) || /"ok"\s*:/.test(text);
}

function resetLogs() {
  for (const suffix of [".in", ".out"]) {
    try {
      fs.rmSync(`${recordLog}${suffix}`, { force: true });
    } catch {
      /* ignore */
    }
  }
}

console.log(`Model: ${model}  Ollama: ${ollamaBaseUrl}  ironplcmcp: ${bin}`);

let lastResult = { stdout: "", stderr: "" };
for (let attempt = 1; attempt <= attempts; attempt++) {
  resetLogs();
  console.log(`Attempt ${attempt}/${attempts}: asking the agent to call ironplc_check...`);
  // `--print-logs` and `--log-level DEBUG` route OpenCode's own server logs to
  // stderr. Without them, a server-side failure surfaces only as an opaque
  // "Unexpected server error. Check server logs for details." message.
  const result = runOpencode(
    [
      "run",
      "--model",
      model,
      "--log-level",
      "DEBUG",
      "--print-logs",
      "--dangerously-skip-permissions",
      prompt,
    ],
    { cwd: workspace, timeoutMs: 300000 },
  );
  lastResult = result;

  const invoked = toolWasInvoked();
  const responded = compilerResponded();
  console.log(`  tool invoked: ${invoked}, compiler responded: ${responded}`);

  if (invoked && responded) {
    console.log(
      "PASS: the agent invoked ironplc_check and the IronPLC compiler returned diagnostics.",
    );
    process.exit(0);
  }
}

console.error(
  "FAIL: the agent did not invoke the IronPLC check tool within the attempt budget.",
);

// Dump every diagnostic channel so the failure can be understood from CI logs
// alone, without re-running locally.
console.error("\n==== last OpenCode stdout ====");
console.error(lastResult.stdout || "(empty)");
console.error("\n==== last OpenCode stderr ====");
console.error(lastResult.stderr || "(empty)");
if (lastResult.error) {
  console.error("\n==== OpenCode spawn error ====");
  console.error(`${lastResult.error.message} (signal: ${lastResult.signal ?? "none"})`);
}

console.error("\n==== MCP traffic: OpenCode -> server (.in) ====");
console.error(readOrPlaceholder(`${recordLog}.in`));
console.error("\n==== MCP traffic: server -> OpenCode (.out) ====");
console.error(readOrPlaceholder(`${recordLog}.out`));
console.error("\n==== ironplcmcp stderr (.err) ====");
console.error(readOrPlaceholder(`${recordLog}.err`));

const serverLogs = readRecentOpencodeLogs();
console.error("\n==== OpenCode server logs ====");
console.error(serverLogs || "(no OpenCode log directory found)");

process.exit(1);
