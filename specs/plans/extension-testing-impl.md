# Implementation Plan: VS Code Extension Testing

**Design:** [Extension Testing Architecture](../design/extension-testing-implementation.md)

## Phase 1: Extract and Unit Test Rendering Logic

### 1a. Create `src/iplcRendering.ts`

Extract the following from `iplcEditorProvider.ts` into a new module:

**Interfaces** (move as-is):
- `DisassemblyResult`
- `DisassemblyHeader`
- `DisassemblyConstant`
- `DisassemblyFunction`
- `DisassemblyInstruction`

**Functions** (move as-is, add `export`):
- `escapeHtml(text: string): string`
- `formatOffset(offset: number): string`
- `getOpcodeClass(opcode: string): string`

**Methods to extract as functions** (currently private methods on `IplcEditorProvider`):
- `getErrorHtml(message: string): string`
- `renderHeader(header: DisassemblyHeader): string`
- `renderConstants(constants: DisassemblyConstant[]): string`
- `renderFunctions(functions: DisassemblyFunction[]): string`
- `getDisassemblyHtml(data: DisassemblyResult): string`

**Shared HTML infrastructure:**

Both `getErrorHtml` and `getDisassemblyHtml` produce full HTML documents with overlapping CSS (body styles, VS Code CSS variables). Extract the shared CSS into a constant and create a helper:

```typescript
const BASE_STYLES = `
  body {
    background: var(--vscode-editor-background);
    color: var(--vscode-editor-foreground);
    font-family: var(--vscode-font-family);
    padding: 20px;
  }
`;

export function wrapInHtmlDocument(body: string, extraStyles: string = ''): string {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <style>${BASE_STYLES}${extraStyles}</style>
</head>
<body>
  ${body}
</body>
</html>`;
}
```

`getErrorHtml` and `getDisassemblyHtml` then call `wrapInHtmlDocument` with their specific body content and any additional styles, eliminating the duplicated HTML boilerplate.

After extraction, `iplcEditorProvider.ts` imports and calls these functions. The class retains only:
- `register()`, `openCustomDocument()`, `resolveCustomEditor()` — VS Code API integration
- `waitForClient()` — LSP lifecycle management

### 1b. Create Unit Test Infrastructure

Create `src/test/unit/` with a standalone mocha runner that does NOT use `@vscode/test-electron`.

**Files to create:**

```
src/test/unit/
├── iplcRendering.test.ts    # Tests for rendering functions
└── testHelpers.ts           # Factory functions for test data
```

**Test runner configuration:**

Add to `package.json`:
```json
"test:unit": "c8 --check-coverage --lines 80 --include 'out/iplcRendering.js' --include 'out/compilerDiscovery.js' mocha 'out/test/unit/**/*.test.js'"
```

The `--include` flags restrict coverage measurement to the extracted unit-testable modules. Without them, `c8` would measure coverage of all loaded files (including test helpers and test files themselves), producing misleading numbers.

The `compile` step already compiles all TypeScript under `src/` to `out/` (tsconfig has `"outDir": "out"` and `"rootDir": "src"`), so the unit tests will be compiled alongside everything else and the `out/test/unit/` glob will find them.

**Test helpers (`testHelpers.ts`):**

Factory functions that return valid default objects, allowing individual tests to override specific fields:

```typescript
export function createTestHeader(overrides?: Partial<DisassemblyHeader>): DisassemblyHeader {
  return {
    formatVersion: 1,
    flags: { hasContentSignature: false, hasDebugSection: false, hasTypeSection: false },
    maxStackDepth: 8,
    maxCallDepth: 2,
    numVariables: 3,
    numFbInstances: 0,
    numFunctions: 1,
    numFbTypes: 0,
    numArrays: 0,
    entryFunctionId: 0,
    inputImageBytes: 4,
    outputImageBytes: 4,
    memoryImageBytes: 0,
    contentHash: 'abc123',
    sourceHash: 'def456',
    ...overrides,
  };
}

export function createTestInstruction(overrides?: Partial<DisassemblyInstruction>): DisassemblyInstruction {
  return {
    offset: 0,
    opcode: 'LOAD_INT',
    operands: '42',
    comment: '',
    ...overrides,
  };
}

export function createTestFunction(overrides?: Partial<DisassemblyFunction>): DisassemblyFunction {
  return {
    id: 0,
    maxStackDepth: 4,
    numLocals: 2,
    bytecodeLength: 16,
    instructions: [createTestInstruction()],
    ...overrides,
  };
}

export function createTestDisassemblyResult(overrides?: Partial<DisassemblyResult>): DisassemblyResult {
  return {
    header: createTestHeader(),
    constants: [{ index: 0, type: 'INT', value: '42' }],
    functions: [createTestFunction()],
    ...overrides,
  };
}
```

### 1c. Unit Test Cases for `iplcRendering.ts`

**`escapeHtml`:**
- `escapeHtml_when_no_special_chars_then_unchanged`
- `escapeHtml_when_all_special_chars_then_all_escaped` — input contains `&`, `<`, `>`, and `"` together; verify each is escaped in the output
- `escapeHtml_when_empty_string_then_returns_empty`

**`formatOffset`:**
- `formatOffset_when_zero_then_returns_0x0000`
- `formatOffset_when_small_value_then_zero_padded`
- `formatOffset_when_large_value_then_hex_formatted`

**`getOpcodeClass`:**

Use a data-driven approach to reduce test count while maintaining full branch coverage:
- `getOpcodeClass_when_load_prefix_then_op_load`
- `getOpcodeClass_when_store_prefix_then_op_store`
- `getOpcodeClass_when_arithmetic_prefix_then_op_arith` — test one representative (e.g. `ADD_INT`); the four prefixes (`ADD`, `SUB`, `MUL`, `DIV`) share a single code path
- `getOpcodeClass_when_control_flow_prefix_then_op_ctrl` — test one representative (e.g. `JMP`); the four prefixes (`RET`, `CALL`, `JMP`, `BR`) share a single code path
- `getOpcodeClass_when_unknown_prefix_then_op_unknown`
- `getOpcodeClass_when_unrecognized_prefix_then_empty_string`

**`getErrorHtml`:**
- `getErrorHtml_when_plain_message_then_contains_message`
- `getErrorHtml_when_html_in_message_then_escaped`
- `getErrorHtml_when_called_then_returns_valid_html_document`

**`renderHeader`:**
- `renderHeader_when_null_then_returns_empty`
- `renderHeader_when_valid_header_then_contains_all_fields`
- `renderHeader_when_no_flags_set_then_displays_none`
- `renderHeader_when_all_flags_set_then_displays_all_flag_names`
- `renderHeader_when_hash_values_then_html_escaped`

**`renderConstants`:**
- `renderConstants_when_null_then_shows_empty`
- `renderConstants_when_empty_array_then_shows_empty`
- `renderConstants_when_has_constants_then_shows_count_and_values`
- `renderConstants_when_special_chars_in_value_then_escaped`

**`renderFunctions`:**
- `renderFunctions_when_null_then_shows_none`
- `renderFunctions_when_empty_array_then_shows_none`
- `renderFunctions_when_has_function_then_shows_metadata_and_instructions`
- `renderFunctions_when_instruction_has_comment_then_shows_comment`
- `renderFunctions_when_instruction_has_no_comment_then_no_comment_span`

**`getDisassemblyHtml`:**
- `getDisassemblyHtml_when_error_set_then_returns_error_html`
- `getDisassemblyHtml_when_valid_data_then_contains_header_constants_functions`
- `getDisassemblyHtml_when_valid_data_then_returns_valid_html_document`

## Phase 2: Extract and Unit Test Compiler Discovery

### 2a. Create `src/compilerDiscovery.ts`

Extract the `findCompiler` function from `extension.ts` into a module with injectable dependencies:

```typescript
export interface CompilerEnvironment {
  platform: string;
  existsSync: (path: string) => boolean;
  getEnv: (name: string) => string | undefined;
  getConfig: (key: string) => string | undefined;
}

export interface CompilerDiscoveryResult {
  path: string;
  source: string;
}

export function findCompilerPath(env: CompilerEnvironment): CompilerDiscoveryResult | undefined {
  // Same logic as current findCompiler, using env instead of globals
}
```

`extension.ts` creates a `CompilerEnvironment` from real `process`, `fs`, and `vscode.workspace.getConfiguration` and calls `findCompilerPath`.

### 2b. Unit Test Cases for `compilerDiscovery.ts`

- `findCompilerPath_when_config_path_exists_then_returns_config_path`
- `findCompilerPath_when_config_path_missing_then_tries_env`
- `findCompilerPath_when_env_var_exists_then_returns_env_path`
- `findCompilerPath_when_env_var_missing_then_tries_platform_paths`
- `findCompilerPath_when_darwin_and_homebrew_exists_then_returns_homebrew_path`
- `findCompilerPath_when_win32_and_localappdata_exists_then_returns_windows_path`
- `findCompilerPath_when_nothing_found_then_returns_undefined`
- `findCompilerPath_when_win32_then_uses_exe_extension`
- `findCompilerPath_when_linux_then_no_exe_extension`

## Phase 3: Mock-Based Tests for Editor Provider

### 3a. Define `LanguageClientLike` Interface

Create a minimal interface in `iplcEditorProvider.ts` for the LSP client methods actually used:

```typescript
export interface LanguageClientLike {
  isRunning(): boolean;
  sendRequest(method: string, params: any): Promise<any>;
  onDidChangeState(listener: (e: { newState: number }) => void): { dispose(): void };
}
```

Change the `IplcEditorProvider` constructor to accept `LanguageClientLike` instead of `LanguageClient`. The real `LanguageClient` already satisfies this interface, so no changes needed at the call site.

Export the numeric values of `State.Running` (2) and `State.Stopped` (1) as constants from the test helpers so that mock implementations in unit tests don't need to import `vscode-languageclient`:

```typescript
// In testHelpers.ts — values match vscode-languageclient's State enum
export const CLIENT_STATE_STOPPED = 1;
export const CLIENT_STATE_RUNNING = 2;
```

### 3b. Unit Test Cases for `resolveCustomEditor`

These tests use mock implementations of `LanguageClientLike` and a mock `WebviewPanel`. The mock panel must support property assignment on `webview.options` since `resolveCustomEditor` sets `enableScripts: false`:

```typescript
function createMockPanel(): { webview: { html: string; options: any } } {
  return { webview: { html: '', options: {} } };
}
```

Test cases:
- `resolveCustomEditor_when_client_running_and_request_succeeds_then_renders_disassembly`
- `resolveCustomEditor_when_client_running_and_request_fails_then_renders_error`
- `resolveCustomEditor_when_client_running_and_result_has_error_then_renders_error`
- `resolveCustomEditor_when_client_not_running_and_starts_within_timeout_then_renders_disassembly`
- `resolveCustomEditor_when_client_not_running_and_times_out_then_renders_compiler_not_found`
- `resolveCustomEditor_when_client_not_running_and_stops_then_renders_compiler_not_found`

### 3c. `waitForClient` Test Cases

If `waitForClient` is made testable (by accepting a client-like interface):

- `waitForClient_when_already_running_then_resolves_true_immediately`
- `waitForClient_when_starts_before_timeout_then_resolves_true`
- `waitForClient_when_stops_before_timeout_then_resolves_false`
- `waitForClient_when_timeout_expires_then_resolves_false`

## Phase 4: Expand Functional Tests

### 4a. Add Test Resources

Create minimal test fixture files:
- `src/test/functional/resources/valid.TcPOU` — minimal TwinCAT POU file
- `src/test/functional/resources/valid.TcGVL` — minimal TwinCAT GVL file
- `src/test/functional/resources/valid.TcDUT` — minimal TwinCAT DUT file

### 4b. New Functional Test Cases

Add to `extension.test.ts`:

- `detects_TcPOU_extension_as_twincat_pou`
- `detects_TcGVL_extension_as_twincat_gvl`
- `detects_TcDUT_extension_as_twincat_dut`

These are simple tests following the same pattern as the existing `.st` detection test. They satisfy the structural invariant from the requirements spec.

## Phase 5: Create Invariant Check Script

### 5a. Create `src/test/checkInvariants.ts`

A script that reads `package.json` and verifies that all declared capabilities appear in test files:

```typescript
import * as fs from 'fs';
import * as path from 'path';

const packageJson = JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'package.json'), 'utf-8'));

// Exceptions: capabilities that are intentionally not tested, with justification.
// Each key is the capability ID; the value is the reason it is excluded.
const EXCEPTIONS: Record<string, string> = {
  'plcopen-xml': 'Uses firstLine detection (no file extension); requires XML content matching which is not reliably testable via openTextDocument',
};

// Collect all test file contents
const testFiles = findTestFiles(path.join(__dirname));
const testContent = testFiles.map(f => fs.readFileSync(f, 'utf-8')).join('\n');

let failures: string[] = [];

// Check languages with extensions
for (const lang of packageJson.contributes.languages) {
  if (EXCEPTIONS[lang.id]) {
    continue;
  }
  if (lang.extensions && lang.extensions.length > 0) {
    if (!testContent.includes(lang.id)) {
      failures.push(`Language '${lang.id}' has no test reference`);
    }
  }
}

// Check commands
for (const cmd of packageJson.contributes.commands) {
  if (EXCEPTIONS[cmd.command]) {
    continue;
  }
  if (!testContent.includes(cmd.command)) {
    failures.push(`Command '${cmd.command}' has no test reference`);
  }
}

// Check custom editors
for (const editor of packageJson.contributes.customEditors) {
  if (EXCEPTIONS[editor.viewType]) {
    continue;
  }
  if (!testContent.includes(editor.viewType)) {
    failures.push(`Custom editor '${editor.viewType}' has no test reference`);
  }
}

// Report exceptions for visibility
const usedExceptions = Object.entries(EXCEPTIONS);
if (usedExceptions.length > 0) {
  console.log('Exceptions (intentionally untested):');
  usedExceptions.forEach(([id, reason]) => console.log(`  - ${id}: ${reason}`));
}

if (failures.length > 0) {
  console.error('Test coverage invariant failures:');
  failures.forEach(f => console.error(`  - ${f}`));
  process.exit(1);
} else {
  console.log('All test coverage invariants satisfied.');
}
```

### 5b. Justfile Integration

```justfile
ci:
  just compile
  just lint
  just check-invariants
  just test-grammar
  just test-unit
  just test

check-invariants:
  node out/test/checkInvariants.js

test-unit:
  npm run test:unit
```
