// Real-agent lane — fidelity signal.
//
// Drives `opencode run` with a local, key-free Ollama model and the IronPLC MCP
// server configured, proving a genuine model can drive the tool (the mock lane
// proves the wiring deterministically). Tool invocation is observed through a
// recording wrapper around `ironplcmcp` (see record-mcp.mjs), so the assertion
// does not depend on OpenCode's output format.
//
// The sanity check only asks two things: can OpenCode read the tool catalog
// (already covered by the connectivity smoke) and can it invoke a tool with
// well-formed arguments. So the pass gate here is: the model invoked `check`
// with well-formed `sources`/`options`. Whether the tool *response* is captured
// in the recording is reported but not required — capturing it races OpenCode's
// session teardown and the per-attempt timeout, and it is asserted reliably in
// the deterministic mock lane instead.
//
// The model is intentionally small (CPU-friendly) and therefore not perfectly
// reliable at choosing tools, so the prompt is highly directive and the run is
// retried a few times.
//
// Configuration (environment variables):
//   OPENCODE_E2E_MODEL     provider/model, default "ollama/qwen2.5:1.5b"
//   OLLAMA_BASE_URL        default "http://localhost:11434/v1"
//   OPENCODE_E2E_ATTEMPTS  retry budget, default 3
//   IRONPLCMCP_BIN         path to ironplcmcp (see lib.mjs for fallbacks)

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  compilerResponded,
  ironplcmcpBin,
  makeWorkspace,
  readOrPlaceholder,
  readRecentOpencodeLogs,
  recordingMcpConfig,
  resetRecordLogs,
  runOpencode,
  testDir,
  toolWasInvoked,
  validateCheckArguments,
} from "./lib.mjs";

const model = process.env.OPENCODE_E2E_MODEL || "ollama/qwen2.5:1.5b";
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
  mcp: recordingMcpConfig({ bin, wrapper, recordLog }),
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

console.log(`Model: ${model}  Ollama: ${ollamaBaseUrl}  ironplcmcp: ${bin}`);

// Pre-flight: assert OpenCode's resolved model catalog actually contains the
// configured `provider/model`. If it does not, the inline provider config in
// opencode.json failed to load — and the subsequent `opencode run` failure
// would otherwise surface as the same opaque "Unexpected server error" that
// any other server-side problem produces, leaving the cause ambiguous.
const modelsListing = runOpencode(["models"], { cwd: workspace, timeoutMs: 60000 });
const modelsOutput = `${modelsListing.stdout || ""}\n${modelsListing.stderr || ""}`;
const modelIsListed = modelsOutput
  .split("\n")
  .some((line) => line.trim() === model);
if (!modelIsListed) {
  console.error(
    `FAIL (pre-flight): OpenCode did not list "${model}" in \`opencode models\`. ` +
      "The inline provider config in opencode.json was not loaded — `opencode run` " +
      "would fail at model resolution.",
  );
  console.error("\n==== opencode models ====");
  console.error(modelsOutput.trim() || "(empty)");
  process.exit(1);
}
console.log(`Pre-flight: OpenCode resolves "${model}".`);

// Keep every attempt's output so the first informative failure is not lost
// when later attempts fail differently or hang.
const attemptResults = [];
for (let attempt = 1; attempt <= attempts; attempt++) {
  resetRecordLogs(recordLog);
  console.log(`Attempt ${attempt}/${attempts}: asking the agent to call ironplc_check...`);
  // `--print-logs` and `--log-level DEBUG` route OpenCode's own server logs to
  // stderr. Without them, a server-side failure surfaces only as an opaque
  // "Unexpected server error. Check server logs for details." message.
  //
  // The timeout must clear the one-time prompt-eval on the first call: a small
  // CPU model ingesting OpenCode's system prompt plus the tool schemas takes
  // several minutes before it emits anything, and a too-tight timeout kills the
  // run just after the tool call, before the round-trip settles. The prompt
  // cache warms after the first attempt, so later attempts are fast.
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
    { cwd: workspace, timeoutMs: 420000 },
  );
  attemptResults.push(result);

  const invoked = toolWasInvoked(recordLog);
  const args = invoked
    ? validateCheckArguments(recordLog)
    : { ok: false, reason: "tool not invoked" };
  // Reported for diagnostics, but not part of the gate: capturing the tool
  // *response* in the recording races OpenCode's teardown and the timeout. The
  // mock lane asserts it deterministically instead.
  const responded = compilerResponded(recordLog);
  console.log(
    `  tool invoked: ${invoked}` +
      `, arguments: ${args.ok ? "well-formed" : `INVALID (${args.reason})`}` +
      `, compiler responded: ${responded}`,
  );

  if (invoked && args.ok) {
    console.log(
      "PASS: the agent invoked ironplc_check with well-formed arguments" +
        (responded
          ? " and the IronPLC compiler returned diagnostics."
          : " (the compiler's response was not captured in the recording; the" +
            " deterministic mock lane asserts the full round-trip)."),
    );
    process.exit(0);
  }
}

console.error(
  "FAIL: the agent did not invoke the IronPLC check tool with well-formed " +
    "arguments within the attempt budget.",
);

// Dump every diagnostic channel so the failure can be understood from CI logs
// alone, without re-running locally.

// Every attempt's stdout/stderr. The first attempt's error is often the most
// informative — later attempts may fail differently, time out, or hang after
// the underlying problem has already been triggered.
attemptResults.forEach((result, index) => {
  const n = index + 1;
  console.error(`\n==== attempt ${n}/${attempts} OpenCode stdout ====`);
  console.error(result.stdout || "(empty)");
  console.error(`\n==== attempt ${n}/${attempts} OpenCode stderr ====`);
  console.error(result.stderr || "(empty)");
  if (result.error) {
    console.error(`\n==== attempt ${n}/${attempts} OpenCode spawn error ====`);
    console.error(`${result.error.message} (signal: ${result.signal ?? "none"})`);
  }
});

console.error("\n==== MCP traffic: OpenCode -> server (.in) ====");
console.error(readOrPlaceholder(`${recordLog}.in`));
console.error("\n==== MCP traffic: server -> OpenCode (.out) ====");
console.error(readOrPlaceholder(`${recordLog}.out`));
console.error("\n==== ironplcmcp stderr (.err) ====");
console.error(readOrPlaceholder(`${recordLog}.err`));

// OpenCode's view of the resolved config: what providers/models it actually
// loaded from opencode.json. Distinguishes "config not parsed" from
// "request to model failed."
const debugConfig = runOpencode(["debug", "config"], { cwd: workspace, timeoutMs: 60000 });
console.error("\n==== opencode debug config ====");
console.error((debugConfig.stdout || "").trim() || (debugConfig.stderr || "(empty)").trim());

const debugV2 = runOpencode(["debug", "v2"], { cwd: workspace, timeoutMs: 60000 });
console.error("\n==== opencode debug v2 (providers only) ====");
console.error(extractProvidersFromV2(debugV2.stdout || ""));

const serverLogs = readRecentOpencodeLogs();
console.error("\n==== OpenCode server logs ====");
console.error(serverLogs || "(no OpenCode log directory found)");

process.exit(1);

/// `opencode debug v2` dumps the full provider catalog (hundreds of entries).
/// We only care about the `providers` array — which is the part affected by
/// our inline config — so strip the rest to keep the failure dump readable.
function extractProvidersFromV2(text) {
  try {
    const parsed = JSON.parse(text);
    if (parsed && Array.isArray(parsed.providers)) {
      return JSON.stringify({ providers: parsed.providers }, null, 2);
    }
  } catch {
    /* fall through */
  }
  return text.trim() || "(empty)";
}
