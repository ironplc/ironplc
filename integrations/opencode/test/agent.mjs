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
import { ironplcmcpBin, makeWorkspace, readOrPlaceholder, runOpencode, testDir } from "./lib.mjs";

const model = process.env.OPENCODE_E2E_MODEL || "ollama/llama3.2:3b";
const [providerId, ...modelParts] = model.split("/");
const modelId = modelParts.join("/");
const ollamaBaseUrl = process.env.OLLAMA_BASE_URL || "http://localhost:11434/v1";
const attempts = Number(process.env.OPENCODE_E2E_ATTEMPTS || 3);

// OpenCode 1.17.x can throw `ProviderModelNotFoundError` from `getModel`,
// before it ever contacts the model, when resolving a config-defined provider
// (here `@ai-sdk/openai-compatible`). It is not an IronPLC fault — `opencode
// models` lists the model and the deterministic layers (connectivity smoke +
// Rust schema guard) still pass — and it is not something this harness can fix
// (a warm-up run and `--title` were both tried and made no difference). We
// detect it so the run can soft-skip instead of blocking the release pipeline.
const PROVIDER_NOT_FOUND = "ProviderModelNotFoundError";

/// Block synchronously for `ms` milliseconds (no async in this linear script).
function sleepSync(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

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

/// Run `opencode run` with the given prompt. `--print-logs` and
/// `--log-level DEBUG` route OpenCode's own server logs to stderr; without them
/// a server-side failure surfaces only as an opaque "Unexpected server error.
/// Check server logs for details." message.
function runAgent(userPrompt, timeoutMs) {
  return runOpencode(
    [
      "run",
      "--model",
      model,
      "--log-level",
      "DEBUG",
      "--print-logs",
      "--dangerously-skip-permissions",
      userPrompt,
    ],
    { cwd: workspace, timeoutMs },
  );
}

let lastResult = { stdout: "", stderr: "" };
let providerInitFailures = 0;
for (let attempt = 1; attempt <= attempts; attempt++) {
  resetLogs();
  console.log(`Attempt ${attempt}/${attempts}: asking the agent to call ironplc_check...`);
  const result = runAgent(prompt, 300000);
  lastResult = result;

  const invoked = toolWasInvoked();
  const responded = compilerResponded();
  const providerInitFailed = new RegExp(PROVIDER_NOT_FOUND).test(result.stderr || "");
  if (providerInitFailed) providerInitFailures++;
  console.log(
    `  tool invoked: ${invoked}, compiler responded: ${responded}` +
      (providerInitFailed ? `, provider init failed: true` : ""),
  );

  if (invoked && responded) {
    console.log(
      "PASS: the agent invoked ironplc_check and the IronPLC compiler returned diagnostics.",
    );
    process.exit(0);
  }

  // A provider-init failure is transient; give OpenCode a moment before retry.
  if (providerInitFailed && attempt < attempts) sleepSync(2000);
}

// When every attempt fails the same way inside OpenCode's model resolution
// (`ProviderModelNotFoundError`, thrown in `getModel` before the MCP server is
// ever contacted), the agent never actually ran. That is an OpenCode/provider
// initialization problem, not an IronPLC regression: the real MCP contract is
// covered deterministically by the connectivity smoke (Layer 2) and the Rust
// schema guard (`compiler/mcp/tests/cli.rs`). Soft-skip rather than block the
// release pipeline on a flaky upstream condition we cannot drive from here.
const openCodeInfraFailure = providerInitFailures === attempts && !toolWasInvoked();
if (openCodeInfraFailure) {
  console.warn(
    `SKIP: every attempt hit OpenCode's ${PROVIDER_NOT_FOUND} during model ` +
      "resolution, so the agent never ran and the IronPLC MCP server was never " +
      "reached. This is an OpenCode/provider initialization problem, not an " +
      "IronPLC tool failure; the MCP contract is still covered by the " +
      "connectivity smoke and the Rust schema guard. Not failing the build. " +
      "Diagnostics below.",
  );
} else {
  console.error(
    "FAIL: the agent did not invoke the IronPLC check tool within the attempt budget.",
  );
}

// Dump every diagnostic channel so the outcome can be understood from CI logs
// alone, without re-running locally.
const log = (...args) => (openCodeInfraFailure ? console.warn(...args) : console.error(...args));
log("\n==== last OpenCode stdout ====");
log(lastResult.stdout || "(empty)");
log("\n==== last OpenCode stderr ====");
log(lastResult.stderr || "(empty)");
if (lastResult.error) {
  log("\n==== OpenCode spawn error ====");
  log(`${lastResult.error.message} (signal: ${lastResult.signal ?? "none"})`);
}

// OpenCode's own DEBUG server logs are already in the stderr above (via
// `--print-logs`) and scoped to this run, so we don't also read its on-disk
// rolling log, which would mix in prior runs.
log("\n==== MCP traffic: OpenCode -> server (.in) ====");
log(readOrPlaceholder(`${recordLog}.in`));
log("\n==== MCP traffic: server -> OpenCode (.out) ====");
log(readOrPlaceholder(`${recordLog}.out`));
log("\n==== ironplcmcp stderr (.err) ====");
log(readOrPlaceholder(`${recordLog}.err`));

// Soft-skip (exit 0) only for the OpenCode provider-init failure; any other
// failure to invoke the tool is a real failure and blocks the build.
process.exit(openCodeInfraFailure ? 0 : 1);
