# OpenCode Integration End-to-End Test Plan

## Context

IronPLC ships an MCP server (`ironplcmcp`) so AI coding agents can drive the
compiler as a tool service. The existing how-to guide documents Claude Desktop,
Cline, and Claude Code. This plan adds first-class support and automated
validation for [OpenCode](https://opencode.ai), the open-source terminal agent,
and guards the integration against regression in CI.

### Bug discovered while building this

OpenCode (via the `@ai-sdk/openai-compatible` runtime and the MCP TypeScript
client) **rejects the entire `tools/list` response** if any tool's JSON Schema
uses a *boolean sub-schema* as the value of a `properties` entry (e.g.
`"options": true`). `schemars` emits a bare `true` for a `serde_json::Value`
field unless the field carries a description. Three tools — `pou_lineage`,
`pou_scope`, `symbols` — had no description on their `options` field, so
OpenCode dropped **all** IronPLC tools with the opaque message
`Failed to get tools`. The other ten tools already had a description (via a doc
comment), which makes `schemars` emit `{"description": ...}` (an object) and is
accepted.

Verified empirically: `opencode mcp list` reports `✗ ironplc failed` before the
fix and `✓ ironplc connected` after. The check tool is exposed to the agent as
`ironplc_check` once the list loads.

## Architecture

Three layers, cheapest first. The first two are deterministic and fast; the
third runs a real (local, key-free) LLM.

1. **Rust schema guard** (`compiler/mcp/tests/cli.rs`, runs in `cargo test`):
   parse `tools/list` and assert no tool uses a boolean schema as a `properties`
   value. Encodes the OpenCode requirement at the unit level; catches the bug
   without any external tooling.

2. **Connectivity smoke** (`integrations/opencode`): run the real `opencode mcp
   list` against the built `ironplcmcp` and assert the server reports
   `connected`. This performs the real MCP handshake and validates every tool
   schema through OpenCode's own loader. No model, no API key, no network
   (beyond the npm install of OpenCode).

3. **Agent end-to-end** (`integrations/opencode`): run `opencode run` driven by
   a **local Ollama model** (`llama3.2:3b`, no API key) with the IronPLC MCP
   server configured, and assert the agent actually invokes the `check` tool and
   the real compiler returns diagnostics. Tool invocation is detected by a thin
   Node *recording wrapper* around `ironplcmcp` that tees the JSON-RPC traffic to
   a log — robust against changes in OpenCode's output format. A directive
   prompt plus a small retry budget absorb the nondeterminism of a 3B model.

### Why a local model and not a hosted one

A hosted model needs an API key (a CI secret, in tension with ADR 0032's secret
scoping) and is nondeterministic. A real model does not exercise any more *of
IronPLC's* code than the local model — it only changes *who* decides to call the
tool. `ai-action/setup-ollama@v2` provides a key-free local model, so the agent
layer stays self-contained.

## File Map

| File | Action |
|------|--------|
| `compiler/mcp/src/tools/pou_lineage.rs` | Add `options` field description (fix) |
| `compiler/mcp/src/tools/pou_scope.rs` | Add `options` field description (fix) |
| `compiler/mcp/src/tools/symbols.rs` | Add `options` field description (fix) |
| `compiler/mcp/tests/cli.rs` | Add boolean-property-schema guard test |
| `integrations/opencode/package.json` | Pin `opencode-ai`; npm scripts |
| `integrations/opencode/justfile` | `setup`, `smoke`, `agent-e2e`, `e2e` recipes |
| `integrations/opencode/README.md` | How to run locally + what each layer checks |
| `integrations/opencode/test/lib.mjs` | Shared config generation + opencode runner |
| `integrations/opencode/test/connectivity.mjs` | Layer 2 |
| `integrations/opencode/test/agent.mjs` | Layer 3 |
| `integrations/opencode/test/record-mcp.mjs` | JSON-RPC recording wrapper |
| `justfile` | `opencode-e2e` recipe: install published compiler + run both layers |
| `.github/workflows/partial_opencode_e2e.yaml` | New reusable workflow |
| `.github/workflows/deployment.yaml` | Wire in the new job after publish-prerelease; gate publish-website |
| `docs/how-to-guides/ai-agents/write-plc-programs-with-an-ai-agent.rst` | Add an OpenCode tab |

## Decisions

- **Runs in the deployment pipeline against the published release**, mirroring
  the VS Code and install-script smoke tests, rather than rebuilding in per-PR
  CI (per maintainer review). The connectivity + agent layers therefore validate
  the shipped `ironplcmcp` binary. The per-PR regression guard for the schema bug
  is the Rust test in `compiler/mcp/tests/cli.rs`.
- **Blocking gate**: like the other deployment smoke tests, a failure blocks
  `publish-website` and therefore the release. The agent layer is mitigated by a
  directive prompt + retries + a larger Ollama context (`OLLAMA_CONTEXT_LENGTH`).
- **Test steps live in the `just opencode-e2e` recipe**, not inline in the
  workflow (per maintainer review).
- **`ai-action/setup-ollama` is pinned to a commit SHA** (per CodeQL).
- **Model: `llama3.2:3b`** — official tool-calling support, ~2GB, CPU-friendly.
  Exposed as a workflow input so it is trivially swappable.
- Tool-call detection uses the recording wrapper (stdio), not OpenCode's
  `--format json` events, to avoid coupling to OpenCode's output schema.

## Verification

- `cargo test -p ironplc-mcp` (schema guard) — local.
- `cd integrations/opencode && IRONPLCMCP_BIN=... npm run smoke` — local
  (verified: `✓ ironplc connected`).
- `just opencode-e2e` — full flow; the Ollama agent layer is CI-only (Ollama
  registry blocked in the dev sandbox). The harness was validated against the
  real OpenCode binary, which exposes the tools to the agent as `ironplc_*`.
- `cd compiler && just` full pipeline before PR.
