# IPLC File Viewer for VS Code

## Summary

Add a read-only custom editor to the IronPLC VS Code extension that displays
the contents of `.iplc` binary files as structured, human-readable disassembly.
Binary parsing happens in the Rust compiler (`ironplcc`) via a custom LSP
request; the VS Code extension renders the result in a webview.

## Motivation

IPLC files are binary bytecode containers produced by the compiler. They are
opaque to developers, making it difficult to debug code generation or understand
what the compiler produced. A built-in viewer provides immediate feedback and
lays the foundation for a future source-level debugger.

## Architecture

```
  .iplc file on disk
       │
       ▼
  ironplcc LSP server
  (custom request: ironplc/disassemble)
       │
       ▼
  Disassembler module (Rust)
  reads container, decodes opcodes
       │
       ▼
  Structured JSON response
       │
       ▼
  VS Code CustomReadonlyEditorProvider
       │
       ▼
  Webview with collapsible sections
  (header, constants, functions + opcodes)
```

### Data Flow

1. User opens an `.iplc` file in VS Code.
2. VS Code activates the `ironplc.iplcViewer` custom editor.
3. The editor provider sends an `ironplc/disassemble` request to the LSP client
   with the file URI.
4. The LSP server reads the binary, runs the disassembler, returns JSON.
5. The webview renders the JSON as a structured, themed view.

## Rust Side

### Disassembler Module

New module `compiler/plc2x/src/disassemble.rs` in the `plc2x` crate.

**Responsibilities:**
- Read an `.iplc` file from a path using the `ironplc-container` crate.
- Decode the file header into a structured representation.
- Decode the constant pool entries with their types and values.
- Walk each function's bytecode, matching opcode bytes to mnemonics and
  extracting operands.
- Return a `serde_json::Value` (or a typed struct) representing the full
  disassembly.

**Opcode decoding:** A match on the opcode byte maps to a mnemonic string.
Unknown opcodes render as `UNKNOWN(0xNN)`. The operand decoding uses the
instruction encoding rules (e.g., opcodes with a u16 operand consume 2
additional bytes in little-endian order).

**Constant cross-referencing:** When a `LOAD_CONST_*` instruction references a
pool index, the disassembler adds a comment showing the constant's value
(e.g., `pool[0]  // = 10`).

### Custom LSP Request

In `compiler/plc2x/src/lsp.rs`, add a handler for `ironplc/disassemble`:

- **Method:** `ironplc/disassemble`
- **Params:** `{ "uri": "file:///path/to/file.iplc" }`
- **Result:** JSON disassembly structure (see below) or error

Follows the existing `cast_request`/`send_response` pattern used by
`SemanticTokensFullRequest` and `DocumentSymbolRequest`.

### JSON Response Structure

```json
{
  "header": {
    "formatVersion": 1,
    "flags": {
      "hasContentSignature": false,
      "hasDebugSection": false,
      "hasTypeSection": false
    },
    "maxStackDepth": 2,
    "maxCallDepth": 0,
    "numVariables": 2,
    "numFbInstances": 0,
    "numFunctions": 1,
    "numFbTypes": 0,
    "numArrays": 0,
    "entryFunctionId": 0,
    "inputImageBytes": 0,
    "outputImageBytes": 0,
    "memoryImageBytes": 0,
    "contentHash": "0000...0000",
    "sourceHash": "0000...0000",
    "sections": {
      "signature": { "offset": 0, "size": 0 },
      "debugSignature": { "offset": 0, "size": 0 },
      "type": { "offset": 0, "size": 0 },
      "constantPool": { "offset": 256, "size": 20 },
      "code": { "offset": 276, "size": 33 },
      "debug": { "offset": 0, "size": 0 }
    }
  },
  "constants": [
    { "index": 0, "type": "I32", "value": "10" },
    { "index": 1, "type": "I32", "value": "32" }
  ],
  "functions": [
    {
      "id": 0,
      "maxStackDepth": 2,
      "numLocals": 2,
      "bytecodeLength": 19,
      "instructions": [
        { "offset": 0, "opcode": "LOAD_CONST_I32", "operands": "pool[0]", "comment": "= 10" },
        { "offset": 3, "opcode": "STORE_VAR_I32", "operands": "var[0]", "comment": "" },
        { "offset": 6, "opcode": "LOAD_VAR_I32", "operands": "var[0]", "comment": "" },
        { "offset": 9, "opcode": "LOAD_CONST_I32", "operands": "pool[1]", "comment": "= 32" },
        { "offset": 12, "opcode": "ADD_I32", "operands": "", "comment": "" },
        { "offset": 13, "opcode": "STORE_VAR_I32", "operands": "var[1]", "comment": "" },
        { "offset": 16, "opcode": "RET_VOID", "operands": "", "comment": "" }
      ]
    }
  ]
}
```

## VS Code Side

### Custom Editor Registration

In `package.json`, add a `customEditors` contribution:

```json
{
  "customEditors": [
    {
      "viewType": "ironplc.iplcViewer",
      "displayName": "IPLC Bytecode Viewer",
      "selector": [{ "filenamePattern": "*.iplc" }],
      "priority": "default"
    }
  ]
}
```

Add `onCustomEditor:ironplc.iplcViewer` to activation events.

### Editor Provider

New file `integrations/vscode/src/iplcEditorProvider.ts`:

- Implements `vscode.CustomReadonlyEditorProvider`.
- `openCustomDocument()`: Stores the file URI.
- `resolveCustomEditor()`: Sends `ironplc/disassemble` to the LSP client,
  receives JSON, generates webview HTML.

### Webview Layout

The webview renders three collapsible sections:

1. **File Header** — Table of metadata fields (version, flags, resource budget,
   section offsets/sizes, hashes displayed as truncated hex).
2. **Constant Pool** — Table with columns: Index, Type, Value.
3. **Functions** — One collapsible panel per function:
   - Metadata: ID, max stack depth, num locals, bytecode length.
   - Instruction table: Offset (hex) | Opcode | Operands | Comment.

**Styling:**
- Uses VS Code CSS variables (`--vscode-editor-background`,
  `--vscode-editor-foreground`, `--vscode-textLink-foreground`, etc.) for
  automatic theme integration.
- Monospace font for instruction tables.
- Opcode syntax coloring by category (load/store, arithmetic, control flow).

**No external dependencies.** HTML is generated from template literals in
TypeScript. No React or other framework.

### Extension Integration

In `extension.ts`:
- Import and register the `IplcEditorProvider` in the `activate()` function.
- Pass the `LanguageClient` instance to the provider so it can send custom
  requests.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Invalid/corrupt `.iplc` file | Disassembler returns error JSON; webview shows error message |
| Unknown opcode byte | Displayed as `UNKNOWN(0xNN)` with raw hex |
| LSP not connected | Webview shows "IronPLC compiler not found" message |
| File read failure | LSP returns error response; webview shows file error |

## Future Evolution

This viewer is the foundation for a source-level debugger. Future work:

- **Debug section rendering:** When the debug section is present, show embedded
  source text and line mappings alongside bytecode.
- **Side-by-side view:** Structured text source on the left, corresponding
  bytecode on the right, with linked highlighting.
- **Step debugging:** Integrate with VS Code's Debug Adapter Protocol to
  highlight the current instruction during execution.

## Implementation Plan

See [Implementation Plan: IPLC File Viewer](../plans/iplc-file-viewer-impl.md) for the file changes and testing plan.
