// Mock lane — deterministic hard gate.
//
// Drives `opencode run` against a local, fake OpenAI-compatible model
// (`mock-provider.mjs`) that always issues one `ironplc_check` tool call. This
// exercises OpenCode's real tool-call plumbing end to end — read the catalog,
// serialize the arguments, send the MCP `tools/call`, handle the result —
// against the real `ironplcmcp`, with no model latency and no flakiness.
//
// It asserts, via the recording wrapper (`record-mcp.mjs`):
//   1. OpenCode invoked `check` (proves it could read the catalog and call it),
//   2. with well-formed arguments (the argument-shape regression guard), and
//   3. the compiler responded with diagnostics (reliable here because the fast
//      mock turn completes before OpenCode tears the session down).
//
// The real-agent lane (`agent.mjs`) proves a genuine model can drive the tool;
// this lane proves the wiring deterministically.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  compilerResponded,
  ironplcmcpBin,
  makeWorkspace,
  readOrPlaceholder,
  recordingMcpConfig,
  runOpencode,
  testDir,
  toolWasInvoked,
  validateCheckArguments,
} from "./lib.mjs";
import { startMockProvider } from "./mock-provider.mjs";

const providerId = "mock";
const modelId = "mock-model";
const model = `${providerId}/${modelId}`;

const bin = ironplcmcpBin();
const wrapper = path.join(testDir, "record-mcp.mjs");
const tmpBase = process.env.RUNNER_TEMP || os.tmpdir();
const recordLog = path.join(
  fs.mkdtempSync(path.join(tmpBase, "opencode-mock-")),
  "mcp-record",
);

const mock = await startMockProvider();
console.log(`Mock provider: ${mock.url}  ironplcmcp: ${bin}`);

try {
  const workspace = makeWorkspace("opencode-mockws-", {
    $schema: "https://opencode.ai/config.json",
    provider: {
      [providerId]: {
        npm: "@ai-sdk/openai-compatible",
        name: providerId,
        options: { baseURL: mock.url, apiKey: "mock-local" },
        models: { [modelId]: { name: modelId, tools: true } },
      },
    },
    mcp: recordingMcpConfig({ bin, wrapper, recordLog }),
  });

  const result = runOpencode(
    [
      "run",
      "--model",
      model,
      "--log-level",
      "DEBUG",
      "--print-logs",
      "--dangerously-skip-permissions",
      // The mock ignores the prompt and always calls the tool; the text only
      // has to be a valid non-empty message.
      "Validate the program with ironplc_check.",
    ],
    { cwd: workspace, timeoutMs: 120000 },
  );

  const invoked = toolWasInvoked(recordLog);
  const args = invoked
    ? validateCheckArguments(recordLog)
    : { ok: false, reason: "tool not invoked" };
  const responded = compilerResponded(recordLog);
  console.log(
    `  tool invoked: ${invoked}` +
      `, arguments: ${args.ok ? "well-formed" : `INVALID (${args.reason})`}` +
      `, compiler responded: ${responded}`,
  );

  if (invoked && args.ok && responded) {
    console.log(
      "PASS: OpenCode read the tool catalog, invoked `check` with well-formed " +
        "arguments, and the IronPLC compiler returned diagnostics.",
    );
    process.exit(0);
  }

  console.error("FAIL: the deterministic mock lane did not complete the round-trip.");
  console.error("\n==== OpenCode stdout ====");
  console.error(result.stdout || "(empty)");
  console.error("\n==== OpenCode stderr ====");
  console.error(result.stderr || "(empty)");
  if (result.error) {
    console.error("\n==== OpenCode spawn error ====");
    console.error(`${result.error.message} (signal: ${result.signal ?? "none"})`);
  }
  console.error("\n==== MCP traffic: OpenCode -> server (.in) ====");
  console.error(readOrPlaceholder(`${recordLog}.in`));
  console.error("\n==== MCP traffic: server -> OpenCode (.out) ====");
  console.error(readOrPlaceholder(`${recordLog}.out`));
  console.error("\n==== ironplcmcp stderr (.err) ====");
  console.error(readOrPlaceholder(`${recordLog}.err`));
  process.exit(1);
} finally {
  await mock.close();
}
