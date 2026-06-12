// A thin recording wrapper around the `ironplcmcp` binary.
//
// OpenCode launches this script as its MCP server. It transparently forwards
// the JSON-RPC stream between OpenCode (stdin/stdout) and the real `ironplcmcp`
// process, while teeing the traffic to log files so the test harness can assert
// — without coupling to OpenCode's output format — that the agent actually
// invoked a tool and that the IronPLC compiler responded.
//
// Configuration (environment variables):
//   IRONPLCMCP_BIN          path to the ironplcmcp binary (required)
//   IRONPLCMCP_RECORD_LOG   log prefix; writes "<prefix>.in" and "<prefix>.out"

import { spawn } from "node:child_process";
import fs from "node:fs";

const bin = process.env.IRONPLCMCP_BIN;
const logPrefix = process.env.IRONPLCMCP_RECORD_LOG;

if (!bin) {
  console.error("record-mcp: IRONPLCMCP_BIN is not set");
  process.exit(2);
}

const child = spawn(bin, [], { stdio: ["pipe", "pipe", "inherit"] });

const inLog = logPrefix ? fs.createWriteStream(`${logPrefix}.in`, { flags: "a" }) : null;
const outLog = logPrefix ? fs.createWriteStream(`${logPrefix}.out`, { flags: "a" }) : null;

// OpenCode -> server: record and forward.
process.stdin.on("data", (chunk) => {
  if (inLog) inLog.write(chunk);
  child.stdin.write(chunk);
});
process.stdin.on("end", () => child.stdin.end());

// server -> OpenCode: record and forward.
child.stdout.on("data", (chunk) => {
  if (outLog) outLog.write(chunk);
  process.stdout.write(chunk);
});

child.on("error", (err) => {
  console.error(`record-mcp: failed to spawn ${bin}: ${err.message}`);
  process.exit(2);
});
child.on("exit", (code, signal) => {
  process.exit(code ?? (signal ? 1 : 0));
});
