# MCP Server Design

## Overview

This document describes the design for an MCP (Model Context Protocol) server that exposes IronPLC compiler capabilities to AI assistants and other MCP clients. The server lives in the `compiler/mcp` crate (`ironplc-mcp`), which already exists as a placeholder.

The goal is to let AI assistants act as a "resident expert" for IEC 61131-3 Structured Text: writing, validating, simulating, and understanding PLC programs by calling compiler and VM operations as MCP tools and resources. The agent treats the MCP server as its professional engineering toolchain, not just a text generator.

## Background: What MCP Servers Typically Expose for Compilers

MCP servers that front compilers typically provide tools in these categories:

1. **Validation / diagnostics** — check source code for syntax and semantic errors, return structured diagnostics with locations and codes
2. **Formatting / pretty-printing** — normalize source code to a canonical form
3. **Symbol information** — list declared types, functions, programs, variables
4. **Compilation** — produce a binary artifact (bytecode, object file, etc.)
5. **Execution / evaluation** — run a program and return output or variable values
6. **Documentation / explanation** — look up what a problem code means, describe a language construct

IronPLC already has all the underlying capabilities. The MCP server maps them to tools and resources.

## Guiding Principle: The Agentic Verification Loop

The server is designed to support an autonomous agent workflow:

1. **Draft** — agent writes ST code using Resource context (symbol tables, type info, dependency graph)
2. **Verify** — agent calls `check` to get structured JSON diagnostics and self-heals
3. **Simulate** — agent calls `run` to verify logical correctness against expected outputs
4. **Finalize** — agent commits only when all checks pass

Resources provide "background knowledge" (scoped context, type tables, dependency graphs). Tools provide "hands" (actions with structured feedback). This separation keeps context lean: the agent only loads the scope relevant to the POU it is editing.

## Resources

Resources give the agent contextual knowledge without requiring it to read raw source files.

### `ironplc://project/manifest`

**REQ-RES-001** The `ironplc://project/manifest` resource returns the list of all source files in the workspace.

**REQ-RES-002** The `ironplc://project/manifest` resource returns the names of all Programs, Functions, and Function Blocks declared across all source files.

**REQ-RES-003** The `ironplc://project/manifest` resource returns each UDT as a top-level named list grouped by kind: `enumerations`, `structures`, `arrays`, `subranges`, `aliases`, `strings`, and `references`.

**Output:**
```json
{
  "files": ["main.st", "types.st"],
  "programs": ["Main"],
  "functions": ["Scale"],
  "function_blocks": ["PID"],
  "enumerations": ["MotorState", "Direction"],
  "structures": ["PidParams"],
  "arrays": [],
  "subranges": [],
  "aliases": [],
  "strings": [],
  "references": []
}
```

### `ironplc://pou/{name}/scope`

**REQ-RES-010** The `ironplc://pou/{name}/scope` resource returns all variables visible to the named POU, including local variables, input/output parameters, and global variables in scope.

**REQ-RES-011** The scope resource derives variable information from the symbol table built during semantic analysis.

**REQ-RES-012** The scope resource returns a Markdown table with columns: Name, Type, Direction (Local / In / Out / InOut / Global), and Initial Value.

### `ironplc://types/all`

**REQ-RES-020** The `ironplc://types/all` resource returns all user-defined types (UDTs, enumerations, type aliases) from the type table.

**REQ-RES-021** Each type entry includes at minimum: `name`, `kind`, and kind-specific detail fields (`values` for enumerations, `fields` for structs).

**Output:**
```json
{
  "types": [
    { "name": "MotorState", "kind": "enum", "values": ["Stopped", "Running", "Fault"] },
    { "name": "PidParams", "kind": "struct", "fields": [{ "name": "Kp", "type": "REAL" }] }
  ]
}
```

### `ironplc://pou/{name}/lineage`

**REQ-RES-030** The `ironplc://pou/{name}/lineage` resource returns the upstream and downstream dependencies of the named POU derived from the dependency DAG.

**REQ-RES-031** The lineage resource renders the dependency graph in DOT format (Graphviz).

DOT is preferred over Mermaid because: the resource is consumed by an AI agent rather than rendered in a browser, making Mermaid's rendering advantage irrelevant; DOT is the native output of graph libraries such as petgraph; and DOT handles POU names with special characters more reliably than Mermaid.

## Tools

### `check`

Runs the parse stage only — no semantic analysis. Returns syntax diagnostics (malformed tokens, missing keywords, structural grammar errors).

Use this for rapid iteration on code structure. It is faster than `analyze` and useful when the agent is drafting code and wants to confirm it parses before investing in semantic correctness.

**Inputs:**
- `sources`: array of `{ name: string, content: string }`
- `options`: optional object with `dialect` and individual feature flags, same as `analyze`

**REQ-TOL-010** The `check` tool runs the parse stage only and does not run semantic analysis.

**REQ-TOL-011** The `check` tool returns a `diagnostics` array using the same format as `analyze`.

**REQ-TOL-012** The `check` tool accepts the same `options` object as `analyze`, since dialect and feature flags affect the parser.

### `analyze`

Runs the full parse and semantic analysis pipeline — the same stages as the CLI `check` command — and returns diagnostics. This covers syntax errors, type errors, undeclared symbols, and all other semantic rules. It stops before code generation, so no bytecode is produced.

This is the highest-value tool. AI assistants use it to validate code they generate before presenting it to the user. The JSON format enables self-healing loops: the agent reads the diagnostics and fixes the code.

**Inputs:**
- `sources`: array of `{ name: string, content: string }` — inline source text (no file I/O required from the client)
- `options`: optional object with:
  - `dialect: string` — one of `"iec61131-3-ed2"` (default), `"iec61131-3-ed3"`, `"rusty"`. Selects a preset that enables the appropriate flags in one shot.
  - individual feature flags (e.g. `allow_c_style_comments: bool`) — override specific flags on top of the dialect preset. The full list of flags and their descriptions is returned by `list_options`.

**REQ-TOL-020** The `analyze` tool runs the parse stage and the full semantic analysis stage on the provided sources.

**REQ-TOL-021** The `analyze` tool does not run code generation.

**REQ-TOL-022** The `analyze` tool returns a `diagnostics` array; an empty array indicates no errors. The caller determines success by checking whether any diagnostic has `severity: "error"`.

**REQ-TOL-023** Each diagnostic in the `analyze` response includes: `code`, `message`, `file`, `start_line`, `start_col`, `end_line`, `end_col`, and `severity`.

**REQ-TOL-024** The `analyze` tool never returns an MCP-level error for a compiler failure; parse and semantic errors are returned as diagnostics.

**REQ-TOL-025** The `analyze` tool accepts an optional `dialect` string in `options`; when omitted, `"iec61131-3-ed2"` is used.

**REQ-TOL-026** The `analyze` tool accepts individual feature flag overrides in `options` that are applied on top of the dialect preset.

**Output:**
```json
{
  "diagnostics": [
    { "code": "P0001", "message": "...", "file": "main.st",
      "start_line": 5, "start_col": 3, "end_line": 5, "end_col": 10,
      "severity": "error" }
  ]
}
```

### `symbols`

Parses and analyzes source text, then returns the top-level symbol table: declared types, function blocks, functions, and programs with their variable declarations.

This lets an AI assistant understand the structure of a program before suggesting changes.

**Inputs:**
- `sources`: array of `{ name: string, content: string }`

**REQ-TOL-010** The `symbols` tool returns the top-level declarations for programs, functions, function blocks, and types found in the provided sources.

**REQ-TOL-011** Each program entry in the `symbols` response includes the program name and its variable declarations (name, type, direction).

**REQ-TOL-012** Each function entry in the `symbols` response includes the function name, return type, and parameter list.

**REQ-TOL-013** The `symbols` response includes a `diagnostics` array using the same format as `check`.

**Output:**
- `programs: [{ name, variables: [{ name, type, direction }] }]`
- `functions: [{ name, return_type, parameters: [...] }]`
- `function_blocks: [{ name, variables: [...] }]`
- `types: [{ name, kind }]`
- `diagnostics: [...]`

### `compile`

Runs the full pipeline (parse → analyze → codegen) and returns the bytecode container as base64-encoded bytes, or diagnostics on failure. Also returns the task configuration extracted from the compiled program, which the agent can use to determine how many cycles to pass to `run`.

**Inputs:**
- `sources`: array of `{ name: string, content: string }`
- `options`: optional compiler options

**REQ-TOL-030** The `compile` tool returns the `.iplc` container encoded as a base64 string in `container_base64` on success.

**REQ-TOL-031** The `compile` tool returns `container_base64: null` and a populated `diagnostics` array on failure. The caller determines success by checking whether `container_base64` is non-null.

**REQ-TOL-032** The `compile` tool returns a `tasks` array describing each task declared in the program, including `name`, `priority`, and `interval_ms` (the cyclic interval in milliseconds, or `null` for event-triggered tasks).

**REQ-TOL-033** The `compile` tool returns a `programs` array listing each program name and the task it is bound to.

**Output:**
```json
{
  "container_base64": "...",
  "tasks": [
    { "name": "Main", "priority": 1, "interval_ms": 10 },
    { "name": "Slow", "priority": 2, "interval_ms": 100 }
  ],
  "programs": [
    { "name": "Control", "task": "Main" }
  ],
  "diagnostics": []
}
```

> **Note:** `compile` adds significant dependency weight (the codegen crate). Defer to a second milestone if needed; `analyze` covers the most important validation use case.

### `run`

Loads a compiled `.iplc` container into the IronPLC VM and executes it for a specified duration of simulated time. The agent derives a sensible `duration_ms` from the task configuration returned by `compile` — for example, one full period of the slowest cyclic task.

This enables the agent to verify logical correctness, not just syntactic validity. The agent can assert that output variables match expectations after a given simulated duration.

**Inputs:**
- `container_base64: string` — the compiled `.iplc` bytes (from `compile`)
- `duration_ms: number` — simulated time to run in milliseconds; the VM derives the number of scan cycles from the task intervals in the container
- `variables: [string]` — list of variable names to include in the trace; names must match declarations visible in the relevant POU scope

**REQ-TOL-040** The `run` tool executes the provided `.iplc` container in the IronPLC VM for the simulated duration specified by `duration_ms`, deriving the number of scan cycles from the task intervals declared in the container.

**REQ-TOL-041** The `run` tool returns a `trace` array with one entry per scan cycle, each containing the simulated timestamp in milliseconds (`time_ms`) and a map of the requested variable names to their values at the end of that cycle.

**REQ-TOL-042** The `run` tool only includes variables named in the `variables` input in each trace entry.

**REQ-TOL-043** The `run` tool returns an empty `trace` and a populated `diagnostics` array if the VM encounters a trap or execution error.

**Output:**
```json
{
  "trace": [
    { "time_ms": 10, "variables": { "Motor_Run": false, "Counter": 0 } },
    { "time_ms": 20, "variables": { "Motor_Run": true,  "Counter": 1 } }
  ],
  "diagnostics": []
}
```

## Prompts

Prompts are instructional templates the agent can invoke to perform structured multi-step workflows.

### `verify-logic`

**REQ-PRM-001** The `verify-logic` prompt instructs the agent to compile the provided source using `compile`, run a simulation using `run` with specified inputs and cycle count, assert that named output variables match expected values, and report pass/fail with the full trace on failure.

## Architecture

### Transport

**REQ-ARC-001** The MCP server uses stdio transport (stdin/stdout JSON-RPC).

This matches how the VS Code extension and CLI are invoked and avoids requiring a network port.

### Crate Structure

The `ironplc-mcp` crate depends on:
- `ironplc-project` — already listed as a dependency; provides `FileBackedProject` and the `Project` trait
- `ironplc-codegen` — for the `compile` and `run` tools
- `ironplc-vm` — for the `run` tool
- `ironplc-problems` — for the `explain_diagnostic` tool
- An MCP SDK crate (see below)

### MCP SDK

The server uses [`rmcp`](https://crates.io/crates/rmcp) (the official Rust SDK from the MCP project), which provides the stdio transport, JSON-RPC dispatch, and tool registration macros. It is `async` and uses `tokio`.

### Source Handling

**REQ-ARC-010** All tools accept inline source text as `{ name: string, content: string }` objects rather than file paths.

**REQ-ARC-011** Each `{ name, content }` pair maps to a `FileId::from_string(name)` and is loaded via `change_text_document`.

This is intentional: MCP clients (AI assistants) work with text in memory, not files on disk, and it avoids path resolution issues across different environments.

### Diagnostic Mapping

The existing `Diagnostic` type (from `ironplc-dsl`) carries file ID, source span (byte offsets), problem code, and message. The MCP server converts byte offsets to line/column numbers using the source text before serializing to JSON. This is the same conversion the LSP server already performs.

### Error Handling

Tool handlers return structured JSON responses in all cases — they never panic or return MCP-level errors for compiler failures. A parse error is a valid `check` response with `ok: false` and populated diagnostics, not an MCP error.

MCP-level errors (invalid tool name, malformed arguments) are handled by the SDK.

### Context Scoping

Resources are designed to return only the context relevant to the POU being edited. The agent should:
- Use `ironplc://pou/{name}/scope` instead of `symbols` when editing a single POU
- Use `ironplc://pou/{name}/lineage` before any refactoring
- Use `ironplc://types/all` only when generating new type-dependent code

This keeps token usage lean and avoids sending the entire project's symbol table for every interaction.
