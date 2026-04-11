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
3. **Simulate** — agent calls `run` or `verify` to confirm logical correctness against expected outputs
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

Every tool in this design is a pure function of its explicit inputs. There is no session workspace, no cached analysis between calls, no `--project-dir` pre-load, and no disk I/O from any tool handler. The single exception is the process-level container cache described under Architecture — it stores compiled `.iplc` bytes keyed by an opaque handle so that `compile → run` and `compile → verify` can hand off without routing bytecode through the LLM context. The cache is a performance optimization, not a source of truth; its contents are never visible to any tool that accepts `sources`.

The agent is the sole owner of project state. It holds the files (in its own context, on its own filesystem, or both), decides which subset to send on any given call, and persists edits on its own side. The MCP server's job is to answer one question at a time about whichever sources the agent hands it, and to hand back structured results the agent can react to.

**REQ-STL-001** Every analysis, context, and execution tool accepts a required `sources` parameter: an array of `{ name: string, content: string }` objects. The tool operates on exactly the supplied sources for that single call. Subsequent calls that want the same inputs must re-send them.

**REQ-STL-002** Every analysis, context, and execution tool accepts a required `options` object that specifies the compiler dialect and any feature-flag overrides. The tool uses those options for exactly that single call. The server does not carry options across calls and does not apply implicit defaults; callers that want the standard IEC 61131-3 Edition 2 dialect must pass `{ "dialect": "iec61131-3-ed2" }` explicitly. The set of valid keys is the set returned by `list_options`; any other key is rejected with a diagnostic and the tool does not run.

**REQ-STL-003** The server holds no per-client state across tool calls other than the process-level container cache (see Container Cache under Architecture). Two successive calls from the same MCP client that supply identical `sources` and `options` produce identical responses up to non-determinism in wall-clock fields such as log timestamps.

**REQ-STL-004** File identity inside a single call is carried by the `name` field of each `sources` entry. Names must be valid UTF-8, non-empty, at most 256 bytes, and must not contain NUL, `/`, or `\`. Duplicate names within a single `sources` array are rejected with a diagnostic before any analysis runs. The server does not interpret names as filesystem paths and never touches the filesystem with them; they exist so that diagnostics can cite a file identifier the agent already recognizes in its own context.

**REQ-STL-005** Every tool response includes a top-level `ok: boolean` field. `ok` is `true` when the tool produced its primary result (for analysis tools, a diagnostics array with no `error`-severity entries; for `compile`, a non-null `container_id`; for `run`, a completed trace; for `verify`, a decided pass/fail) and `false` when it did not. The `ok` field never replaces a tool's specific result fields; it exists as a single uniform success predicate so that agent code handling many tools can share one success check.

**REQ-STL-006** The server performs no disk I/O from any tool or resource handler. It does not accept filesystem paths as tool inputs, does not read files relative to any working directory, and does not write compilation or analysis artifacts to disk. The only files the server process ever opens are its own log output (see Logging and Observability) and, optionally, its own binary-embedded problem-code documentation.

## Resources

Resources give the agent contextual knowledge without requiring it to read raw source files. Every resource reads from the session workspace (see Session State Model).

### `ironplc://project/manifest`

**REQ-RES-001** The `ironplc://project/manifest` resource returns the list of all source files in the session workspace.

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

**REQ-RES-040** The `ironplc://source/{file}` resource returns the raw text of the named source file from the session workspace.

**REQ-RES-041** The `{file}` template parameter matches the file names returned by `ironplc://project/manifest`.

**REQ-RES-042** The source resource returns a JSON object with the fields `file`, `content`, and `length_bytes`.

**REQ-RES-043** When the requested file is not in the session workspace, the resource returns `found: false`, a `null` `content`, and a populated `diagnostics` array, rather than an MCP-level error.

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

**REQ-RES-031** The lineage resource returns a JSON object with three fields: `pou` (the requested POU name), `upstream` (an array of POU names that the requested POU depends on, directly or transitively), and `downstream` (an array of POU names that depend on the requested POU, directly or transitively). This JSON representation is the default because agents parse adjacency-list JSON more reliably than DOT syntax, and the JSON encoding is shorter for the same information.

**REQ-RES-032** The lineage resource accepts an optional `format` query-string parameter (`ironplc://pou/{name}/lineage?format=dot`) that returns the same graph in DOT (Graphviz) syntax instead. DOT remains available for callers that want to render the graph visually, but it is never the default and is never required by any tool in this design.

**Output:**
```json
{
  "pou": "Motor",
  "upstream":   ["PID", "Scale"],
  "downstream": ["Main"]
}
```

## Tools

Tools are grouped into three categories:

1. **Workspace mutation tools** — `workspace_set`, `workspace_put`, `workspace_remove`, `workspace_clear`, `workspace_set_options`. These modify the session workspace.
2. **Analysis tools** — `parse`, `check`, `format`, `symbols`, `list_options`, `explain_diagnostic`. These read the session workspace (or a per-call `sources` override) and return structured information.
3. **Execution tools** — `compile`, `run`, `verify`. These produce or consume bytecode and/or drive the VM.

### `workspace_set`

Replaces the entire session workspace with a new set of sources. This is the "load from scratch" path an agent uses when it receives a user-supplied program text or wants to wipe state between unrelated tasks.

**Inputs:**
- `sources`: array of `{ name: string, content: string }`

**REQ-TOL-100** The `workspace_set` tool replaces the session workspace's file set with the supplied `sources`; any files previously in the workspace are discarded.

**REQ-TOL-101** The `workspace_set` tool leaves the session's active options unchanged.

**REQ-TOL-102** The `workspace_set` tool returns the resulting `files` list and a `diagnostics` array; diagnostics are only populated if a file name is duplicated or otherwise malformed, not for parse or semantic errors (those are surfaced by `check`).

**Output:**
```json
{ "files": ["main.st", "types.st"], "diagnostics": [] }
```

### `workspace_put`

Adds a single file to the session workspace, or replaces the content of an existing file with the same name. This is the incremental-edit path.

**Inputs:**
- `name: string`
- `content: string`

**REQ-TOL-110** The `workspace_put` tool inserts a new file into the session workspace when no file with `name` exists, or replaces the content of the existing file when one does.

**REQ-TOL-111** The `workspace_put` tool invalidates any cached semantic-analysis artifacts so subsequent resource reads reflect the new content.

**REQ-TOL-112** The `workspace_put` tool returns the current `files` list after the mutation and a `diagnostics` array (empty on success).

### `workspace_remove`

Removes a single file from the session workspace.

**Inputs:**
- `name: string`

**REQ-TOL-120** The `workspace_remove` tool deletes the named file from the session workspace and invalidates cached semantic-analysis artifacts.

**REQ-TOL-121** The `workspace_remove` tool returns `found: false` and a populated `diagnostics` array when no file with the given name is present, rather than raising an MCP-level error.

### `workspace_clear`

Empties the session workspace.

**Inputs:** none.

**REQ-TOL-130** The `workspace_clear` tool removes every file from the session workspace and invalidates cached semantic-analysis artifacts.

**REQ-TOL-131** The `workspace_clear` tool leaves the session's active options unchanged.

### `workspace_set_options`

Updates the session's active compiler options. These options become the defaults for any subsequent analysis or execution tool call that does not pass its own `options`.

**Inputs:**
- `options`: object with `dialect` and individual feature flags (same schema as `check.options`)

**REQ-TOL-140** The `workspace_set_options` tool replaces the session's active options with the supplied `options` object.

**REQ-TOL-141** The `workspace_set_options` tool rejects unknown option keys with a diagnostic listing the unknown keys and does not modify the session's active options in that case.

**REQ-TOL-142** The `workspace_set_options` tool invalidates any cached semantic-analysis artifacts, because changing options can change parse and analysis outcomes.

### `parse`

Runs the parse stage only — no semantic analysis. Returns syntax diagnostics (malformed tokens, missing keywords, structural grammar errors).

Use this for rapid iteration on code structure. It is faster than `check` and useful when the agent is drafting code and wants to confirm it parses before investing in semantic correctness.

**Inputs:**
- `sources`: optional array of `{ name: string, content: string }` — when omitted, the tool parses the session workspace (see Session State Model)
- `options`: optional object with `dialect` and individual feature flags, same as `check` — when omitted, the session's active options are used

**REQ-TOL-010** The `parse` tool runs the parse stage only and does not run semantic analysis.

**REQ-TOL-011** The `parse` tool returns a `diagnostics` array using the same format as `check`.

**REQ-TOL-012** The `parse` tool accepts the same `options` object as `check`, since dialect and feature flags affect the parser.

**REQ-TOL-013** The `parse` tool operates on the session workspace when `sources` is omitted and on the supplied `sources` otherwise; a per-call `sources` override does not mutate the session workspace (see REQ-SES-020).

**REQ-TOL-014** The `parse` tool returns a best-effort `structure` array alongside `diagnostics`, even when `diagnostics` contains errors. Each entry describes a top-level declaration the parser was able to recognize and contains `kind` (`"program"`, `"function"`, `"function_block"`, `"type"`, or `"configuration"`), `name` (string, or `null` when the parser could not recover a name), `file`, `start_line`, and `end_line`. This gives the agent an outline of its own in-progress draft to reason about even when the source is not yet valid — without it, a broken parse leaves the agent with only an opaque diagnostic and no structural context.

**Output:**
```json
{
  "structure": [
    { "kind": "program", "name": "Main", "file": "main.st", "start_line": 1, "end_line": 22 },
    { "kind": "function_block", "name": null, "file": "main.st", "start_line": 24, "end_line": 40 }
  ],
  "diagnostics": [
    { "code": "P0001", "message": "expected `;`", "file": "main.st",
      "start_line": 18, "start_col": 10, "end_line": 18, "end_col": 11, "severity": "error" }
  ]
}
```

### `check`

Runs the full parse and semantic analysis pipeline — the same stages as the CLI `check` command — and returns diagnostics. This covers syntax errors, type errors, undeclared symbols, and all other semantic rules. It stops before code generation, so no bytecode is produced.

This is the highest-value tool. AI assistants use it to validate code they generate before presenting it to the user. The JSON format enables self-healing loops: the agent reads the diagnostics and fixes the code.

**Inputs:**
- `sources`: optional array of `{ name: string, content: string }` — inline source text for a "what-if" check. When omitted, the tool runs against the session workspace (see Session State Model).
- `options`: optional object with:
  - `dialect: string` — one of `"iec61131-3-ed2"` (default), `"iec61131-3-ed3"`, `"rusty"`. Selects a preset that enables the appropriate flags in one shot.
  - individual feature flags (e.g. `allow_c_style_comments: bool`) — override specific flags on top of the dialect preset. The full list of flags and their descriptions is returned by `list_options`.
  - When omitted, the session's active options are used.

**REQ-TOL-020** The `check` tool runs the parse stage and the full semantic analysis stage on the provided sources.

**REQ-TOL-021** The `check` tool does not run code generation.

**REQ-TOL-022** The `check` tool returns a `diagnostics` array; an empty array indicates no errors. The caller determines success by checking whether any diagnostic has `severity: "error"`.

**REQ-TOL-023** Each diagnostic in the `check` response includes: `code`, `message`, `file`, `start_line`, `start_col`, `end_line`, `end_col`, and `severity`.

**REQ-TOL-024** The `check` tool never returns an MCP-level error for a compiler failure; parse and semantic errors are returned as diagnostics.

**REQ-TOL-025** The `check` tool accepts an optional `dialect` string in `options`; when omitted, `"iec61131-3-ed2"` is used.

**REQ-TOL-026** The `check` tool accepts individual feature flag overrides in `options` that are applied on top of the dialect preset.

**REQ-TOL-027** The `check` tool operates on the session workspace when `sources` is omitted and on the supplied `sources` otherwise; a per-call `sources` override does not mutate the session workspace (see REQ-SES-020).

**REQ-TOL-028** When the `check` tool is called with a per-call `options` override whose `dialect` differs from the session's active dialect, or whose feature flags differ from the session's active flags, the response includes a diagnostic with `severity: "warning"` and `code: "P-MCP-001"` stating that a dialect or feature-flag override is active. This warning is always emitted even when no other diagnostics are present, so that an analyst watching the log stream (see REQ-ARC-045) can detect agents that toggle dialect to erase errors rather than fix them. The same warning is emitted by `parse`, `compile`, and `verify` on per-call options overrides.

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
- `sources`: optional array of `{ name: string, content: string }` — when omitted, the tool formats the session workspace
- `options`: optional object with the same `dialect` and feature flags as `check` — when omitted, the session's active options are used

**REQ-TOL-080** The `format` tool parses each source in the request and, on successful parse, returns the rendered canonical form in a `sources` array whose entries match the input names one-to-one.

**REQ-TOL-081** When any source fails to parse, the `format` tool returns the failing source's original content unchanged in the `sources` array, records the parser's diagnostics in `diagnostics`, and sets `formatted: false` for that entry.

**REQ-TOL-082** The `format` tool is idempotent: running `format` on its own output returns byte-identical content.

**REQ-TOL-083** The `format` tool produces the same canonical output that the `plc2plc` crate produces for a given AST and dialect.

**REQ-TOL-084** The `format` tool operates on the session workspace when `sources` is omitted and on the supplied `sources` otherwise; a per-call `sources` override does not mutate the session workspace. In particular, `format` does not write its formatted output back into the session; the caller must call `workspace_put` explicitly if it wants to persist the formatted content.

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
- `sources`: optional array of `{ name: string, content: string }` — when omitted, the tool reads from the session workspace
- `options`: optional compiler options — when omitted, the session's active options are used
- `pou`: optional string — when present, the response is narrowed to just the named POU and the types its declarations reference (see REQ-ARC-060)

**REQ-TOL-050** The `symbols` tool returns the top-level declarations for programs, functions, function blocks, and types found in the sources under analysis.

**REQ-TOL-051** Each program entry in the `symbols` response includes the program name and its variable declarations. Each variable entry contains `name`, `type`, `direction` (one of `"Local"`, `"In"`, `"Out"`, `"InOut"`, `"Global"`, `"External"`), `address` (the direct-variable string such as `"%IX0.0"` when the variable is mapped to a hardware address, otherwise `null`), and `external` (`true` when the variable can be driven from outside the program — i.e. `direction` is `"In"`, `"External"`, or `"Global"`, or `address` is a `%I*` hardware input).

**REQ-TOL-052** Each function entry in the `symbols` response includes the function name, return type, and parameter list.

**REQ-TOL-053** The `symbols` response includes a `diagnostics` array using the same format as `check`.

**REQ-TOL-054** The `symbols` tool operates on the session workspace when `sources` is omitted and on the supplied `sources` otherwise; a per-call `sources` override does not mutate the session workspace (see REQ-SES-020).

**REQ-TOL-055** When the `pou` input is present, the `symbols` response includes only the matching POU (in exactly one of `programs`, `functions`, or `function_blocks`) and only the types actually referenced by that POU's declarations. When no POU with the given name exists, the response returns `found: false` and an empty `programs`/`functions`/`function_blocks`/`types` set along with a diagnostic, rather than an MCP-level error.

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

### `get_resource`

Returns the content of any `ironplc://` resource as a tool call, so an autonomous agent can reach the project-scoped knowledge surface without relying on MCP-client-specific resource-exposure behavior.

MCP distinguishes between **tools** (freely callable by the model) and **resources** (typically surfaced to the human and selectively attached to turns). In IronPLC's case the project-scoped surfaces — `manifest`, `source/{file}`, `pou/{name}/scope`, `pou/{name}/lineage`, `types/all`, `project/io` — are agent-facing: the agent needs to read them itself to plan an edit or a verification. Exposing them only as resources is a trap, because on several MCP clients the model cannot pull resources autonomously. `get_resource` closes that gap without duplicating every resource as a separate tool.

**Inputs:**
- `uri: string` — any `ironplc://` URI served by this server, with template parameters substituted (for example, `"ironplc://pou/Motor/scope"` or `"ironplc://source/main.st"`)

**REQ-TOL-160** The `get_resource` tool resolves the `uri` against the server's resource routing table and returns a response object containing `uri` (echoing the input), `content` (the JSON body the resource would have returned), and `diagnostics`.

**REQ-TOL-161** The `get_resource` tool returns `content: null` with a populated `diagnostics` array when the URI does not match any registered resource template, or when required template parameters are missing. It does not raise an MCP-level error.

**REQ-TOL-162** `get_resource` returns the **same** data a direct resource read would have returned for the same URI, computed against the same session workspace state (see REQ-SES-010 / REQ-SES-011). A client that supports both tool calls and resource reads can use them interchangeably.

**REQ-TOL-163** `get_resource` log entries follow REQ-ARC-041's resource-summary shape (resolved URI, response size in bytes, bound template parameters), not the tool-summary shape, so that log analysts see a uniform view of how the project-scoped surface is being consulted regardless of which call path the agent used.

**Output:**
```json
{
  "uri": "ironplc://pou/Motor/scope",
  "content": {
    "pou": "Motor",
    "variables": [
      { "name": "Start", "type": "BOOL", "direction": "In", "initial_value": "FALSE" }
    ]
  },
  "diagnostics": []
}
```

### `compile`

Runs the full pipeline (parse → semantic analysis → codegen) and returns an opaque, session-scoped **container handle** that identifies the compiled `.iplc` bytes inside the server. Also returns the task configuration extracted from the compiled program, which the agent can use to choose a sensible `duration_ms` for `run`.

The container handle is the primary transport: agents pass it back to `run` and `verify` without ever routing the bytecode through the LLM context. Base64-encoded bytes are available on request for clients that need to persist or transmit the artifact.

**Inputs:**
- `sources`: optional array of `{ name: string, content: string }` — when omitted, the tool compiles the session workspace
- `options`: optional compiler options — when omitted, the session's active options are used
- `include_bytes`: optional boolean (default `false`) — when `true`, the response also includes `container_base64`

**REQ-TOL-030** The `compile` tool returns a session-scoped `container_id` string that uniquely identifies the compiled `.iplc` container inside the current session.

**REQ-TOL-031** The `compile` tool returns `container_id: null` and a populated `diagnostics` array on failure. The caller determines success by checking whether `container_id` is non-null.

**REQ-TOL-032** The `compile` tool returns a `tasks` array describing each task declared in the program, including `name`, `priority`, and `interval_ms` (the cyclic interval in milliseconds, or `null` for event-triggered tasks).

**REQ-TOL-033** The `compile` tool returns a `programs` array listing each program name and the task it is bound to.

**REQ-TOL-034** The `compile` tool operates on the session workspace when `sources` is omitted and on the supplied `sources` otherwise; a per-call `sources` override does not mutate the session workspace (see REQ-SES-020).

**REQ-TOL-035** The `compile` tool returns the `.iplc` container encoded as a base64 string in `container_base64` only when the caller sets `include_bytes: true`; otherwise `container_base64` is `null`. This keeps the default response small and lets the agent pass `container_id` back to `run`/`verify` without ever routing the bytecode through the LLM context.

**REQ-TOL-036** The server stores the compiled container bytes in a session-scoped container cache keyed by `container_id`. Entries are dropped when the session ends or when the caller invokes `container_drop`.

**Output:**
```json
{
  "container_id": "c_9f3a1e",
  "container_base64": null,
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

### `container_drop`

Removes a previously compiled container from the session container cache. Agents normally do not need to call this — the cache is already bounded by the session lifetime — but it is provided for long-running sessions that churn through many `compile` calls.

**Inputs:**
- `container_id: string`

**REQ-TOL-150** The `container_drop` tool removes the container identified by `container_id` from the session container cache and returns `removed: true`.

**REQ-TOL-151** The `container_drop` tool returns `removed: false` and a populated `diagnostics` array when the `container_id` is unknown, rather than raising an MCP-level error.

### `run`

Loads a compiled `.iplc` container into the IronPLC VM and executes it for a specified duration of simulated time, under server-enforced resource limits. The agent derives a sensible `duration_ms` from the task configuration returned by `compile` — for example, one full period of the slowest cyclic task.

This enables the agent to verify logical correctness, not just syntactic validity. The agent can drive inputs over time via a `stimuli` schedule and observe the resulting output values in the returned trace.

**Inputs:**
- `container_id: string` — the session-scoped handle returned by `compile` (preferred)
- `container_base64: string` — inline `.iplc` bytes; exactly one of `container_id` and `container_base64` must be present
- `duration_ms: number` — simulated time to run in milliseconds
- `variables: [string]` — list of fully-qualified variable names to include in the trace (see Variable Naming in Architecture)
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

**REQ-TOL-040** The `run` tool executes the referenced `.iplc` container in the IronPLC VM for the simulated duration specified by `duration_ms`, deriving the number of scan cycles from the task intervals declared in the container. Exactly one of `container_id` and `container_base64` must be present; when `container_id` is supplied, the server looks up the compiled bytes in the session container cache (see REQ-TOL-036).

**REQ-TOL-041** The `run` tool returns a `trace` array. Each entry contains `time_ms` (simulated milliseconds since start of run), `task` (the name of the task whose cycle end produced the entry), and `variables` (a map from the fully-qualified names requested in `variables` to their values at that instant). Entries are time-ordered by `time_ms`; ties are broken by task priority (lower priority number first), then by task name. Each requested variable name must be fully qualified and must resolve against the loaded container (see REQ-ARC-020 and REQ-ARC-021); unresolved or ambiguous names abort the run with a diagnostic. Wildcard names (for example `"*"`, `"Main.*"`) are rejected with a diagnostic — the agent must explicitly enumerate the variables of interest. The `variables` array's length is capped at the server-configured `max_variables_per_run` limit (see VM Sandboxing); a request that exceeds the cap is rejected before the VM starts.

**REQ-TOL-042** The `run` tool accepts a time-ordered `stimuli` array. Each stimulus is applied at the start of the first scan cycle whose simulated end-time is greater than or equal to the stimulus `time_ms`. Values persist in their target variables until overwritten by a later stimulus or by the program itself. The `run` tool only permits stimuli to write variables reported by the `ironplc://project/io` resource as externally drivable (`VAR_INPUT` of Programs, `VAR_EXTERNAL`, globals, and `%I*`-mapped variables); attempts to write a local, `VAR_OUTPUT`, or `%Q*`-mapped variable result in a diagnostic and abort the run. A stimulus whose value does not match the declared type of the target variable (for example, setting a `BOOL` to `42`) is likewise rejected.

**REQ-TOL-043** The JSON encoding of values in `stimuli.set`, `trace[].variables`, and `summary.final_values` is recursive and is defined for every IEC 61131-3 type the compiler accepts:
  - `BOOL` ↔ JSON boolean.
  - `SINT`/`INT`/`DINT`/`USINT`/`UINT`/`UDINT` ↔ JSON number.
  - `LINT`/`ULINT` ↔ JSON string in decimal (to preserve 64-bit precision).
  - `REAL`/`LREAL` ↔ JSON number. The special IEEE-754 values are encoded as the JSON strings `"NaN"`, `"Infinity"`, and `"-Infinity"`.
  - `STRING`/`WSTRING` ↔ JSON string.
  - `TIME`/`DATE`/`DT`/`TOD` ↔ JSON string in IEC 61131-3 literal syntax (e.g. `"T#500ms"`, `"D#2025-01-01"`).
  - Enumeration values ↔ JSON string containing the symbolic enum value name (e.g. `"Running"`).
  - `ARRAY[L..U] OF T` ↔ JSON array of length `U - L + 1`, lowest index first, each element encoded per this same rule.
  - `STRUCT` / function-block instances ↔ JSON object whose keys are field names and whose values are encoded per this same rule.
  - Nested aggregates apply the rules recursively.
  Any value that does not match its declared type is rejected with a diagnostic.

**REQ-TOL-044** The `run` tool accepts an optional `trace.mode` of `"every_cycle"` (default), `"every_ms"`, `"on_change"`, or `"final_only"`:
  - `"every_cycle"` emits one sample per task cycle end, as described in REQ-TOL-041.
  - `"every_ms"` requires `trace.interval_ms` and emits at most one sample per interval. A sample is emitted at the first task cycle end whose `time_ms` is greater than or equal to the next interval tick; samples carry the actual `time_ms` (they are not interpolated).
  - `"on_change"` emits a sample only when at least one variable in `variables` has a different value than the most recently emitted sample for that variable set. The first cycle always emits.
  - `"final_only"` emits exactly one sample at the end of the run, containing the final values of every requested variable. `task` on that sample is the literal string `"final"`.

**REQ-TOL-045** The `run` tool accepts an optional `tasks` filter. When present, only cycles from the named tasks appear in the trace; cycles from other tasks still execute in the VM and still influence `summary.completed_cycles`, but are not emitted as samples.

**REQ-TOL-046** The `run` tool caps the returned trace at `min(trace.max_samples, server_max_samples)` entries, where `server_max_samples` is a server-configured limit described under VM Sandboxing. When the cap is hit, the last entry in `trace` is the most recent sample that fit; `truncated: true` is set in the response; and `terminated_reason` is set to `"sample_cap"`. The run still executes to completion (up to other limits) — only the emitted trace is truncated.

**REQ-TOL-047** The `run` tool enforces the resource limits described under VM Sandboxing (maximum simulated `duration_ms`, VM fuel, wall-clock time). When a limit is exceeded, or when the VM encounters a trap, the run terminates early. In all early-termination cases the response carries the partial trace up to the last cycle that completed before termination, a populated `diagnostics` entry identifying the cause, and `terminated_reason` set to one of `"duration"`, `"fuel"`, `"wall_clock"`, `"sample_cap"`, `"error"`, or — for a run that finished cleanly — `"completed"`. An agent-supplied `limits` override may only tighten the server-configured bounds, never loosen them.

**REQ-TOL-048** The `run` tool's response always includes a `summary` object with at least: `final_values` (a map of every requested variable to its value at the last simulated instant, regardless of `trace.mode`), `completed_cycles` (a map from task name to the number of cycles that completed for that task), and `terminated_reason`. `summary` is populated even when the trace is empty because of `"final_only"` mode or because `"on_change"` never fired.

**Output:**
```json
{
  "trace": [
    { "time_ms": 10, "task": "Main", "variables": { "Main.MotorRun": false, "Main.Counter": 0 } },
    { "time_ms": 20, "task": "Main", "variables": { "Main.MotorRun": true,  "Main.Counter": 1 } }
  ],
  "truncated": false,
  "terminated_reason": "duration",
  "summary": {
    "final_values": { "Main.MotorRun": true, "Main.Counter": 50 },
    "completed_cycles": { "Main": 100, "Slow": 10 }
  },
  "diagnostics": []
}
```

### `verify`

Compiles the provided sources, runs them in the VM against a caller-supplied stimulus schedule, evaluates caller-supplied expectations, and returns a single pass/fail result with the trace. This is the "one call the agent makes when the user says 'make it work'".

The primary reason this is a dedicated tool rather than a prompt is that expectation evaluation lives on the server side: the agent gets back `{ passed: true }` or a structured list of failures. Agents react reliably to structured pass/fail; reducing a raw trace into assertions in the agent's own reasoning is error-prone and burns tokens.

**Inputs:**
- `sources`: optional array of `{ name: string, content: string }` — when omitted, the tool verifies the session workspace
- `options`: optional compiler options (same as `check`) — when omitted, the session's active options are used
- `duration_ms: number`
- `stimuli: [Stimulus]` — same shape as `run`'s `stimuli`
- `expectations: [Expectation]`
- `trace_variables: [string]` — optional; additional fully-qualified names to include in the returned trace beyond those referenced by expectations
- `trace: TraceOptions` — optional; same shape as `run`'s `trace`. Because `verify` must evaluate expectations against recorded samples, a user-supplied `trace.mode` that drops the sample at which an expectation's `at` resolves causes that expectation to fail with `actual: null`.
- `limits: LimitOverrides` — optional; same shape and semantics as `run`'s `limits` (tighten only, never loosen)

An `Expectation` is:
```json
{
  "variable": "Main.MotorRun",
  "at": "final",
  "equals": true
}
```

The `at` field is either the literal string `"final"` (evaluated at the last recorded cycle) or `{ "time_ms": N }` (evaluated at the first recorded cycle whose `time_ms >= N`). Exactly one comparator field must be present: `equals`, `not_equals`, `greater_than`, `greater_or_equal`, `less_than`, `less_or_equal`, or `approximately` with a required `tolerance` field for floating-point comparisons.

**REQ-TOL-090** The `verify` tool compiles the provided sources, runs them against the supplied stimuli and duration using the same VM semantics, value encoding, trace options, and resource limits as `run`, evaluates the supplied expectations, and returns `passed`, `failures`, `trace`, `summary`, `truncated`, and `terminated_reason`.

**REQ-TOL-091** The `verify` tool sets `passed: false` and returns the full list of unsatisfied expectations in `failures` when any expectation is not met. Each failure entry includes the original expectation, the `actual` value observed (or `null` if the sample required by the expectation's `at` was not recorded), and the `time_ms` at which the comparison was evaluated.

**REQ-TOL-092** The `verify` tool returns `passed: false` with compile or run diagnostics in `diagnostics` when the sources fail to compile, when the VM traps, or when the run terminates early due to a resource limit. In these cases `failures` is empty, `trace` is empty or partial up to the point of termination, and `terminated_reason` carries the specific cause inherited from `run` (`"error"`, `"duration"`, `"fuel"`, `"wall_clock"`, or `"sample_cap"`).

**REQ-TOL-093** The `verify` tool evaluates the `approximately` comparator as `abs(actual - expected) <= tolerance`; both `equals` and `not_equals` on floating-point types are rejected with a diagnostic instructing the caller to use `approximately`.

**REQ-TOL-094** The `verify` tool includes in its returned `trace` at least every variable referenced by an expectation, plus any variable named in `trace_variables`. The server is free to suppress other variables from the trace to stay within the sample cap.

**REQ-TOL-095** The `verify` tool operates on the session workspace when `sources` is omitted and on the supplied `sources` otherwise; a per-call `sources` override does not mutate the session workspace (see REQ-SES-020). The `verify` tool does not populate the session container cache — any compiled artifact it produces is discarded at the end of the call.

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
    { "time_ms": 10, "task": "Main", "variables": { "Main.Start": false, "Main.MotorRun": false } }
  ],
  "truncated": false,
  "terminated_reason": "completed",
  "summary": {
    "final_values": { "Main.Start": false, "Main.MotorRun": false },
    "completed_cycles": { "Main": 100 }
  },
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
- `ironplc-project` — provides the `Project` trait (used to back the in-memory session workspace) and `FileBackedProject` (used only by the optional `--project-dir` startup pre-load)
- `ironplc-plc2plc` — for the `format` tool
- `ironplc-codegen` — for the `compile` and `verify` tools
- `ironplc-vm` — for the `run` and `verify` tools
- `ironplc-problems` — for the `explain_diagnostic` tool
- An MCP SDK crate (see below)

### MCP SDK

The server uses [`rmcp`](https://crates.io/crates/rmcp) (the official Rust SDK from the MCP project), which provides the stdio transport, JSON-RPC dispatch, and tool registration macros. It is `async` and uses `tokio`.

### Source Handling

**REQ-ARC-010** Source text enters the server through one of two paths:

1. **Session workspace** — populated via `workspace_set`, `workspace_put`, `workspace_remove`, `workspace_clear`, or the optional `--project-dir` startup pre-load. The session workspace is the default subject of every project-scoped resource and every analysis / execution tool that is not given inline `sources` (see Session State Model).

2. **Per-call `sources` override** — an inline `{ name: string, content: string }` array passed directly to a tool call. Per-call overrides are a "what-if" mechanism and do not mutate the session workspace.

Neither path accepts raw filesystem paths as arguments in any tool or resource. MCP clients (AI assistants) work with text in memory, not files on disk, and the two-path model avoids path-resolution issues across different environments while still supporting quick "what-if" experiments on top of an established project.

**REQ-ARC-011** Each `{ name, content }` pair the server receives — whether from a workspace mutation tool or from a per-call `sources` override — is mapped to a `FileId::from_string(name)` and loaded via `change_text_document` against the appropriate `Project` instance.

**REQ-ARC-012** For per-call `sources` overrides, the server constructs a temporary `Project` instance for the duration of the call and discards it once the response is produced; this prevents override data from leaking into the session workspace.

**REQ-ARC-013** The session workspace is backed by an in-memory `Project` implementation. `FileBackedProject` is used only by the optional `--project-dir` startup pre-load to walk a directory and enumerate files; the resulting files are then copied into the in-memory session workspace. After startup, no tool or resource reads from the on-disk project directory.

### Variable Naming

**REQ-ARC-020** Variable names appearing in `run.variables`, `run.stimuli[].set`, `verify.trace_variables`, `verify.expectations[].variable`, and the `inputs`/`outputs` arrays of `ironplc://project/io` are fully qualified. The format is `<program_name>.<variable_name>` for program-scoped variables, `<program_name>.<fb_instance>.<variable_name>` for variables inside function-block instances, and the bare variable name (no prefix) for globals and resource-level variables.

**REQ-ARC-021** When a requested variable name is ambiguous or does not resolve against the loaded container, the server returns a diagnostic identifying the unresolved name and aborts the tool call.

Fully-qualified names are required even when only one program exists, so that agent-authored prompts and saved scenarios remain valid as a project grows.

### VM Sandboxing and Resource Limits

`run` and `verify` execute agent-supplied code in the IronPLC VM. Without explicit bounds, a pathological program (infinite loop, runaway counter, arbitrarily long simulation) can pin a CPU and blow out the agent's context with an unbounded trace. The server therefore enforces a set of resource limits on every VM invocation.

The limits are configured at server startup and exposed as a `LimitOverrides` object that `run` and `verify` callers may use to **tighten** the bounds for a single call. Callers cannot loosen them: a per-call value that exceeds the server-configured default is rejected with a diagnostic.

```json
{
  "max_duration_ms": 60000,
  "max_fuel": 50000000,
  "max_wall_clock_ms": 5000,
  "max_samples": 1000,
  "max_variables_per_run": 64
}
```

**REQ-ARC-030** The server imposes a `max_duration_ms` (simulated time), a `max_fuel` (VM instruction budget), a `max_wall_clock_ms` (real-world execution time), a `max_samples` (trace entry cap), and a `max_variables_per_run` (maximum length of the `variables` input to `run` and `verify`) on every VM invocation. The defaults are configurable at server startup via command-line arguments (e.g. `--max-duration-ms`, `--max-fuel`, `--max-wall-clock-ms`, `--max-samples`, `--max-variables-per-run`) and have sane defaults for an interactive agent session: 60000 ms simulated, 50000000 fuel, 5000 ms wall-clock, 1000 samples, 64 variables per run.

**REQ-ARC-031** The server rejects any `limits` override in `run.limits` or `verify.limits` whose field exceeds the server-configured default for that field, returning a diagnostic that names the offending field and does not start the VM.

**REQ-ARC-032** When a VM invocation would exceed a limit, the VM terminates cleanly at the end of the most recent completed task cycle. The `run`/`verify` response includes a diagnostic identifying the exceeded limit and sets `terminated_reason` to `"duration"`, `"fuel"`, `"wall_clock"`, or `"sample_cap"` as appropriate.

**REQ-ARC-033** The `max_fuel` budget is shared across all tasks for a single VM invocation; fuel consumed by any task counts against the same budget. Stimulus application is billed against fuel.

**REQ-ARC-034** When a VM invocation completes without exceeding any limit, `terminated_reason` is `"completed"`. When the VM traps (type error, division by zero, array bounds violation, etc.) it is `"error"`.

**REQ-ARC-035** The server is not required to enforce wall-clock limits with hard real-time precision. The implementation is permitted to check the wall-clock between task cycle ends, so the actual termination time may exceed `max_wall_clock_ms` by up to one task cycle's worth of VM work.

### Logging and Observability

The MCP server is the first place we get to watch a real AI agent drive the IronPLC toolchain. Understanding how agents actually use it — which tools they reach for, in what sequence, how often `check` diagnostics lead to a successful self-heal on the next call, how often `run`/`verify` terminates on a resource limit rather than completing — is essential for refining the tool surface, the problem-code docs, and the prompts. This observability must be designed in, not grafted on after the fact.

**REQ-ARC-040** The server emits a structured log entry for every tool call and every resource read. Each entry contains at minimum: `session_id` (a UUID assigned when the session starts), `seq` (a monotonic per-session counter), `timestamp` (ISO 8601 UTC), `kind` (`"tool"` or `"resource"`), `name` (the tool name, or the resource URI template such as `"ironplc://pou/{name}/scope"`), `duration_ms` (wall-clock execution time of the handler), `outcome` (`"ok"` or `"error"`), and — when `outcome` is `"error"` — `error_kind` drawn from a stable taxonomy (`"invalid_arguments"`, `"unknown_name"`, `"limit_exceeded"`, `"parse_failed"`, `"analysis_failed"`, `"vm_trap"`, `"internal"`).

**REQ-ARC-041** Each log entry additionally includes a small tool-specific or resource-specific summary so that an analyst can reconstruct agent behavior without the payload itself:
  - Analysis tools (`parse`, `check`, `format`, `symbols`): `source_count`, `source_total_bytes`, `used_session_workspace` (`true` when the call read the session, `false` when it used a per-call `sources` override), `options_override` (`true` when a per-call `options` override was supplied, `false` otherwise), `dialect_override` (the overridden dialect id, or `null`), `diagnostic_count`, `error_count`, `warning_count`, and (for `check`) a sorted deduplicated `problem_codes` array.
  - Workspace mutation tools (`workspace_set`, `workspace_put`, `workspace_remove`, `workspace_clear`, `workspace_set_options`): `file_count_before`, `file_count_after`, and — for `workspace_set_options` — the list of option keys changed.
  - `compile`: `container_id`, `container_size_bytes`, `task_count`, `program_count`, `include_bytes`, plus the analysis-tool fields above.
  - `run`: `container_id`, `duration_ms_requested`, `duration_ms_simulated` (how far the VM actually got), `fuel_consumed`, `trace_mode`, `trace_samples_emitted`, `truncated`, `terminated_reason`, `stimulus_count`, `task_count`.
  - `verify`: every field `run` logs, plus `expectation_count`, `failure_count`, and `passed`.
  - Resources: the resolved URI, the size in bytes of the serialized response, and — for `source/{file}` and `pou/{name}/*` — the template-parameter values that were bound.

**REQ-ARC-042** The server does **not** log source text, stimulus values, expectation values, trace variable values, or explanation bodies by default. These fields are replaced in the log with fixed-width content hashes (first 12 hex characters of a SHA-256 over the UTF-8 bytes) so that an analyst can detect "the agent sent the same source twice" or "the source changed between `check` calls" without the payload ever leaving the host. A `--log-level=debug` startup flag opts into logging full payloads for local debugging; this mode must print a warning to stderr at session start that payload logging is enabled.

**REQ-ARC-043** Logs are written to stderr by default, because the stdio transport uses stdout for the MCP JSON-RPC stream and any log output on stdout would corrupt the protocol. The `--log-file <path>` startup flag redirects logs to a file. The `--log-format` startup flag accepts `"json"` (one JSON object per line, the default) or `"text"` (human-readable, not intended for machine analysis).

**REQ-ARC-044** At session start the server emits a `session_start` event containing `session_id`, the server version, the effective resource limits (after applying command-line overrides), the `--project-dir` value if present, the file count of any pre-loaded workspace, and the startup `options` (with any secret-looking fields redacted). At session end the server emits a `session_end` event containing `session_id`, the total session wall-clock, the total number of tool calls and resource reads, per-tool call counts, and the reason for session termination (`"client_disconnect"`, `"signal"`, `"internal_error"`).

**REQ-ARC-045** The log stream is sufficient — without any payload fields — to answer at least: (1) the full ordered sequence of tool and resource calls in a session; (2) which calls used the session workspace vs. a per-call `sources` override; (3) which `check` calls returned which problem codes and whether a subsequent call presented a source with a different content hash; (4) which `verify` calls passed, and for failing ones, the `terminated_reason` and `failure_count`; (5) the distribution of `terminated_reason` values across `run` and `verify` calls in the session.

### Diagnostic Mapping

The existing `Diagnostic` type (from `ironplc-dsl`) carries file ID, source span (byte offsets), problem code, and message. The MCP server converts byte offsets to line/column numbers using the source text before serializing to JSON. This is the same conversion the LSP server already performs.

### Error Handling

Tool handlers return structured JSON responses in all cases — they never panic or return MCP-level errors for compiler failures. A parse error is a valid `check` response with `ok: false` and populated diagnostics, not an MCP error.

MCP-level errors (invalid tool name, malformed arguments) are handled by the SDK.

### Tool Descriptions Agents See

Every MCP tool is registered with a short description string that the client hands to the model. Those strings are the first line of defense against common agent failure modes: picking the wrong tool, escalating to a heavier tool to "try again", or toggling options to make errors disappear. These descriptions are part of the contract, not marketing text.

**REQ-ARC-050** The MCP tool descriptions shipped with the server must follow the guidance below. These are contractual, not flavor text; any change requires a design-note update in this document.

- `parse`: "Syntax check only. Use while drafting to confirm the source tokenizes and parses. Do NOT use this to validate a change — it does not catch type errors, undeclared symbols, or any other semantic rule. Call `check` for that."
- `check`: "Primary validator. Runs parse and full semantic analysis and returns structured diagnostics. ALWAYS run this before reporting success to the user and before calling `compile`, `run`, or `verify`. Self-heal by reading the returned diagnostics, fixing the code, and calling `check` again. Call `explain_diagnostic` to understand any unfamiliar problem code BEFORE editing the source."
- `format`: "Canonical re-rendering using the project's formatter. Safe to call on any parseable source. Does not mutate the session — pair with `workspace_put` to persist the formatted content."
- `symbols`: "Top-level declarations of programs, functions, function blocks, and types. Use the `pou` filter when you only care about one POU; calling this without the filter can return a large response for a real project."
- `list_options`: "Enumerates dialects and feature flags you are allowed to pass in `options`. Call this ONCE per session when you need to know which flags exist. Do NOT toggle flags or change dialect to make errors go away — the server emits a warning when you do, and the behavior is logged."
- `explain_diagnostic`: "Look up the human-readable explanation for a problem code (e.g. `P0042`). Call this before editing code in response to a diagnostic you do not fully understand."
- `get_resource`: "Fetches any `ironplc://` resource as a tool call. Prefer the targeted resource URIs (`pou/{name}/scope`, `project/io`) over the whole-project `symbols` tool when you only need context for one POU."
- `compile`: "Only call this when you need a compiled artifact to `run` or `verify`. For validation, call `check` instead — `check` is faster, produces the same diagnostics, and does not incur codegen cost. A failing `compile` does not give you any information that a failing `check` would not."
- `run`: "Simulates a compiled container in the VM for a caller-specified duration. Use this only after `check` passes. Drive inputs over time via `stimuli` and observe outputs via `variables`. The returned `trace` is bounded; use `trace.mode: \"final_only\"` or the `summary` object when you only care about outcomes."
- `verify`: "One-shot compile + run + assertion evaluation. Prefer this over a manual `compile`+`run` when you have concrete expectations. Failed expectations are returned structurally — react to `failures`, not to the raw trace."
- `workspace_*`: Each workspace tool description must explicitly state that it mutates the session workspace, and that subsequent tool/resource reads (without a per-call `sources` override) will see the change.

**REQ-ARC-051** Tool descriptions must NOT make claims the server cannot verify. In particular, tool descriptions may not promise things like "always faster than X" or "preferred for all use cases"; they must state concrete semantic differences and call out the cases where the wrong tool is tempting.

### Context Scoping

Context scoping is enforced structurally, not by prose recommendations to the agent. Three mechanisms work together to keep token usage lean:

**REQ-ARC-060** The `symbols` tool accepts an optional `pou: string` filter. When present, the response includes only the named POU and any types directly referenced by its variable declarations. Agents editing a single POU call `symbols` with the filter; the server does not need to return the full project symbol table.

**REQ-ARC-061** The `get_resource` tool description (see REQ-ARC-050) explicitly steers the agent toward the scoped resources (`pou/{name}/scope`, `pou/{name}/lineage`, `project/io`) rather than the whole-project `symbols` tool.

**REQ-ARC-062** Every tool and resource response carries a `response_size_bytes` field in the structured log (see REQ-ARC-041). An analyst reviewing the session log can trivially spot an agent that is pulling the entire project on every turn instead of scoped context, and use that evidence to refine tool descriptions or add further structural guards.

## Future Work

These items are intentionally out of scope for the initial milestone. They are listed here so the surface stays small and so implementers and reviewers know what has been considered and deliberately deferred.

- **Persistent test harness.** The `verify` tool is invocation-scoped: each call carries its own `stimuli` and `expectations`. A future milestone should add a conventional directory (e.g. `tests/`) in the session workspace and a `run_tests` tool that discovers and runs every saved scenario. The agent then curates tests alongside the source instead of re-inventing assertions on every turn.
- **Aggregate stimulus values for complex types.** The value encoding in REQ-TOL-043 covers reading arrays and structs in `trace[].variables`. Driving them from `stimuli.set` is also specified, but may need refinement for partial-update semantics (for example, setting a single array element by index, or setting one field of a struct without supplying the whole object). A follow-up should pin these down once real agent usage reveals what shapes are needed.
- **Streaming traces.** Long `run` calls currently return the complete trace in a single response. A future revision may offer incremental trace delivery via MCP's streaming facilities, which would let the agent react to intermediate values without extending `duration_ms` conservatively.
- **Workspace snapshots.** There is currently no way to checkpoint the session workspace and roll it back on a failed edit. A `workspace_snapshot` / `workspace_restore` pair would let an agent try a risky refactor and atomically revert.
- **Multi-project support.** The server holds a single session workspace per connection. Multi-project scenarios (shared library + application) can be expressed today by loading both into one session, but a proper multi-project layout with per-project dialects and import boundaries would require a different state model.
- **IEC 61131-3 Edition 3 dialect semantics.** The `dialect` preset exists but the precise set of Ed. 3 features supported by the MCP server should be documented once the compiler's Ed. 3 support is complete. A future revision of this doc should link directly to a compatibility matrix.
- **LSP parity for the in-memory analyzer.** The LSP server already converts byte offsets to line/column diagnostics (see Diagnostic Mapping). The MCP server borrows that code; once both mature, they should share a single implementation so that a diagnostic seen in an editor and a diagnostic seen by an agent are byte-identical.
