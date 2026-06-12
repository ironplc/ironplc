# OpenCode Integration Tests

These tests validate that IronPLC's MCP server (`ironplcmcp`) works with
[OpenCode](https://opencode.ai), the open-source terminal coding agent.

They run in CI via `.github/workflows/partial_opencode_e2e.yaml` and can be run
locally with the recipes below.

## Layers

The suite has three layers, cheapest and most deterministic first:

1. **Rust schema guard** — `compiler/mcp/tests/cli.rs`
   (`tools_list_when_parsed_then_no_tool_uses_boolean_property_schema`). Runs in
   `cargo test`. Asserts no tool advertises a boolean JSON schema as a
   `properties` value, which OpenCode rejects (it drops the *entire* tool list).
   No OpenCode required.

2. **Connectivity smoke** — `just smoke` (`test/connectivity.mjs`). Runs the real
   `opencode mcp list` against the built `ironplcmcp` and asserts the server
   reports `connected`. This exercises the full MCP handshake and OpenCode's own
   schema loader. No model, no API key.

3. **Agent end-to-end** — `just agent-e2e` (`test/agent.mjs`). Runs `opencode
   run` driven by a local, key-free Ollama model with the IronPLC MCP server
   configured, and asserts the agent invokes the `check` tool and the compiler
   returns diagnostics. Tool invocation is observed through a recording wrapper
   (`test/record-mcp.mjs`) so the assertion does not depend on OpenCode's output
   format.

## Running locally

```sh
# 1. Install the pinned OpenCode CLI.
just setup            # or: npm ci

# 2. Build the MCP server.
( cd ../../compiler && cargo build --release -p ironplc-mcp --bin ironplcmcp )

# 3. Connectivity smoke (deterministic).
just smoke

# 4. Agent end-to-end (requires Ollama).
ollama serve &                  # if not already running
ollama pull llama3.2:3b
just agent-e2e
```

## Configuration

The scripts read these environment variables (all optional):

| Variable | Default | Used by |
|----------|---------|---------|
| `IRONPLCMCP_BIN` | cargo target dirs, then PATH | both |
| `OPENCODE_E2E_MODEL` | `ollama/llama3.2:3b` | agent |
| `OLLAMA_BASE_URL` | `http://localhost:11434/v1` | agent |
| `OPENCODE_E2E_ATTEMPTS` | `3` | agent |
