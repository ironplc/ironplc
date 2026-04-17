# MCP Server Design

## Overview

This document describes the design for an MCP (Model Context Protocol) server that exposes IronPLC compiler capabilities to AI assistants and other MCP clients. The server lives in the `compiler/mcp` crate (`ironplc-mcp`), which already exists as a placeholder.

The goal is to let AI assistants act as a "resident expert" for IEC 61131-3 Structured Text: writing, validating, simulating, and understanding PLC programs by calling compiler and VM operations as MCP tools. The agent treats the MCP server as its professional engineering toolchain, not just a text generator.

## Background: What MCP Servers Typically Expose for Compilers

MCP servers that front compilers typically provide tools in these categories:

1. **Validation / diagnostics** — check source code for syntax and semantic errors, return structured diagnostics with locations and codes
2. **Formatting / pretty-printing** — normalize source code to a canonical form
3. **Symbol information** — list declared types, functions, programs, variables
4. **Compilation** — produce a binary artifact (bytecode, object file, etc.)
5. **Execution / evaluation** — run a program and return output or variable values
6. **Documentation / explanation** — look up what a problem code means, describe a language construct

IronPLC already has all the underlying capabilities. The MCP server maps them to tools.

## Guiding Principle: The Agentic Verification Loop

The server is designed to support an autonomous agent workflow:

1. **Draft** — agent assembles the relevant set of source files in its own context and queries the server for structural information (symbol table, I/O surface, dependency graph) as needed
2. **Verify** — agent calls `check` (full parse + semantic analysis) with the current sources and reads structured JSON diagnostics to self-heal
3. **Simulate** — agent calls `run` to execute the compiled program in the VM and walks the returned trace to confirm logical correctness against expected outputs
4. **Finalize** — agent persists the sources on its own side when all checks pass

The agent is the sole owner of project state. The server never writes to disk, never reads the filesystem from a tool handler, and never holds source text between calls; it is a pure compilation and simulation service driven by whatever sources the agent sends on each call.

## Tool Vocabulary

The MCP tool names are aligned with the existing CLI vocabulary to avoid contributor confusion:

| Stage                               | CLI command | MCP tool   |
|-------------------------------------|-------------|------------|
| Tokenize / parse only (no semantic) | `tokenize`  | `parse`    |
| Parse + full semantic analysis      | `check`     | `check`    |
| Parse + semantic analysis + codegen | `compile`   | `compile`  |

Agents familiar with the CLI can use the MCP tools with matching expectations.

## Design Principle: Stateless Tool Surface

Every tool in this design is a pure function of its explicit inputs. There is no session workspace, no cached analysis between calls, no `--project-dir` pre-load, and no disk I/O from any tool handler. The single exception is the process-level container cache described under Architecture — it stores compiled `.iplc` bytes keyed by an opaque handle so that `compile → run` can hand off without routing bytecode through the LLM context. The cache is a performance optimization, not a source of truth; its contents are never visible to any tool that accepts `sources`.

The agent is the sole owner of project state. It holds the files (in its own context, on its own filesystem, or both), decides which subset to send on any given call, and persists edits on its own side. The MCP server's job is to answer one question at a time about whichever sources the agent hands it, and to hand back structured results the agent can react to.

**REQ-STL-001** Every analysis, context, and execution tool accepts a required `sources` parameter: an array of `{ name: string, content: string }` objects. The tool operates on exactly the supplied sources for that single call. Subsequent calls that want the same inputs must re-send them.

**REQ-STL-002** Every analysis, context, and execution tool accepts a required `options` object that specifies the compiler dialect and any feature-flag overrides. The tool uses those options for exactly that single call. The server does not carry options across calls and does not apply implicit defaults; callers that want the standard IEC 61131-3 Edition 2 dialect must pass `{ "dialect": "iec61131-3-ed2" }` explicitly. The set of valid keys is the set returned by `list_options`; any other key is rejected with a diagnostic and the tool does not run.

**REQ-STL-003** The server holds no per-client state across tool calls other than the process-level container cache (see Container Cache under Architecture). Two successive calls from the same MCP client that supply identical `sources` and `options` produce identical responses up to non-determinism in wall-clock fields such as log timestamps.

**REQ-STL-004** File identity inside a single call is carried by the `name` field of each `sources` entry. Names must be valid UTF-8, non-empty, at most 256 bytes, and must not contain NUL, `/`, or `\`. Duplicate names within a single `sources` array are rejected with a diagnostic before any analysis runs. The server does not interpret names as filesystem paths and never touches the filesystem with them; they exist so that diagnostics can cite a file identifier the agent already recognizes in its own context.

**REQ-STL-005** Every tool response includes a top-level `ok: boolean` field. `ok` is `true` when the tool produced its primary result (for analysis tools, a diagnostics array with no `error`-severity entries; for `compile`, a non-null `container_id`; for `run`, a completed trace) and `false` when it did not. The `ok` field never replaces a tool's specific result fields; it exists as a single uniform success predicate so that agent code handling many tools can share one success check.

**REQ-STL-006** The server performs no disk I/O from any tool or resource handler. It does not accept filesystem paths as tool inputs, does not read files relative to any working directory, and does not write compilation or analysis artifacts to disk. The only files the server process ever opens are its own log output (see Logging and Observability) and, optionally, its own binary-embedded problem-code documentation.

## Tools

Tools are grouped into three categories:

1. **Analysis tools** — `parse`, `check`, `format`, `symbols`, `list_options`, `explain_diagnostic`. Given `sources` and `options`, these run some stage of the compiler and return structured information about the source.
2. **Context tools** — `project_manifest`, `project_io`, `pou_scope`, `pou_lineage`, `types_all`. These are lightweight lookups that answer a specific structural question about a source set without requiring the agent to parse the much larger output of `symbols` itself.
3. **Execution tools** — `compile`, `container_drop`, `run`. These produce or consume bytecode and drive the VM.

Every tool obeys REQ-STL-001..006: `sources` and `options` are required on every analysis, context, and execution tool; there is no implicit session; and every response carries a top-level `ok: boolean` field in addition to its tool-specific fields.

### `parse`

Runs the parse stage only — no semantic analysis. Returns syntax diagnostics (malformed tokens, missing keywords, structural grammar errors) plus a best-effort outline of what the parser could recognize.

Use this for rapid iteration on code structure. It is faster than `check` and useful when the agent is drafting code and wants to confirm it parses before investing in semantic correctness.

**Inputs:**
- `sources`: required array of `{ name: string, content: string }`
- `options`: required object; see REQ-STL-002

**REQ-TOL-010** The `parse` tool runs the parse stage only and does not run semantic analysis.

**REQ-TOL-011** The `parse` tool returns a `diagnostics` array using the same format as `check`.

**REQ-TOL-012** The `parse` tool accepts the same `options` object as `check`, since dialect and feature flags affect the parser.

**REQ-TOL-013** The `parse` tool returns a best-effort `structure` array alongside `diagnostics`, even when `diagnostics` contains errors. Each entry describes a top-level declaration the parser was able to recognize and contains `kind` (`"program"`, `"function"`, `"function_block"`, `"type"`, or `"configuration"`), `name` (string, or `null` when the parser could not recover a name), `file`, `start` (0-indexed byte offset of the declaration's name), and `end` (0-indexed byte offset one past the last byte of the name). This gives the agent an outline of its own in-progress draft to reason about even when the source is not yet valid — without it, a broken parse leaves the agent with only an opaque diagnostic and no structural context.

**Output:**
```json
{
  "ok": false,
  "structure": [
    { "kind": "program", "name": "Main", "file": "main.st", "start": 8, "end": 12 },
    { "kind": "function_block", "name": null, "file": "main.st", "start": 250, "end": 260 }
  ],
  "diagnostics": [
    { "code": "P0001", "message": "expected `;`", "file": "main.st",
      "start": 142, "end": 143, "severity": "error" }
  ]
}
```

### `check`

Runs the full parse and semantic analysis pipeline — the same stages as the CLI `check` command — and returns diagnostics. This covers syntax errors, type errors, undeclared symbols, and all other semantic rules. It stops before code generation, so no bytecode is produced.

This is the highest-value tool. AI assistants use it to validate code they generate before presenting it to the user. The JSON format enables self-healing loops: the agent reads the diagnostics, calls `explain_diagnostic` for any codes it does not recognize, fixes the code, and calls `check` again.

**Inputs:**
- `sources`: required array of `{ name: string, content: string }`
- `options`: required object with:
  - `dialect: string` — one of `"iec61131-3-ed2"`, `"iec61131-3-ed3"`, or any other dialect id returned by `list_options`. Selects a preset that enables the appropriate flags in one shot.
  - individual feature flags (e.g. `allow_c_style_comments: bool`) — override specific flags on top of the dialect preset. The full list of flags and their descriptions is returned by `list_options`.

**REQ-TOL-020** The `check` tool runs the parse stage and the full semantic analysis stage on the provided sources.

**REQ-TOL-021** The `check` tool does not run code generation.

**REQ-TOL-022** The `check` tool returns a `diagnostics` array and a top-level `ok: boolean`. `ok` is `true` when the diagnostics array contains no entries with `severity: "error"`; otherwise `ok` is `false`.

**REQ-TOL-023** Each diagnostic in the `check` response includes: `code`, `message`, `file`, `start`, `end`, and `severity`. `start` and `end` are 0-indexed byte offsets into the source text of the file identified by `file`. `end` points one past the last byte of the span, so an empty span has `start == end`. Byte offsets are the same offsets the compiler stores internally; no line/column conversion is applied.

**REQ-TOL-024** The `check` tool never returns an MCP-level error for a compiler failure; parse and semantic errors are returned as diagnostics.

**REQ-TOL-025** The `check` tool rejects an `options` object that is missing `dialect`, that contains a `dialect` value not returned by `list_options`, or that contains any key not returned by `list_options`. Rejection is surfaced as a diagnostic with `severity: "error"` and `ok: false`; the tool does not run.

**REQ-TOL-026** The `check` tool accepts individual feature flag overrides in `options` that are applied on top of the dialect preset.

**Output:**
```json
{
  "ok": false,
  "diagnostics": [
    { "code": "P0001", "message": "...", "file": "main.st",
      "start": 42, "end": 49, "severity": "error" }
  ]
}
```

### `format`

Parses the provided source and re-renders it in canonical form using the existing `plc2plc` renderer. Returns the formatted sources, or diagnostics if the input cannot be parsed.

This keeps agent-authored code stylistically consistent with the rest of a project and removes "did the agent indent this correctly?" from the self-healing loop.

**Inputs:**
- `sources`: required array of `{ name: string, content: string }`
- `options`: required object; same `dialect` and feature-flag schema as `check`

**REQ-TOL-080** The `format` tool parses each source in the request and, on successful parse, returns the rendered canonical form in a `sources` array whose entries match the input names one-to-one.

**REQ-TOL-081** When a source fails to parse, the `format` tool returns the failing source's original content unchanged in its `sources` entry, sets `formatted: false` for that entry, and includes that entry's parser diagnostics in a per-entry `diagnostics` array scoped to the failing file. The top-level `diagnostics` array aggregates every per-entry diagnostic so that callers that do not care which file failed can scan a single list. `ok` is `true` only when every entry formatted successfully.

**REQ-TOL-082** The `format` tool is idempotent: running `format` on its own output returns byte-identical content.

**REQ-TOL-083** The `format` tool produces the same canonical output that the `plc2plc` crate produces for a given AST and dialect.

**REQ-TOL-084** The `format` tool is pure: it does not retain any of the supplied sources, and its output is not cached. Callers that want to persist the formatted content must store it themselves.

**Output:**
```json
{
  "ok": true,
  "sources": [
    { "name": "main.st", "content": "PROGRAM Main\n  VAR\n    x : DINT;\n  END_VAR\nEND_PROGRAM\n", "formatted": true, "diagnostics": [] }
  ],
  "diagnostics": []
}
```

### `symbols`

Parses and analyzes source text, then returns the top-level symbol table: declared types, function blocks, functions, and programs with their variable declarations.

This is the full-project answer to "what is declared here?" For questions about a single POU or about the I/O surface, prefer one of the lighter-weight context tools (`pou_scope`, `project_io`, `types_all`) — they return less data and are cheaper on the agent's context window.

**Inputs:**
- `sources`: required array of `{ name: string, content: string }`
- `options`: required object; same schema as `check`
- `pou`: optional string — when present, the response is narrowed to just the named POU and the types its declarations reference

**REQ-TOL-050** The `symbols` tool returns the top-level declarations for programs, functions, function blocks, and types found in the sources under analysis.

**REQ-TOL-051** Each program entry in the `symbols` response includes the program name and its variable declarations. Each variable entry contains `name`, `type`, `direction` (one of `"Local"`, `"In"`, `"Out"`, `"InOut"`, `"Global"`, `"External"`), `address` (the direct-variable string such as `"%IX0.0"` when the variable is mapped to a hardware address, otherwise `null`), and `external` (`true` when the variable can be driven from outside the program — i.e. `direction` is `"In"`, `"InOut"`, `"External"`, or `"Global"`, or `address` is a `%I*` hardware input).

**REQ-TOL-052** Each function entry in the `symbols` response includes the function name, return type, and parameter list.

**REQ-TOL-053** The `symbols` response includes a `diagnostics` array using the same format as `check`, and a top-level `ok` following the same rule as `check` (true when no `error`-severity diagnostics).

**REQ-TOL-054** When the `pou` input is present, the `symbols` response includes only the matching POU (in exactly one of `programs`, `functions`, or `function_blocks`) and only the types actually referenced by that POU's declarations. When no POU with the given name exists, the response returns `ok: false`, `found: false`, and an empty `programs`/`functions`/`function_blocks`/`types` set along with a diagnostic, rather than raising an MCP-level error.

**REQ-TOL-055** The `symbols` tool bounds its response by the server-configured `max_symbols_response_bytes` limit (default 256 KiB). When an unfiltered call would exceed the limit, the tool returns `ok: false`, `truncated: true`, an empty `programs`/`functions`/`function_blocks`/`types` set, and a single diagnostic instructing the caller to pass a `pou` filter or to call one of the context tools instead. This prevents a single `symbols` call on a large project from silently consuming the agent's entire context window.

**Output:**
- `ok: bool`
- `programs: [{ name, variables: [{ name, type, direction, address, external }] }]`
- `functions: [{ name, return_type, parameters: [...] }]`
- `function_blocks: [{ name, variables: [...] }]`
- `types: [{ name, kind }]`
- `truncated: bool`
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

**REQ-TOL-070** The `explain_diagnostic` tool accepts a `code` string and returns `code`, `title`, `description`, and optionally `suggested_fix`. The returned text is plain-text rendering of the source reStructuredText.

**REQ-TOL-071** The `explain_diagnostic` tool returns `ok: false`, `found: false`, and a populated `diagnostics` array when the code is unknown, rather than raising an MCP-level error.

**REQ-TOL-072** The text returned by `explain_diagnostic` is embedded in the server binary at build time via `include_str!` from the same problem-code documentation published under `docs/compiler/problems/`. The tool handler performs no filesystem I/O.

**Output:**
```json
{
  "ok": true,
  "found": true,
  "code": "P0001",
  "title": "...",
  "description": "...",
  "suggested_fix": "...",
  "diagnostics": []
}
```

## Context Tools

Context tools are lightweight lookups that answer specific structural questions about a source set. They are what `symbols` would return if narrowed to a single concern, and they exist so the agent can request just the slice it needs instead of paying for the full symbol table every time. Every context tool takes the same required `sources` + `options` that the analysis tools take.

These tools replace the resource URIs (`ironplc://project/manifest`, `ironplc://pou/{name}/scope`, etc.) from earlier drafts of this design. MCP resources cannot take caller-supplied parameters at read time, which is incompatible with the stateless model: there is no session to read from, so the only way for a context lookup to know which source set it is describing is to accept the sources as a tool input.

### `project_manifest`

Returns a flat summary of what is declared across the supplied sources: every file name, every top-level POU name, and every user-defined type grouped by kind.

**Inputs:**
- `sources`: required array of `{ name: string, content: string }`
- `options`: required object; same schema as `check`

**REQ-TOL-200** The `project_manifest` tool returns the list of file names in the supplied `sources`, the names of all Programs, Functions, and Function Blocks declared across those files, and the UDTs grouped by kind (`enumerations`, `structures`, `arrays`, `subranges`, `aliases`, `strings`, `references`).

**REQ-TOL-201** The `project_manifest` tool runs parse and semantic analysis. When analysis fails, the tool returns `ok: false`, the partial manifest for whatever the parser could recognize, and the analysis diagnostics.

**Output:**
```json
{
  "ok": true,
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
  "references": [],
  "diagnostics": []
}
```

### `project_io`

Returns every variable the caller can drive (`inputs`) and every variable the caller can observe (`outputs`) across the supplied sources. This is the "what can I stimulate?" / "what should I observe?" query that callers of `run` use to plan a scenario.

**Inputs:**
- `sources`: required array of `{ name: string, content: string }`
- `options`: required object; same schema as `check`

**REQ-TOL-210** The `project_io` tool returns every variable that can be driven from outside the program: `VAR_INPUT` parameters of Programs, `VAR_IN_OUT` parameters of Programs, `VAR_EXTERNAL` references, global variables without a direct-variable address, and variables mapped to a hardware input address (`%I*`).

**REQ-TOL-211** The `project_io` tool returns every variable that represents an output visible outside the program: `VAR_OUTPUT` parameters of Programs, `VAR_IN_OUT` parameters of Programs, global variables without a direct-variable address, and variables mapped to a hardware output address (`%Q*`). A `VAR_IN_OUT` variable and a non-addressed global appear in both `inputs` and `outputs`; this reflects the IEC 61131-3 semantics that they can be both driven and observed. Variables mapped to marker memory (`%M*`) are neither inputs nor outputs and do not appear in either list.

**REQ-TOL-212** Each entry in the `inputs` and `outputs` arrays contains `name` (fully qualified; see Variable Naming in Architecture), `type`, and `address` (the direct-variable string such as `"%IX0.0"` when present, otherwise `null`).

**Output:**
```json
{
  "ok": true,
  "inputs":  [{ "name": "Main.Start",    "type": "BOOL", "address": "%IX0.0" }],
  "outputs": [{ "name": "Main.MotorRun", "type": "BOOL", "address": "%QX0.0" }],
  "diagnostics": []
}
```

### `pou_scope`

Returns all variables visible to the named POU, derived from the symbol table built during semantic analysis. Use this when editing one POU and wanting to know the names and types you are allowed to reference.

**Inputs:**
- `sources`: required array of `{ name: string, content: string }`
- `options`: required object; same schema as `check`
- `pou`: required string — the POU name to scope the query to

**REQ-TOL-220** The `pou_scope` tool returns a `variables` array for the named POU. Each entry contains: `name`, `type`, `direction` (one of `"Local"`, `"In"`, `"Out"`, `"InOut"`, `"Global"`, `"External"`), and `initial_value` (opaque string rendering, or `null` when no initial value is declared). The rendering is for display only and is not guaranteed to be a parseable expression.

**REQ-TOL-221** The `pou_scope` tool resolves `pou` against Programs, Functions, and Function Blocks in that order. When no POU with the given name exists, the tool returns `ok: false`, `found: false`, an empty `variables` array, and a diagnostic.

**Output:**
```json
{
  "ok": true,
  "found": true,
  "pou": "Motor",
  "variables": [
    { "name": "Start",    "type": "BOOL", "direction": "In",    "initial_value": "FALSE" },
    { "name": "Counter",  "type": "DINT", "direction": "Local", "initial_value": "0" },
    { "name": "MotorRun", "type": "BOOL", "direction": "Out",   "initial_value": null }
  ],
  "diagnostics": []
}
```

### `pou_lineage`

Returns the upstream and downstream dependencies of the named POU, derived from the project's dependency DAG.

**Inputs:**
- `sources`: required array of `{ name: string, content: string }`
- `options`: required object; same schema as `check`
- `pou`: required string — the POU name to query

**REQ-TOL-230** The `pou_lineage` tool returns a JSON object with three fields: `pou` (the requested POU name), `upstream` (an array of POU names that the requested POU depends on, directly or transitively), and `downstream` (an array of POU names that depend on the requested POU, directly or transitively). JSON adjacency-list is the only format this tool produces; callers that want a DOT rendering should convert client-side.

**REQ-TOL-231** When no POU with the given name exists, the tool returns `ok: false`, `found: false`, empty `upstream` and `downstream` arrays, and a diagnostic.

**Output:**
```json
{
  "ok": true,
  "found": true,
  "pou": "Motor",
  "upstream":   ["PID", "Scale"],
  "downstream": ["Main"],
  "diagnostics": []
}
```

### `types_all`

Returns every user-defined type (UDT, enumeration, type alias, array, subrange, string, reference) declared in the supplied sources, with enough detail to reason about field and variant names without re-parsing the source.

**Inputs:**
- `sources`: required array of `{ name: string, content: string }`
- `options`: required object; same schema as `check`

**REQ-TOL-240** The `types_all` tool returns a `types` array. Each entry contains `name`, `kind` (`"enum"`, `"struct"`, `"array"`, `"subrange"`, `"alias"`, `"string"`, `"reference"`), and kind-specific detail fields: `values` for enumerations, `fields` for structures, `element_type` + `bounds` for arrays, `base_type` + `low` + `high` for subranges, `target_type` for aliases, `length` for strings, `target_type` for references.

**Output:**
```json
{
  "ok": true,
  "types": [
    { "name": "MotorState", "kind": "enum", "values": ["Stopped", "Running", "Fault"] },
    { "name": "PidParams", "kind": "struct", "fields": [{ "name": "Kp", "type": "REAL" }] }
  ],
  "diagnostics": []
}
```

## Execution Tools

Execution tools produce or consume compiled bytecode and drive the VM. They depend on `ironplc-codegen` and `ironplc-vm` and ship in Milestone 2.

### `compile`

Runs the full pipeline (parse → semantic analysis → codegen) on the supplied sources and returns an opaque **container handle** that identifies the compiled `.iplc` bytes inside the server. Also returns the task configuration extracted from the compiled program, which the agent uses to choose a sensible `duration_ms` for `run`.

The container handle is the primary transport: agents pass it back to `run` without ever routing the bytecode through the LLM context. Base64-encoded bytes are available on request for clients that need to persist or transmit the artifact.

**Inputs:**
- `sources`: required array of `{ name: string, content: string }`
- `options`: required object; same schema as `check`
- `include_bytes`: optional boolean (default `false`) — when `true`, the response also includes `container_base64`

**REQ-TOL-030** The `compile` tool returns a `container_id` string that uniquely identifies the compiled `.iplc` container inside the server process. `container_id` values are opaque strings with no structure the caller should rely on.

**REQ-TOL-031** The `compile` tool returns `ok: false`, `container_id: null`, and a populated `diagnostics` array on failure.

**REQ-TOL-032** The `compile` tool returns a `tasks` array describing each task declared in the program. Each entry contains `name`, `priority`, `kind` (one of `"cyclic"`, `"single"`, `"event"`), and `interval_ms` (the cyclic interval in milliseconds when `kind == "cyclic"`, otherwise `null`).

**REQ-TOL-033** The `compile` tool returns a `programs` array listing each program name and the task it is bound to. When a program is not bound to any task, its `task` field is `null`.

**REQ-TOL-034** The `compile` tool returns the `.iplc` container encoded as a base64 string in `container_base64` only when the caller sets `include_bytes: true`; otherwise `container_base64` is `null`. This keeps the default response small and lets the agent pass `container_id` back to `run` without ever routing the bytecode through the LLM context.

**REQ-TOL-035** The server stores the compiled container bytes in a process-level container cache keyed by `container_id`. See Container Cache under Architecture for the cache's capacity and eviction policy.

**REQ-TOL-036** A container produced by `compile` is an immutable snapshot of the exact `sources` and `options` used at compile time. Subsequent calls to `compile` or `run` — including calls whose `sources` differ — do not affect any previously cached container. A `container_id` that has not been evicted from the cache can be passed to `run` for the lifetime of the server process.

**Output:**
```json
{
  "ok": true,
  "container_id": "c_9f3a1e",
  "container_base64": null,
  "tasks": [
    { "name": "Main", "priority": 1, "kind": "cyclic", "interval_ms": 10 },
    { "name": "Slow", "priority": 2, "kind": "cyclic", "interval_ms": 100 }
  ],
  "programs": [
    { "name": "Control", "task": "Main" }
  ],
  "diagnostics": []
}
```

### `container_drop`

Removes a previously compiled container from the process container cache. Agents normally do not need to call this — the cache is already bounded by its LRU capacity and by process lifetime — but it is provided for long-running servers that churn through many `compile` calls and want to reclaim memory eagerly.

**Inputs:**
- `container_id: string`

**REQ-TOL-150** The `container_drop` tool removes the container identified by `container_id` from the process container cache and returns `ok: true`, `removed: true`.

**REQ-TOL-151** The `container_drop` tool returns `ok: false`, `removed: false`, and a populated `diagnostics` array when the `container_id` is unknown (either never existed, or already evicted by LRU or by a prior `container_drop`), rather than raising an MCP-level error.

### `run`

Loads a compiled `.iplc` container into the IronPLC VM and executes it for a specified duration of simulated time, under server-enforced resource limits. The agent derives a sensible `duration_ms` from the task configuration returned by `compile` — for example, one full period of the slowest cyclic task.

This enables the agent to verify logical correctness, not just syntactic validity. The agent can drive inputs over time via a `stimuli` schedule and observe the resulting output values in the returned trace.

**Inputs:**
- `container_id: string` — the handle returned by `compile` (preferred)
- `container_base64: string` — inline `.iplc` bytes; exactly one of `container_id` and `container_base64` must be present
- `duration_ms: number` — simulated time to run in milliseconds
- `variables: [string]` — list of fully-qualified variable names to include in the trace (see Variable Naming in Architecture). May be empty when combined with `trace_outputs: true`.
- `trace_outputs: boolean` — optional (default `false`); when `true`, the server expands the trace set to include every externally observable variable in the container (same set as `project_io.outputs`), in addition to any explicit `variables`. The combined set is still subject to `max_variables_per_run`.
- `stimuli: [Stimulus]` — optional time-ordered schedule of writes applied to externally-drivable variables; an empty or omitted schedule runs the program with declared initial values only
- `trace: TraceOptions` — optional object controlling the trace sampling mode and size; see below
- `limits: LimitOverrides` — optional object that may tighten (but not loosen) the server-configured resource limits; see VM Sandboxing in Architecture
- `tasks: [string]` — optional filter; when present, only cycles from the named tasks appear in the trace

A `Stimulus` is an object:
```json
{ "time_ms": 100, "set": { "Main.Start": true, "Main.Speed": 75 } }
```

A `TraceOptions` object has these fields (all optional):
```json
{
  "mode": "every_cycle" | "every_ms" | "on_change" | "final_only",
  "interval_ms": 50,
  "max_samples": 500
}
```

**REQ-TOL-040** The `run` tool executes the referenced `.iplc` container in the IronPLC VM for the simulated duration specified by `duration_ms`, deriving the number of scan cycles from the task intervals declared in the container. Exactly one of `container_id` and `container_base64` must be present; when `container_id` is supplied, the server looks up the compiled bytes in the process container cache (see REQ-TOL-035). When a container declares no tasks at all, the `run` tool returns `ok: false` and a diagnostic instructing the caller to declare at least one task; it does not attempt to invent a cycle schedule.

**REQ-TOL-041** The `run` tool returns a `trace` array. Each entry contains `time_ms` (simulated milliseconds since start of run), `task` (the name of the task whose cycle end produced the entry), and `variables` (a map from the fully-qualified names in the effective trace set to their values at that instant). Entries are time-ordered by `time_ms`; ties are broken by task priority (lower priority number first), then by task name. Each requested variable name must be fully qualified and must resolve against the loaded container (see REQ-ARC-020 and REQ-ARC-021); unresolved or ambiguous names cause the run to return `ok: false` with a diagnostic before the VM starts. Wildcard names (for example `"*"`, `"Main.*"`) are rejected with a diagnostic — agents that want to trace many variables set `trace_outputs: true` or enumerate the names explicitly. The effective trace set (the union of `variables` and the expansion of `trace_outputs`) is capped at the server-configured `max_variables_per_run` limit (see VM Sandboxing); a request that exceeds the cap is rejected with a diagnostic that reports the limit and the request size.

**REQ-TOL-042** The `run` tool accepts a `stimuli` array that must be sorted by strictly non-decreasing `time_ms`; an out-of-order array is rejected with a diagnostic before the VM starts. Each stimulus is applied at the start of the first scan cycle whose simulated **start** time is greater than or equal to the stimulus `time_ms`. Values persist in their target variables until overwritten by a later stimulus or by the program itself. The `run` tool only permits stimuli to write variables that `project_io` would classify as inputs for the same container (`VAR_INPUT` or `VAR_IN_OUT` of Programs, `VAR_EXTERNAL`, non-addressed globals, and `%I*`-mapped variables); attempts to write a local, a pure `VAR_OUTPUT`, a `%Q*`-mapped variable, or a `%M*`-mapped marker result in a diagnostic and the run is not started. A stimulus whose value does not match the declared type of the target variable (for example, setting a `BOOL` to `42`) is likewise rejected.

**REQ-TOL-043** The JSON encoding of values in `stimuli.set`, `trace[].variables`, and `summary.final_values` is recursive and is defined for every IEC 61131-3 type the compiler accepts:
  - `BOOL` ↔ JSON boolean.
  - `SINT`/`INT`/`DINT`/`USINT`/`UINT`/`UDINT` ↔ JSON number.
  - `LINT`/`ULINT` ↔ JSON string in decimal (to preserve 64-bit precision). Stimuli may also supply a JSON number for these types when the value fits in a signed 53-bit integer; larger magnitudes must use the string form.
  - `REAL`/`LREAL` ↔ JSON number. The special IEEE-754 values are encoded as the JSON strings `"NaN"`, `"Infinity"`, and `"-Infinity"`.
  - `STRING`/`WSTRING` ↔ JSON string.
  - `TIME`/`DATE`/`DT`/`TOD` ↔ JSON string in IEC 61131-3 literal syntax (e.g. `"T#500ms"`, `"D#2025-01-01"`).
  - Enumeration values ↔ JSON string of the form `"<EnumTypeName>.<ValueName>"` (e.g. `"MotorState.Running"`). The bare value name (`"Running"`) is also accepted on input when it is unambiguous across the container's type table; ambiguous bare names are rejected with a diagnostic.
  - `ARRAY[L..U] OF T` ↔ JSON array of length `U - L + 1`, lowest index first, each element encoded per this same rule. On `stimuli.set`, the entire array must be supplied; partial updates by index are out of scope for this milestone (see Future Work).
  - `STRUCT` / function-block instances ↔ JSON object whose keys are field names and whose values are encoded per this same rule. On `stimuli.set`, every field must be supplied; partial updates by field are out of scope for this milestone (see Future Work).
  - Nested aggregates apply the rules recursively.
  Any value that does not match its declared type is rejected with a diagnostic.

**REQ-TOL-044** The `run` tool accepts an optional `trace.mode` of `"every_cycle"` (default), `"every_ms"`, `"on_change"`, or `"final_only"`:
  - `"every_cycle"` emits one sample per task cycle end, as described in REQ-TOL-041.
  - `"every_ms"` requires `trace.interval_ms` and emits at most one sample per interval. A sample is emitted at the first task cycle end whose `time_ms` is greater than or equal to the next interval tick; samples carry the actual `time_ms` (they are not interpolated).
  - `"on_change"` emits a sample only when at least one variable in the effective trace set has a different value than the most recently emitted sample for that set. The first cycle always emits.
  - `"final_only"` emits exactly one sample at the end of the run, containing the final values of every variable in the effective trace set. `task` on that sample is the literal string `"final"`.

**REQ-TOL-045** The `run` tool accepts an optional `tasks` filter. When present, only cycles from the named tasks appear in the trace; cycles from other tasks still execute in the VM and still influence `summary.completed_cycles`, but are not emitted as samples.

**REQ-TOL-046** The `run` tool caps the returned trace at `min(trace.max_samples, server_max_samples)` entries, where `server_max_samples` is a server-configured limit described under VM Sandboxing. When the cap is hit, the last entry in `trace` is the most recent sample that fit; `truncated: true` is set in the response; and `terminated_reason` is set to `"sample_cap"`. The run still executes to completion (up to other limits) — only the emitted trace is truncated.

**REQ-TOL-047** The `run` tool enforces the resource limits described under VM Sandboxing (maximum simulated `duration_ms`, VM fuel, wall-clock time). When a limit is exceeded, or when the VM encounters a trap, the run terminates early. In all early-termination cases the response carries the partial trace up to the last cycle that completed before termination, a populated `diagnostics` entry identifying the cause, and `terminated_reason` set to one of `"duration"`, `"fuel"`, `"wall_clock"`, `"sample_cap"`, `"error"`, or — for a run that finished cleanly — `"completed"`. An agent-supplied `limits` override may only tighten the server-configured bounds, never loosen them. `ok` is `true` only when `terminated_reason == "completed"`.

**REQ-TOL-048** The `run` tool's response always includes a `summary` object with at least: `final_values` (a map of every variable in the effective trace set to its value at the last simulated instant, regardless of `trace.mode`), `completed_cycles` (a map from task name to the number of cycles that completed for that task), and `terminated_reason`. `summary` is populated even when the trace is empty because of `"final_only"` mode or because `"on_change"` never fired.

**Output:**
```json
{
  "ok": true,
  "trace": [
    { "time_ms": 10, "task": "Main", "variables": { "Main.MotorRun": false, "Main.Counter": 0 } },
    { "time_ms": 20, "task": "Main", "variables": { "Main.MotorRun": true,  "Main.Counter": 1 } }
  ],
  "truncated": false,
  "terminated_reason": "completed",
  "summary": {
    "final_values": { "Main.MotorRun": true, "Main.Counter": 50 },
    "completed_cycles": { "Main": 100, "Slow": 10 }
  },
  "diagnostics": []
}
```

## Architecture

### Transport

**REQ-ARC-001** The MCP server uses stdio transport (stdin/stdout JSON-RPC).

This matches how the VS Code extension and CLI are invoked and avoids requiring a network port.

### Crate Structure

The `ironplc-mcp` crate depends on:
- `ironplc-project` — provides the `Project` trait and an in-memory implementation used to hold a single tool call's sources while the compiler runs
- `ironplc-plc2plc` — for the `format` tool
- `ironplc-codegen` — for the `compile` tool
- `ironplc-vm` — for the `run` tool
- `ironplc-problems` — for the `explain_diagnostic` tool
- An MCP SDK crate (see below)

`ironplc-mcp` does **not** depend on `FileBackedProject` or any other filesystem-backed project implementation. REQ-STL-006 forbids filesystem I/O from tool handlers, and the only `Project` instance the server ever constructs is a short-lived in-memory one that holds the `sources` array of the tool call currently in flight.

### MCP SDK

The server uses [`rmcp`](https://crates.io/crates/rmcp) (the official Rust SDK from the MCP project), which provides the stdio transport, JSON-RPC dispatch, and tool registration macros. It is `async` and uses `tokio`.

### Source Handling

**REQ-ARC-010** Source text enters the server exactly once per tool call, as the `sources: [{ name, content }]` array on that call. The server constructs a fresh in-memory `Project` instance, loads every `(name, content)` pair into it, runs the tool's handler, and discards the `Project` when the handler returns. Tool calls never share `Project` instances and never persist them.

**REQ-ARC-011** Each `{ name, content }` pair is mapped to a `FileId::from_string(name)` and loaded via `change_text_document` against the per-call `Project` instance. File names are validated against REQ-STL-004 before any compiler code runs.

**REQ-ARC-012** The server does not accept raw filesystem paths as arguments in any tool, does not read files from the on-disk project directory, and does not write compilation or analysis artifacts to disk. This is the same requirement as REQ-STL-006; it is repeated here so that an implementer browsing the Architecture section cannot miss it.

### Container Cache

`compile` returns an opaque `container_id` that later `run` calls can reference without routing the bytecode through the LLM context. The container cache is the only piece of cross-call server state in this design, and it is explicitly a performance optimization: losing a cached container is never a correctness problem, only a "the agent must call `compile` again" problem.

**REQ-ARC-070** The server maintains a single process-wide container cache keyed by `container_id`. The cache stores the raw `.iplc` bytes, the task and program metadata returned by `compile`, and the symbol table needed by `run` to resolve fully-qualified variable names. It does not store the original `sources` or `options`.

**REQ-ARC-071** The cache has a bounded capacity, measured in both entry count (default 64 containers) and total bytes (default 64 MiB). Both bounds are configurable at server startup via `--max-cached-containers` and `--max-cached-container-bytes`. When inserting a new container would exceed either bound, the cache evicts entries in least-recently-used order until the new entry fits; a container that is larger than the entire byte budget on its own causes `compile` to return `ok: false` with a diagnostic rather than pinning an oversized entry.

**REQ-ARC-072** Cached containers never expire on a timer; they are only evicted by LRU pressure, by `container_drop`, or by process exit. Entries are immutable from the moment `compile` inserts them — no tool mutates a cached container in place — which makes REQ-TOL-036's snapshot guarantee trivially implementable.

**REQ-ARC-073** A `run` call whose `container_id` is not in the cache returns `ok: false` with a diagnostic that names the unknown `container_id`. The agent's recovery is to re-compile. The diagnostic must distinguish "never existed" from "evicted by LRU" only if doing so is cheap; a single shared error message is acceptable.

### Variable Naming

**REQ-ARC-020** Variable names appearing in `run.variables`, `run.stimuli[].set`, and the `inputs`/`outputs` arrays of the `project_io` tool are fully qualified. The format is:

- `<program>.<variable>` for program-local variables.
- `<program>.<fb_instance>.<variable>` for variables inside a function-block instance declared directly in a program. Nested function-block instances extend this rule recursively: `<program>.<outer_fb>.<inner_fb>.<variable>`, with one dot separating every level of nesting.
- `<resource>.<variable>` for resource-scoped globals declared inside a `RESOURCE` block.
- `<configuration>.<resource>.<program>.<variable>` (and longer forms) when disambiguation across multiple resources or configurations is required.
- The bare variable name (no prefix) for top-level `VAR_GLOBAL` variables declared directly in a `CONFIGURATION`.

The server resolves a request by scanning from the most specific prefix to the least specific; a bare name first matches top-level globals, then resource-scoped globals if the container has exactly one resource, and so on. An ambiguous match is always rejected — the resolution rule exists to reduce bookkeeping in the common single-configuration, single-resource case, not to paper over ambiguity.

**REQ-ARC-021** When a requested variable name is ambiguous or does not resolve against the loaded container, the server returns `ok: false` with a diagnostic identifying the unresolved name. The tool call does not run and the VM does not start.

Fully-qualified names are required even when only one program exists, so that agent-authored prompts and saved scenarios remain valid as a project grows.

### VM Sandboxing and Resource Limits

`run` executes agent-supplied code in the IronPLC VM. Without explicit bounds, a pathological program (infinite loop, runaway counter, arbitrarily long simulation) can pin a CPU and blow out the agent's context with an unbounded trace. The server therefore enforces a set of resource limits on every VM invocation.

The limits are configured at server startup and exposed as a `LimitOverrides` object that `run` callers may use to **tighten** the bounds for a single call. Callers cannot loosen them: a per-call value that exceeds the server-configured default is rejected with a diagnostic.

```json
{
  "max_duration_ms": 60000,
  "max_fuel": 50000000,
  "max_wall_clock_ms": 5000,
  "max_samples": 1000,
  "max_variables_per_run": 64
}
```

**REQ-ARC-030** The server imposes a `max_duration_ms` (simulated time), a `max_fuel` (VM instruction budget), a `max_wall_clock_ms` (real-world execution time), a `max_samples` (trace entry cap), and a `max_variables_per_run` (maximum length of the effective trace set in `run`) on every VM invocation. The defaults are configurable at server startup via command-line arguments (e.g. `--max-duration-ms`, `--max-fuel`, `--max-wall-clock-ms`, `--max-samples`, `--max-variables-per-run`) and have sane defaults for an interactive agent session: 60000 ms simulated, 50000000 fuel, 5000 ms wall-clock, 1000 samples, 64 variables per run.

**REQ-ARC-031** The server rejects any `limits` override in `run.limits` whose field exceeds the server-configured default for that field, returning a diagnostic that names the offending field and does not start the VM.

**REQ-ARC-032** When a VM invocation would exceed a limit, the VM terminates cleanly at the end of the most recent completed task cycle. The `run` response includes a diagnostic identifying the exceeded limit and sets `terminated_reason` to `"duration"`, `"fuel"`, `"wall_clock"`, or `"sample_cap"` as appropriate.

**REQ-ARC-033** The `max_fuel` budget is shared across all tasks for a single VM invocation; fuel consumed by any task counts against the same budget. Stimulus application is billed against fuel.

**REQ-ARC-034** When a VM invocation completes without exceeding any limit, `terminated_reason` is `"completed"`. When the VM traps (type error, division by zero, array bounds violation, etc.) it is `"error"`.

**REQ-ARC-035** The server is not required to enforce wall-clock limits with hard real-time precision. The implementation is permitted to check the wall-clock between task cycle ends, so the actual termination time may exceed `max_wall_clock_ms` by up to one task cycle's worth of VM work.

### Logging and Observability

The MCP server is the first place we get to watch a real AI agent drive the IronPLC toolchain. Understanding how agents actually use it — which tools they reach for, in what sequence, how often `check` diagnostics lead to a successful self-heal on the next call, how often `run` terminates on a resource limit rather than completing — is essential for refining the tool surface, the problem-code docs, and the tool descriptions. This observability must be designed in, not grafted on after the fact.

Because the server holds no session state, the "unit of observation" is a single connection: every MCP client connection gets a fresh `connection_id` and every tool call within that connection carries the connection id plus a monotonic per-connection sequence number. An analyst reconstructing agent behavior reads a contiguous log stream for a given `connection_id`.

**REQ-ARC-040** The server emits a structured log entry for every tool call. Each entry contains at minimum: `connection_id` (a UUID assigned when the MCP client connects), `seq` (a monotonic per-connection counter), `timestamp` (ISO 8601 UTC), `name` (the tool name), `duration_ms` (wall-clock execution time of the handler), `ok` (the top-level `ok` field from the response, see REQ-STL-005), and — when `ok` is `false` — `error_kind` drawn from a stable taxonomy (`"invalid_arguments"`, `"unknown_container"`, `"limit_exceeded"`, `"parse_failed"`, `"analysis_failed"`, `"vm_trap"`, `"internal"`).

**REQ-ARC-041** Each log entry additionally includes a tool-specific summary so that an analyst can reconstruct agent behavior without the payload itself:
  - Analysis tools (`parse`, `check`, `format`, `symbols`) and context tools (`project_manifest`, `project_io`, `pou_scope`, `pou_lineage`, `types_all`): `source_count`, `source_total_bytes`, `dialect` (the dialect id that was used), `diagnostic_count`, `error_count`, `warning_count`, and — for `check` and `parse` — a sorted deduplicated `problem_codes` array.
  - `compile`: `container_id`, `container_size_bytes`, `task_count`, `program_count`, `include_bytes`, plus the analysis-tool fields above.
  - `container_drop`: `container_id`, `removed`.
  - `run`: `container_id`, `duration_ms_requested`, `duration_ms_simulated` (how far the VM actually got), `fuel_consumed`, `trace_mode`, `trace_variable_count`, `trace_samples_emitted`, `truncated`, `terminated_reason`, `stimulus_count`.
  - `list_options` and `explain_diagnostic`: `response_size_bytes`.
  - Every entry also carries `response_size_bytes` so that an analyst can spot an agent pulling large responses repeatedly.

**REQ-ARC-042** The server does **not** log source text, stimulus values, expectation values, trace variable values, or explanation bodies by default. These fields are replaced in the log with fixed-width content hashes (first 12 hex characters of a SHA-256 over the UTF-8 bytes of the canonical JSON encoding) so that an analyst can detect "the agent sent the same source twice" or "the source changed between `check` calls" without the payload ever leaving the host. A `--log-level=debug` startup flag opts into logging full payloads for local debugging; this mode must print a warning to stderr at connection start that payload logging is enabled.

**REQ-ARC-043** Logs are written to stderr by default, because the stdio transport uses stdout for the MCP JSON-RPC stream and any log output on stdout would corrupt the protocol. The `--log-file <path>` startup flag redirects logs to a file. The `--log-format` startup flag accepts `"json"` (one JSON object per line, the default) or `"text"` (human-readable, not intended for machine analysis).

**REQ-ARC-044** At connection start the server emits a `connection_start` event containing `connection_id`, the server version, the effective resource limits (after applying command-line overrides), and the effective container-cache bounds. At connection end the server emits a `connection_end` event containing `connection_id`, the total connection wall-clock, the total number of tool calls, per-tool call counts, and the reason for termination (`"client_disconnect"`, `"signal"`, `"internal_error"`).

**REQ-ARC-045** The log stream is sufficient — without any payload fields — to answer at least: (1) the full ordered sequence of tool calls in a connection; (2) which `check` calls returned which problem codes and whether a subsequent call presented a source with a different content hash; (3) the distribution of `terminated_reason` values across `run` calls in the connection; (4) container-cache pressure, measured as the ratio of `unknown_container` errors to `compile` calls.

### Diagnostic Mapping

The existing `Diagnostic` type (from `ironplc-dsl`) carries file ID, source span (byte offsets), problem code, and message. The MCP server converts byte offsets to line/column numbers using the source text supplied in the same call, before serializing to JSON. This is the same conversion the LSP server already performs, and once both servers mature they should share a single implementation (see Future Work).

Line numbers in the serialized diagnostic are 1-indexed. Columns are 1-indexed and count Unicode scalar values — not bytes, not UTF-16 code units — so that a diagnostic's span is identical regardless of how the agent's MCP client encodes strings internally. A tab counts as one column; the server does not interpret tab stops. `end_line`/`end_col` refer to the character immediately after the last character of the span, so an empty span has `start == end`. These are the same semantics as REQ-TOL-023.

### Error Handling

Tool handlers return structured JSON responses in all cases — they never panic or return MCP-level errors for compiler failures. A parse error is a valid `check` response with `ok: false` and populated diagnostics, not an MCP error.

MCP-level errors (invalid tool name, malformed arguments that the SDK rejects before the handler runs) are handled by the SDK.

### Tool Descriptions Agents See

Every MCP tool is registered with a short description string that the client hands to the model. Those strings are the first line of defense against common agent failure modes: picking the wrong tool, escalating to a heavier tool to "try again", or toggling options to make errors disappear. These descriptions are part of the contract, not marketing text.

**REQ-ARC-050** The MCP tool descriptions shipped with the server must follow the guidance below. These are contractual, not flavor text; any change requires a design-note update in this document.

- `parse`: "Syntax check only. Use while drafting to confirm the source tokenizes and parses. Do NOT use this to validate a change — it does not catch type errors, undeclared symbols, or any other semantic rule. Call `check` for that."
- `check`: "Primary validator. Runs parse and full semantic analysis and returns structured diagnostics. ALWAYS run this before reporting success to the user and before calling `compile` or `run`. Self-heal by reading the returned diagnostics, fixing the code, and calling `check` again. Call `explain_diagnostic` to understand any unfamiliar problem code BEFORE editing the source."
- `format`: "Canonical re-rendering using the project's formatter. Safe to call on any parseable source. The server holds no state between calls; if you want the formatted content saved anywhere, store it yourself."
- `symbols`: "Full symbol table for a set of sources. Large responses are capped — prefer the `pou` filter or one of the context tools (`pou_scope`, `project_io`, `types_all`) when you only need part of the answer."
- `list_options`: "Enumerates dialects and feature flags you are allowed to pass in `options`. Every analysis, context, and execution tool requires an `options` object; unknown keys are rejected. Do NOT toggle flags or change dialect to make errors go away — dialect changes are recorded in the log stream."
- `explain_diagnostic`: "Look up the human-readable explanation for a problem code (e.g. `P0042`). Call this before editing code in response to a diagnostic you do not fully understand."
- `project_manifest`: "Flat summary of what is declared in a source set (file names, POU names, UDT names by kind). Cheap to call; use it at the start of a task to build a mental model before drilling in."
- `project_io`: "Inputs the caller can drive and outputs the caller can observe, for planning a `run` call. This is the right tool to call before constructing `stimuli` or deciding which variables to `trace`."
- `pou_scope`: "Every variable visible to a single POU. Prefer this over `symbols` when editing one POU."
- `pou_lineage`: "Upstream and downstream POU dependencies. Use this to decide which other POUs to pull into context before editing one."
- `types_all`: "Every user-defined type with enough detail to reference a field or enum value without re-reading the source."
- `compile`: "Only call this when you need a compiled artifact to `run`. For validation, call `check` instead — `check` is faster, produces the same diagnostics, and does not incur codegen cost. A failing `compile` does not give you any information that a failing `check` would not."
- `container_drop`: "Explicitly releases a compiled container from the cache. Not usually necessary — the cache evicts on LRU pressure — but available for long-running connections."
- `run`: "Simulates a compiled container in the VM for a caller-specified duration. Use this only after `check` passes. Drive inputs over time via `stimuli` and observe outputs via `variables` (or set `trace_outputs: true` to pull every externally visible variable). The returned `trace` is bounded; use the `summary` object when you only care about outcomes. Evaluate pass/fail conditions yourself against the returned trace and `summary.final_values`."

**REQ-ARC-051** Tool descriptions must NOT make claims the server cannot verify. In particular, tool descriptions may not promise things like "always faster than X" or "preferred for all use cases"; they must state concrete semantic differences and call out the cases where the wrong tool is tempting.

### Context Scoping

Context scoping is enforced structurally, not by prose recommendations to the agent. Three mechanisms work together to keep token usage lean:

**REQ-ARC-060** The `symbols` tool accepts an optional `pou: string` filter and caps unfiltered responses at `max_symbols_response_bytes` (see REQ-TOL-055). Agents editing a single POU are pushed toward the filter; a server that hits the cap returns an empty set and a diagnostic that names the context tools instead.

**REQ-ARC-061** The Context Tools group (`project_manifest`, `project_io`, `pou_scope`, `pou_lineage`, `types_all`) is the blessed path for targeted lookups. The tool descriptions in REQ-ARC-050 explicitly steer the agent toward a context tool over `symbols` whenever the agent's question is narrower than "the whole project".

**REQ-ARC-062** Every tool response carries a `response_size_bytes` field in the structured log (see REQ-ARC-041). An analyst reviewing the connection log can trivially spot an agent that is pulling the entire project on every turn instead of scoped context, and use that evidence to refine tool descriptions or add further structural guards.

## Milestones

The design above is intentionally split into two milestones so that the first release can ship without codegen or VM integration.

**Milestone 1 — validation surface.** `parse`, `check`, `format`, `symbols`, `list_options`, `explain_diagnostic`, plus every context tool (`project_manifest`, `project_io`, `pou_scope`, `pou_lineage`, `types_all`). This milestone depends only on the compiler front end, the analyzer, `plc2plc`, and `ironplc-problems`. It covers the highest-value agent use case — "write PLC code, get structured feedback, self-heal" — without pulling in `ironplc-codegen` or `ironplc-vm`.

**Milestone 2 — execution surface.** `compile`, `container_drop`, `run`, the container cache (REQ-ARC-070..073), and the VM sandboxing limits (REQ-ARC-030..035). This milestone adds dependencies on `ironplc-codegen` and `ironplc-vm`.

Both milestones obey every REQ in the Design Principle section (REQ-STL-001..006). The milestone split is a delivery schedule, not an architectural boundary: the stateless tool surface is designed so that adding the Milestone 2 tools does not require revisiting any Milestone 1 interface.

## Future Work

These items are intentionally out of scope for this design. They are listed here so the surface stays small and so implementers and reviewers know what has been considered and deliberately deferred.

- **Stateful caching layer.** The current design re-parses and re-analyzes every `check` call. A future revision may add a content-addressed cache keyed by a hash of `(sources, options)` so that repeated calls on unchanged inputs skip analysis. This is purely a performance optimization and must not change any tool's observable behavior.
- **Server-side assertion evaluation.** This design deliberately omits a `verify` tool that would compile + run + evaluate caller-supplied expectations and return a structured pass/fail result. The decision is to let real agent usage tell us how badly we need it: agents can currently drive `compile` + `run` themselves and evaluate pass/fail against the returned `trace` and `summary.final_values` in their own reasoning. If that workflow produces unreliable pass/fail conclusions in practice — hallucinated passes, missed regressions, float-comparison bugs — a future milestone should add `verify` with a stripped-down expectation grammar (at-final comparisons, `equals` / `not_equals` / `approximately` with tolerance for floats) and the trace-sample guarantees needed to evaluate time-indexed expectations reliably.
- **Persistent test harness.** A future milestone should let the agent register named scenarios out-of-band and invoke them by name, so the agent curates tests alongside the source instead of re-inventing assertions on every turn. This work sits on top of the `verify` deferral above and should not be started before `verify` exists.
- **Aggregate stimulus values for complex types.** REQ-TOL-043 currently requires the caller to supply a full array or full struct when driving one via `stimuli.set`. A follow-up should define partial-update semantics (for example, setting a single array element by index, or setting one field of a struct without supplying the whole object) once real agent usage reveals what shapes are needed.
- **Streaming traces.** Long `run` calls currently return the complete trace in a single response. A future revision may offer incremental trace delivery via MCP's streaming facilities, which would let the agent react to intermediate values without extending `duration_ms` conservatively.
- **On-disk workspace management.** The server never touches disk in this design. An agent that wants to persist an edit writes it through its own filesystem tools. A future companion tool or MCP client helper could formalize the "write these sources back to a project directory" workflow, but it deliberately lives outside the MCP boundary — the server itself stays stateless and file-system-free.
- **IEC 61131-3 Edition 3 dialect semantics.** The `dialect` preset exists but the precise set of Ed. 3 features supported by the MCP server should be documented once the compiler's Ed. 3 support is complete. A future revision of this doc should link directly to a compatibility matrix.
- **LSP parity for the byte-offset-to-line-column conversion.** The LSP server already converts byte offsets to line/column diagnostics (see Diagnostic Mapping). The MCP server borrows that code; once both mature, they should share a single implementation so that a diagnostic seen in an editor and a diagnostic seen by an agent are byte-identical.
