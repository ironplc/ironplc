// Layer 2 — connectivity smoke test (deterministic, no model, no API key).
//
// Configures OpenCode with the IronPLC MCP server and runs `opencode mcp list`.
// This performs the real MCP handshake and validates every advertised tool
// schema through OpenCode's own loader. The server is reported as `connected`
// only when the full tool list loads — which is exactly the failure mode that
// a boolean property schema (e.g. `"options": true`) would trigger.

import { ironplcmcpBin, makeWorkspace, runOpencode, stripAnsi } from "./lib.mjs";

const bin = ironplcmcpBin();
console.log(`Using ironplcmcp: ${bin}`);

const workspace = makeWorkspace("opencode-conn-", {
  $schema: "https://opencode.ai/config.json",
  mcp: {
    ironplc: { type: "local", command: [bin], enabled: true },
  },
});

const result = runOpencode(["mcp", "list"], { cwd: workspace, timeoutMs: 90000 });
const output = stripAnsi(`${result.stdout || ""}\n${result.stderr || ""}`);
console.log("---- opencode mcp list ----");
console.log(output.trim());
console.log("---------------------------");

// Inspect the line describing the `ironplc` server. OpenCode prints
// "✓ ironplc  connected" on success and "✗ ironplc  failed" on failure.
const ironplcLine = output
  .split("\n")
  .find((line) => /\bironplc\b/.test(line) && /(connected|failed)/.test(line));

if (!ironplcLine || !ironplcLine.includes("connected")) {
  console.error(
    "FAIL: OpenCode could not load the IronPLC MCP server. This usually means a " +
      "tool schema is incompatible with OpenCode (for example a boolean JSON " +
      "schema used as a `properties` value).",
  );
  process.exit(1);
}

console.log("PASS: OpenCode connected to the IronPLC MCP server and loaded its tools.");
