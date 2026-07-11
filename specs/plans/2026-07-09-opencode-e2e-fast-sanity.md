# OpenCode E2E: fast, reliable sanity check

**Date:** 2026-07-09
**Status:** implementing

## Problem

The OpenCode integration E2E (`just opencode-e2e`) is slow (~12 min) and
flaky, and it fails even when it has actually proven what we care about.

Two failure runs illustrate it:

- `llama3.2:3b` run — attempts 1 and 2 died on Ollama `500`s after ~5 min each
  (huge ~9.8k-token prompt, CPU-only runner); attempt 3 finally invoked the tool
  but the request was aborted at session teardown, so the check *result* was
  never recorded and the run failed.
- `qwen2.5:1.5b` run — no `500`s, no aborts, and the model invoked `ironplc_check`
  in 2 of 3 attempts. The run still failed because the pass gate additionally
  required the tool *response* to be captured in the recording log
  (`compilerResponded()`), and that capture keeps losing a race to OpenCode's
  teardown / the harness's own 5-minute per-attempt timeout.

The sanity check only needs to answer two questions:

1. **Can OpenCode read the tool catalog?** (already proven by the connectivity
   smoke and the Rust schema guard.)
2. **Can OpenCode invoke a tool with well-formed arguments?** (the regression
   that motivated changing the tool arguments — a parameter shape OpenCode could
   not serialize.)

Model *quality* is explicitly out of scope.

## Approach

Keep two lanes, because a fake is not the same as a real model:

- **Mock lane (new, deterministic hard gate).** A tiny OpenAI-compatible
  endpoint (`test/mock-provider.mjs`) that always drives one `ironplc_check`
  tool call, then ends the turn. OpenCode does everything real: reads the
  catalog, serializes the arguments, sends the MCP `tools/call`, handles the
  result. Because there is no model latency, the full round-trip completes with
  no teardown race, so this lane reliably asserts invocation **and** that the
  compiler responded. Runs on every invocation; no Ollama.

- **Real-agent lane (retargeted).** Still drives a genuine local model
  (`qwen2.5:1.5b`) so we keep fidelity. The pass gate becomes: the model
  invoked `ironplc_check` **with well-formed arguments** (`sources` + `options`).
  The tool *response* capture becomes informational, because it depends on
  timing we do not control. The per-attempt timeout is raised so the one-time
  prompt-eval on the first call does not get killed mid-round-trip.

## Changes

- `test/lib.mjs` — shared helpers: recording-MCP config fragment, log reset,
  `toolWasInvoked`, `compilerResponded`, `invokedToolCall` (parsed),
  `validateCheckArguments`.
- `test/mock-provider.mjs` — new deterministic OpenAI-compatible model server
  (streaming + non-streaming).
- `test/mock-e2e.mjs` — new mock lane; asserts invoked + well-formed args +
  compiler responded.
- `test/agent.mjs` — retarget gate to invocation + argument shape; raise
  timeout; default model `qwen2.5:1.5b`; use shared helpers.
- `package.json`, `integrations/opencode/justfile` — add `mock-e2e`; wire into
  `e2e`.
- root `justfile` — run the mock lane (deterministic) before the Ollama lane;
  default model `qwen2.5:1.5b`.
- `.github/workflows/partial_opencode_e2e.yaml`, `deployment.yaml` — default
  model `qwen2.5:1.5b`.
- `README.md` — document the mock lane and the new defaults.

## Out of scope

- Shrinking the real-lane prompt by exposing fewer MCP tools (a further speedup;
  tracked separately).
- Re-enabling the deployment wiring (still commented out until this stabilizes).
