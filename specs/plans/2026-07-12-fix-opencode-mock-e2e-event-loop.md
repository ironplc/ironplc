# Fix OpenCode mock-e2e hang: run the mock provider out-of-process

## Goal

Make the deterministic `mock-e2e` lane of the OpenCode integration test pass. It
currently hangs for the full 120 s timeout and fails with `tool invoked: false`,
regardless of the Ollama model used (the mock lane does not use Ollama).

## Root cause

`test/mock-e2e.mjs` starts the fake OpenAI provider **in-process**
(`startMockProvider()` → `http.createServer`) and then drives OpenCode through
`runOpencode()`, which uses **`spawnSync`**. `spawnSync` blocks the Node event
loop for the entire lifetime of the OpenCode child, so the in-process HTTP
server can never run its request callback. OpenCode's TCP connection is accepted
into the kernel backlog but never answered; its provider fetch fails with
`AI_APICallError: Cannot connect to API` / `AI_RetryError: Failed after 3
attempts`, so the model never returns a tool call, no MCP `tools/call` is ever
sent, and the harness kills OpenCode at the timeout (`ETIMEDOUT`/`SIGTERM`).

This was verified locally: an in-process server plus a blocking
`spawnSync(curl)` sees zero requests and curl times out with "0 bytes received";
serving the exact same mock provider from a separate process makes OpenCode exit
0 with `tool invoked: true`.

The connectivity lane passes because it only talks to the MCP server (a child
OpenCode spawns itself) and needs no in-process HTTP server to be live during
`spawnSync`. The real-agent lane passes because it talks to an external Ollama
server, also out-of-process.

## Architecture

Give the mock provider its own process (and therefore its own event loop) so it
can serve requests while `runOpencode`'s `spawnSync` blocks the harness event
loop. `runOpencode` stays synchronous — the other lanes rely on that and are
unaffected.

Add `startMockProviderProcess()` to `test/mock-provider.mjs`: it spawns
`node mock-provider.mjs` (the module already serves until killed when run
directly), parses the printed base URL, and returns the same `{ url, close }`
shape as the in-process `startMockProvider()`. `mock-e2e.mjs` swaps to it with no
other changes.

## File map

- `integrations/opencode/test/mock-provider.mjs` — add `startMockProviderProcess()`;
  make the direct-run banner emit a stable, parseable URL line.
- `integrations/opencode/test/mock-e2e.mjs` — use `startMockProviderProcess()`
  instead of `startMockProvider()`; note why out-of-process is required.

## Tasks

- [ ] Add `startMockProviderProcess()` to `mock-provider.mjs`
- [ ] Switch `mock-e2e.mjs` to the out-of-process provider with a regression comment
- [ ] Run `npm run mock-e2e` locally and confirm it passes (`tool invoked: true`,
      well-formed arguments, compiler responded)
- [ ] Confirm `npm run smoke` still passes
