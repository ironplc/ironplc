# Program Run Button for VS Code

## Goal

Add a "Run Program" code lens above `PROGRAM` declarations in VS Code, with a
status bar toolbar for pause/stop during execution. This brings the playground's
run experience into the IDE.

## Architecture

The LSP server already implements `ironplc/run`, `ironplc/step`, and
`ironplc/stop` custom requests. This plan adds the VS Code frontend wiring:

1. **CodeLens provider** - Detects `PROGRAM` keywords via regex on the document
   text and shows a "Run Program" lens above each one.
2. **Run command** (`ironplc.runProgram`) - Sends `ironplc/run` with the full
   document text, then starts a periodic `ironplc/step` interval (100ms).
3. **Status bar controls** - Pause and Stop buttons appear while running.
4. **Output panel** - Variable values displayed in a VS Code output channel,
   updated on a render interval (500ms).
5. **Run session manager** - TypeScript class encapsulating the run lifecycle
   (start/pause/resume/stop) and LSP communication, unit-testable with a mock
   client.

### Key Design Decisions

- **CodeLens via regex** (not LSP server-side): Simpler, avoids adding a new
  LSP capability. The regex `^\s*PROGRAM\s+(\w+)` is sufficient.
- **Freewheeling default**: If no task configuration exists, use a default cycle
  time of 100ms (100,000 us), matching the playground.
- **Reuse `LanguageClientLike` interface**: The existing mock pattern from
  `iplcEditorLogic.ts` enables unit testing of the run session manager.

## File Map

### New files
- `integrations/vscode/src/runSession.ts` - Run session manager (start/step/stop lifecycle)
- `integrations/vscode/src/runCodeLensProvider.ts` - CodeLens provider for PROGRAM
- `integrations/vscode/src/test/unit/runSession.test.ts` - Unit tests for run session
- `integrations/vscode/src/test/unit/runCodeLensProvider.test.ts` - Unit tests for code lens

### Modified files
- `integrations/vscode/package.json` - Add `ironplc.runProgram` and `ironplc.stopProgram` commands
- `integrations/vscode/src/extension.ts` - Register code lens provider, commands, and status bar items

## Tasks

- [ ] Write run session manager (`runSession.ts`) with start/pause/resume/stop
- [ ] Write code lens provider (`runCodeLensProvider.ts`)
- [ ] Register commands, code lens provider, and status bar in `extension.ts`
- [ ] Add commands to `package.json`
- [ ] Write unit tests for run session manager
- [ ] Write unit tests for code lens provider
- [ ] Add functional test reference for new command (satisfies invariant checker)
- [ ] Verify extension builds and all tests pass
