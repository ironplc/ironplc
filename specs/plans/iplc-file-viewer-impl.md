# Implementation Plan: IPLC File Viewer

**Design:** [IPLC File Viewer](../design/iplc-file-viewer.md)

## File Changes

| File | Change |
|------|--------|
| `compiler/plc2x/src/disassemble.rs` | **New** — Disassembler module |
| `compiler/plc2x/src/lsp.rs` | Add `ironplc/disassemble` request handler |
| `compiler/plc2x/src/lib.rs` | Export disassemble module |
| `compiler/plc2x/Cargo.toml` | Add `serde_json` dep if not present |
| `integrations/vscode/package.json` | Add customEditors, activation events |
| `integrations/vscode/src/iplcEditorProvider.ts` | **New** — Editor provider |
| `integrations/vscode/src/extension.ts` | Register editor provider |

## Testing

### Rust

- **Unit test:** Disassemble the existing `steel_thread.iplc` test fixture;
  verify JSON structure, opcode mnemonics, constant values, function metadata.
- **Round-trip test:** Build container with `ContainerBuilder`, serialize,
  disassemble, verify output matches expected structure.

### VS Code

- **Functional test:** Verify custom editor provider is registered for `.iplc`
  file extension.
- Webview rendering is validated manually (standard for VS Code custom editors).
