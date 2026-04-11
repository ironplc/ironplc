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
2. **Verify** — agent calls `check` (full parse + semantic analysis) to get structured JSON diagnostics and self-heals
3. **Simulate** — agent calls `run` to verify logical correctness against expected outputs
4. **Finalize** — agent commits only when all checks pass

## Tool Vocabulary

The MCP tool names are aligned with the existing CLI vocabulary to avoid contributor confusion:

| Stage                               | CLI command | MCP tool   |
|-------------------------------------|-------------|------------|
| Tokenize / parse only (no semantic) | `tokenize`  | `parse`    |
| Parse + full semantic analysis      | `check`     | `check`    |
| Parse + semantic analysis + codegen | `compile`   | `compile`  |

Agents familiar with the CLI can use the MCP tools with matching expectations.

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

### `ironplc://source/{file}`

**REQ-RES-040** The `ironplc://source/{file}` resource returns the raw text of the named source file from the project's backing store.

**REQ-RES-041** The `{file}` template parameter matches the file names returned by `ironplc://project/manifest`.

**REQ-RES-042** The source resource returns a JSON object with the fields `file`, `content`, and `length_bytes`.

**REQ-RES-043** When the requested file is not in the manifest, the resource returns `found: false`, a `null` `content`, and a populated `diagnostics` array, rather than an MCP-level error.

This resource exists so an agent editing an existing project does not have to fall back to its own filesystem tools to read source text, which would bypass the MCP boundary and reintroduce path-resolution issues.

**Output:**
```json
{
  "found": true,
  "file": "motor.st",
  "content": "PROGRAM Main ... END_PROGRAM",
  "length_bytes": 324,
  "diagnostics": []
}
```

### `ironplc://project/io`

**REQ-RES-050** The `ironplc://project/io` resource returns every variable in the project that can be driven from outside the program: `VAR_INPUT` parameters of Programs, `VAR_EXTERNAL` references, global variables, and any variable mapped to a hardware input address (`%I*`).

**REQ-RES-051** The `ironplc://project/io` resource returns every variable that represents an output visible outside the program: `VAR_OUTPUT` parameters of Programs, global variables marked as outputs, and any variable mapped to a hardware output address (`%Q*`).

**REQ-RES-052** Each entry in the `inputs` and `outputs` arrays contains `name` (fully qualified; see Variable Naming in Architecture), `type`, and `address` (the direct-variable string such as `"%IX0.0"` when present, otherwise `null`).

This resource gives the agent a single call that answers "what can I drive?" and "what should I observe?" when planning a `run` or `verify` invocation. Without it, an agent has to synthesize this information by walking `symbols` output and guessing which variables are externally observable.

**Output:**
```json
{
  "inputs":  [{ "name": "Main.Start",    "type": "BOOL", "address": "%IX0.0" }],
  "outputs": [{ "name": "Main.MotorRun", "type": "BOOL", "address": "%QX0.0" }]
}
```

### `ironplc://pou/{name}/scope`

**REQ-RES-010** The `ironplc://pou/{name}/scope` resource returns all variables visible to the named POU, including local variables, input/output parameters, and global variables in scope.

**REQ-RES-011** The scope resource derives variable information from the symbol table built during semantic analysis.

**REQ-RES-012** The scope resource returns a JSON object with a `variables` array. Each entry contains: `name`, `type`, `direction` (one of `"Local"`, `"In"`, `"Out"`, `"InOut"`, `"Global"`), and `initial_value` (string representation, or `null` when no initial value is declared).

**Output:**
```json
{
  "pou": "Motor",
  "variables": [
    { "name": "Start",    "type": "BOOL", "direction": "In",    "initial_value": "FALSE" },
    { "name": "Counter",  "type": "DINT", "direction": "Local", "initial_value": "0" },
    { "name": "MotorRun", "type": "BOOL", "direction": "Out",   "initial_value": null }
  ]
}
```

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

### `parse`

Runs the parse stage only — no semantic analysis. Returns syntax diagnostics (malformed tokens, missing keywords, structural grammar errors).

Use this for rapid iteration on code structure. It is faster than `check` and useful when the agent is drafting code and wants to confirm it parses before investing in semantic correctness.

**Inputs:**
- `sources`: array of `{ name: string, content: string }`
- `options`: optional object with `dialect` and individual feature flags, same as `check`

**REQ-TOL-010** The `parse` tool runs the parse stage only and does not run semantic analysis.

**REQ-TOL-011** The `parse` tool returns a `diagnostics` array using the same format as `check`.

**REQ-TOL-012** The `parse` tool accepts the same `options` object as `check`, since dialect and feature flags affect the parser.

### `check`

Runs the full parse and semantic analysis pipeline — the same stages as the CLI `check` command — and returns diagnostics. This covers syntax errors, type errors, undeclared symbols, and all other semantic rules. It stops before code generation, so no bytecode is produced.

This is the highest-value tool. AI assistants use it to validate code they generate before presenting it to the user. The JSON format enables self-healing loops: the agent reads the diagnostics and fixes the code.

**Inputs:**
- `sources`: array of `{ name: string, content: string }` — inline source text (no file I/O required from the client)
- `options`: optional object with:
  - `dialect: string` — one of `"iec61131-3-ed2"` (default), `"iec61131-3-ed3"`, `"rusty"`. Selects a preset that enables the appropriate flags in one shot.
  - individual feature flags (e.g. `allow_c_style_comments: bool`) — override specific flags on top of the dialect preset. The full list of flags and their descriptions is returned by `list_options`.

**REQ-TOL-020** The `check` tool runs the parse stage and the full semantic analysis stage on the provided sources.

**REQ-TOL-021** The `check` tool does not run code generation.

**REQ-TOL-022** The `check` tool returns a `diagnostics` array; an empty array indicates no errors. The caller determines success by checking whether any diagnostic has `severity: "error"`.

**REQ-TOL-023** Each diagnostic in the `check` response includes: `code`, `message`, `file`, `start_line`, `start_col`, `end_line`, `end_col`, and `severity`.

**REQ-TOL-024** The `check` tool never returns an MCP-level error for a compiler failure; parse and semantic errors are returned as diagnostics.

**REQ-TOL-025** The `check` tool accepts an optional `dialect` string in `options`; when omitted, `"iec61131-3-ed2"` is used.

**REQ-TOL-026** The `check` tool accepts individual feature flag overrides in `options` that are applied on top of the dialect preset.

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

### `format`

Parses the provided source and re-renders it in canonical form using the existing `plc2plc` renderer. Returns the formatted sources, or diagnostics if the input cannot be parsed.

This keeps agent-authored code stylistically consistent with the rest of a project and removes "did the agent indent this correctly?" from the self-healing loop.

**Inputs:**
- `sources`: array of `{ name: string, content: string }`
- `options`: optional object with the same `dialect` and feature flags as `check`

**REQ-TOL-080** The `format` tool parses each source in the request and, on successful parse, returns the rendered canonical form in a `sources` array whose entries match the input names one-to-one.

**REQ-TOL-081** When any source fails to parse, the `format` tool returns the failing source's original content unchanged in the `sources` array, records the parser's diagnostics in `diagnostics`, and sets `formatted: false` for that entry.

**REQ-TOL-082** The `format` tool is idempotent: running `format` on its own output returns byte-identical content.

**REQ-TOL-083** The `format` tool produces the same canonical output that the `plc2plc` crate produces for a given AST and dialect.

**Output:**
```json
{
  "sources": [
    { "name": "main.st", "content": "PROGRAM Main\n  VAR\n    x : DINT;\n  END_VAR\nEND_PROGRAM\n", "formatted": true }
  ],
  "diagnostics": []
}
```

### `symbols`

Parses and analyzes source text, then returns the top-level symbol table: declared types, function blocks, functions, and programs with their variable declarations.

This lets an AI assistant understand the structure of a program before suggesting changes.

**Inputs:**
- `sources`: array of `{ name: string, content: string }`

**REQ-TOL-050** The `symbols` tool returns the top-level declarations for programs, functions, function blocks, and types found in the provided sources.

**REQ-TOL-051** Each program entry in the `symbols` response includes the program name and its variable declarations. Each variable entry contains `name`, `type`, `direction` (one of `"Local"`, `"In"`, `"Out"`, `"InOut"`, `"Global"`, `"External"`), `address` (the direct-variable string such as `"%IX0.0"` when the variable is mapped to a hardware address, otherwise `null`), and `external` (`true` when the variable can be driven from outside the program — i.e. `direction` is `"In"`, `"External"`, or `"Global"`, or `address` is a `%I*` hardware input).

**REQ-TOL-052** Each function entry in the `symbols` response includes the function name, return type, and parameter list.

**REQ-TOL-053** The `symbols` response includes a `diagnostics` array using the same format as `check`.

**Output:**
- `programs: [{ name, variables: [{ name, type, direction, address, external }] }]`
- `functions: [{ name, return_type, parameters: [...] }]`
- `function_blocks: [{ name, variables: [...] }]`
- `types: [{ name, kind }]`
- `diagnostics: [...]`

### `list_options`

Returns the set of compiler options the agent may pass in an `options` object to `parse`, `check`, or `compile`. This includes the list of dialect presets and the individual feature flags that can override them.

This tool lets an agent discover what flags exist without memorizing them and without risk of silent failure from a misspelled flag name.

**Inputs:** none.

**REQ-TOL-060** The `list_options` tool takes no inputs.

**REQ-TOL-061** The `list_options` tool returns a `dialects` array whose entries each contain `id`, `display_name`, and `description`.

**REQ-TOL-062** The `list_options` tool returns a `flags` array whose entries each contain `id`, `type` (`"bool"`, `"string"`, `"enum"`), `default`, `description`, and — for enum flags — an `allowed_values` array.

**REQ-TOL-063** The option `id` values returned by `list_options` are the exact keys accepted in the `options` object of `parse`, `check`, and `compile`.

**Output:**
```json
{
  "dialects": [
    { "id": "iec61131-3-ed2", "display_name": "IEC 61131-3 Ed. 2", "description": "..." },
    { "id": "iec61131-3-ed3", "display_name": "IEC 61131-3 Ed. 3", "description": "..." },
    { "id": "rusty",          "display_name": "RuSTy-compatible",   "description": "..." }
  ],
  "flags": [
    { "id": "allow_c_style_comments", "type": "bool", "default": false, "description": "..." }
  ]
}
```

### `explain_diagnostic`

Returns the human-readable explanation for a compiler problem code (e.g. `P0001`). This is the same text published under `docs/compiler/problems/P####.rst` and is already maintained as part of the project.

The self-healing loop depends on this: without it, an agent sees `P0042` in a diagnostic and has no way to understand why the compiler flagged the code, leading to guessed or destructive fixes.

**Inputs:**
- `code: string` — the problem code, case-insensitive (e.g. `"P0001"`).

**REQ-TOL-070** The `explain_diagnostic` tool accepts a `code` string and returns `code`, `title`, `description`, and optionally `suggested_fix`.

**REQ-TOL-071** The `explain_diagnostic` tool returns `found: false` and a populated `diagnostics` array when the code is unknown, rather than raising an MCP-level error.

**REQ-TOL-072** The text returned by `explain_diagnostic` is sourced from the same problem-code documentation that is published under `docs/compiler/problems/`.

**Output:**
```json
{
  "found": true,
  "code": "P0001",
  "title": "...",
  "description": "...",
  "suggested_fix": "...",
  "diagnostics": []
}
```

### `compile`

Runs the full pipeline (parse → semantic analysis → codegen) and returns the bytecode container as base64-encoded bytes, or diagnostics on failure. Also returns the task configuration extracted from the compiled program, which the agent can use to determine how many cycles to pass to `run`.

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

> **Note:** `compile` adds significant dependency weight (the codegen crate). Defer to a second milestone if needed; `check` covers the most important validation use case.

### `run`

Loads a compiled `.iplc` container into the IronPLC VM and executes it for a specified duration of simulated time. The agent derives a sensible `duration_ms` from the task configuration returned by `compile` — for example, one full period of the slowest cyclic task.

This enables the agent to verify logical correctness, not just syntactic validity. The agent can drive inputs over time via a `stimuli` schedule and observe the resulting output values in the returned trace.

**Inputs:**
- `container_base64: string` — the compiled `.iplc` bytes (from `compile`)
- `duration_ms: number` — simulated time to run in milliseconds; the VM derives the number of scan cycles from the task intervals in the container
- `variables: [string]` — list of fully-qualified variable names to include in the trace (see Variable Naming in Architecture)
- `stimuli: [Stimulus]` — optional time-ordered schedule of writes applied to externally-drivable variables; an empty or omitted schedule runs the program with declared initial values only

A `Stimulus` is an object:
```json
{ "time_ms": 100, "set": { "Main.Start": true, "Main.Speed": 75 } }
```

**REQ-TOL-040** The `run` tool executes the provided `.iplc` container in the IronPLC VM for the simulated duration specified by `duration_ms`, deriving the number of scan cycles from the task intervals declared in the container.

**REQ-TOL-041** The `run` tool returns a `trace` array with one entry per scan cycle, each containing the simulated timestamp in milliseconds (`time_ms`) and a map of the requested variable names to their values at the end of that cycle.

**REQ-TOL-042** The `run` tool only includes variables named in the `variables` input in each trace entry.

**REQ-TOL-043** The `run` tool returns an empty `trace` and a populated `diagnostics` array if the VM encounters a trap or execution error.

**REQ-TOL-044** The `run` tool accepts a time-ordered `stimuli` array. Each stimulus is applied at the start of the first scan cycle whose simulated end-time is greater than or equal to the stimulus `time_ms`. Values persist in their target variables until overwritten by a later stimulus or by the program itself.

**REQ-TOL-045** The `run` tool only permits stimuli to write variables reported by the `ironplc://project/io` resource as externally drivable (i.e. `VAR_INPUT` of Programs, `VAR_EXTERNAL`, globals, and `%I*`-mapped variables). Attempting to write a local, `VAR_OUTPUT`, or `%Q*`-mapped variable results in a diagnostic and aborts the run.

**REQ-TOL-046** The `run` tool rejects a stimulus whose value does not match the declared type of the target variable (for example, setting a `BOOL` to `42`), returning a diagnostic and aborting the run.

**REQ-TOL-047** The JSON encoding of values in `stimuli.set` and `trace[].variables` follows these conventions: `BOOL` ↔ JSON boolean; `SINT`/`INT`/`DINT`/`USINT`/`UINT`/`UDINT` ↔ JSON number; `LINT`/`ULINT` ↔ JSON string (to preserve 64-bit precision); `REAL`/`LREAL` ↔ JSON number; `STRING`/`WSTRING` ↔ JSON string; `TIME`/`DATE`/`DT`/`TOD` ↔ JSON string in IEC 61131-3 literal syntax (e.g. `"T#500ms"`). The encoding for arrays, structures, and other aggregates is deferred to a follow-up revision of this design.

**Output:**
```json
{
  "trace": [
    { "time_ms": 10, "variables": { "Main.MotorRun": false, "Main.Counter": 0 } },
    { "time_ms": 20, "variables": { "Main.MotorRun": true,  "Main.Counter": 1 } }
  ],
  "diagnostics": []
}
```

### `verify`

Compiles the provided sources, runs them in the VM against a caller-supplied stimulus schedule, evaluates caller-supplied expectations, and returns a single pass/fail result with the trace. This is the "one call the agent makes when the user says 'make it work'".

The primary reason this is a dedicated tool rather than a prompt is that expectation evaluation lives on the server side: the agent gets back `{ passed: true }` or a structured list of failures. Agents react reliably to structured pass/fail; reducing a raw trace into assertions in the agent's own reasoning is error-prone and burns tokens.

**Inputs:**
- `sources`: array of `{ name: string, content: string }`
- `options`: optional compiler options (same as `check`)
- `duration_ms: number`
- `stimuli: [Stimulus]` — same shape as `run`'s `stimuli`
- `expectations: [Expectation]`
- `trace_variables: [string]` — optional; additional fully-qualified names to include in the returned trace beyond those referenced by expectations

An `Expectation` is:
```json
{
  "variable": "Main.MotorRun",
  "at": "final",
  "equals": true
}
```

The `at` field is either the literal string `"final"` (evaluated at the last recorded cycle) or `{ "time_ms": N }` (evaluated at the first recorded cycle whose `time_ms >= N`). Exactly one comparator field must be present: `equals`, `not_equals`, `greater_than`, `greater_or_equal`, `less_than`, `less_or_equal`, or `approximately` with a required `tolerance` field for floating-point comparisons.

**REQ-TOL-090** The `verify` tool compiles the provided sources, runs them against the supplied stimuli and duration using the same VM semantics as `run`, evaluates the supplied expectations, and returns `passed`, `failures`, and `trace`.

**REQ-TOL-091** The `verify` tool sets `passed: false` and returns the full list of unsatisfied expectations in `failures` when any expectation is not met. Each failure entry includes the original expectation, the `actual` value observed, and the `time_ms` at which the comparison was evaluated.

**REQ-TOL-092** The `verify` tool returns `passed: false` with compile or run diagnostics in `diagnostics` when the sources fail to compile or the VM traps; in this case `failures` is empty and `trace` is either empty or partial up to the trap.

**REQ-TOL-093** The `verify` tool evaluates the `approximately` comparator as `abs(actual - expected) <= tolerance`; both `equals` and `not_equals` on floating-point types are rejected with a diagnostic instructing the caller to use `approximately`.

**REQ-TOL-094** The `verify` tool includes in its returned `trace` at least every variable referenced by an expectation, plus any variable named in `trace_variables`.

**Output:**
```json
{
  "passed": false,
  "failures": [
    {
      "expectation": { "variable": "Main.MotorRun", "at": "final", "equals": true },
      "actual": false,
      "time_ms": 1000
    }
  ],
  "trace": [
    { "time_ms": 10, "variables": { "Main.Start": false, "Main.MotorRun": false } }
  ],
  "diagnostics": []
}
```

## Prompts

Prompts are instructional templates the agent can invoke to perform structured multi-step workflows.

### `verify-logic`

**REQ-PRM-001** The `verify-logic` prompt instructs the agent to: (1) consult `ironplc://project/io` to identify the externally drivable inputs and the observable outputs of the program under test; (2) derive a stimulus schedule and a set of expectations from the user's natural-language specification, using fully-qualified variable names; (3) call the `verify` tool with the sources, `duration_ms`, `stimuli`, and `expectations`; and (4) report pass/fail to the user, including the returned `failures` and an excerpt of the `trace` on failure.

**REQ-PRM-002** The `verify-logic` prompt instructs the agent, on a failed verification, to inspect the `failures` and `trace` returned by `verify`, edit the sources, and retry the `verify` call. The prompt instructs the agent to give up after three consecutive failed iterations and surface the last failure to the user rather than continuing to edit.

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

### Variable Naming

**REQ-ARC-020** Variable names appearing in `run.variables`, `run.stimuli[].set`, `verify.trace_variables`, `verify.expectations[].variable`, and the `inputs`/`outputs` arrays of `ironplc://project/io` are fully qualified. The format is `<program_name>.<variable_name>` for program-scoped variables, `<program_name>.<fb_instance>.<variable_name>` for variables inside function-block instances, and the bare variable name (no prefix) for globals and resource-level variables.

**REQ-ARC-021** When a requested variable name is ambiguous or does not resolve against the loaded container, the server returns a diagnostic identifying the unresolved name and aborts the tool call.

Fully-qualified names are required even when only one program exists, so that agent-authored prompts and saved scenarios remain valid as a project grows.

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
