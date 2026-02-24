# VS Code Extension Testing

This document defines the testing strategy for the IronPLC VS Code extension. The goal is to eliminate reliance on manual testing by establishing automated test coverage at multiple levels.

## Problem Statement

The VS Code extension is growing in complexity (custom editors, LSP integration, compiler discovery) but the automated test coverage has not kept pace. The existing tests cover:

- **Grammar snapshot tests**: Syntax highlighting via `vscode-tmgrammar-snap` (good)
- **3 functional tests**: Language ID detection for `.st` files (minimal)
- **Windows-only end-to-end smoke test**: Checks that a log file exists after 30 seconds (brittle, low signal)

What is NOT tested:

- The `IplcEditorProvider` (custom `.iplc` bytecode viewer) - zero coverage
- The `findCompiler` discovery logic - zero coverage
- All error paths (compiler not found, LSP timeout, disassembly failure)
- HTML rendering correctness
- Pure utility functions (`escapeHtml`, `formatOffset`, `getOpcodeClass`)

A recent defect where the `.iplc` viewer displayed "compiler not found" instead of rendering content was only caught by manual testing. This is unacceptable for a solo maintainer.

## Architecture Constraint

The extension has a structural problem that limits testability: pure logic (HTML rendering, opcode classification, offset formatting) is interleaved with VS Code API calls and LSP client interactions inside class methods. Functions are not exported. This means testing anything requires running inside a full VS Code instance, which is slow, flaky, and makes it impossible to test edge cases.

The testing strategy below addresses this in two phases: first extract and test pure logic (high value, low risk), then add mock-based tests for integration points.

## Testing Tiers

### Tier 1: Pure Unit Tests

**What**: Extract pure functions into a separate module and test them with mocha directly (no VS Code instance needed).

**Why**: These tests are fast (milliseconds), reliable (no external dependencies), and can cover edge cases exhaustively. This tier alone would have caught the recent `.iplc` rendering defect.

**Functions to extract and test**:

From `iplcEditorProvider.ts`:
- `escapeHtml(text: string): string`
- `formatOffset(offset: number): string`
- `getOpcodeClass(opcode: string): string`
- `getErrorHtml(message: string): string` (extract as a standalone function)
- `renderHeader(header: DisassemblyHeader): string`
- `renderConstants(constants: DisassemblyConstant[]): string`
- `renderFunctions(functions: DisassemblyFunction[]): string`
- `getDisassemblyHtml(data: DisassemblyResult): string`

**Implementation approach**:

1. Create `src/iplcRendering.ts` containing the extracted pure functions and the TypeScript interfaces (`DisassemblyResult`, `DisassemblyHeader`, etc.)
2. Have `iplcEditorProvider.ts` import from `iplcRendering.ts`
3. Create `src/test/unit/` directory for tests that run without VS Code
4. Add a separate mocha test runner for unit tests (does not use `@vscode/test-electron`)
5. Add `test:unit` npm script and corresponding justfile target

**Example test cases**:

```typescript
// iplcRendering.test.ts

suite('escapeHtml', () => {
  test('escapeHtml_when_ampersand_then_escaped', () => {
    assert.strictEqual(escapeHtml('a&b'), 'a&amp;b');
  });

  test('escapeHtml_when_angle_brackets_then_escaped', () => {
    assert.strictEqual(escapeHtml('<script>'), '&lt;script&gt;');
  });
});

suite('getOpcodeClass', () => {
  test('getOpcodeClass_when_load_opcode_then_returns_op_load', () => {
    assert.strictEqual(getOpcodeClass('LOAD_INT'), 'op-load');
  });

  test('getOpcodeClass_when_unknown_opcode_then_returns_op_unknown', () => {
    assert.strictEqual(getOpcodeClass('UNKNOWN_42'), 'op-unknown');
  });

  test('getOpcodeClass_when_unrecognized_opcode_then_returns_empty', () => {
    assert.strictEqual(getOpcodeClass('NOP'), '');
  });
});

suite('formatOffset', () => {
  test('formatOffset_when_zero_then_returns_0x0000', () => {
    assert.strictEqual(formatOffset(0), '0x0000');
  });

  test('formatOffset_when_large_value_then_formatted', () => {
    assert.strictEqual(formatOffset(255), '0x00FF');
  });
});

suite('renderHeader', () => {
  test('renderHeader_when_null_header_then_returns_empty', () => {
    assert.strictEqual(renderHeader(null as any), '');
  });

  test('renderHeader_when_valid_header_then_contains_format_version', () => {
    const header = createTestHeader({ formatVersion: 3 });
    const html = renderHeader(header);
    assert.ok(html.includes('3'));
    assert.ok(html.includes('Format Version'));
  });

  test('renderHeader_when_flags_set_then_displays_flag_names', () => {
    const header = createTestHeader({
      flags: { hasContentSignature: true, hasDebugSection: false, hasTypeSection: true }
    });
    const html = renderHeader(header);
    assert.ok(html.includes('Content Signature'));
    assert.ok(html.includes('Type Section'));
    assert.ok(!html.includes('Debug Section'));
  });
});

suite('getDisassemblyHtml', () => {
  test('getDisassemblyHtml_when_error_in_result_then_shows_error', () => {
    const result: DisassemblyResult = {
      error: 'something broke',
      header: null as any,
      constants: [],
      functions: [],
    };
    const html = getDisassemblyHtml(result);
    assert.ok(html.includes('something broke'));
    assert.ok(html.includes('error'));
  });

  test('getDisassemblyHtml_when_valid_result_then_shows_viewer', () => {
    const result = createTestDisassemblyResult();
    const html = getDisassemblyHtml(result);
    assert.ok(html.includes('IPLC Bytecode Viewer'));
    assert.ok(html.includes('File Header'));
  });
});

suite('getErrorHtml', () => {
  test('getErrorHtml_when_message_with_html_then_escaped', () => {
    const html = getErrorHtml('<script>alert("xss")</script>');
    assert.ok(!html.includes('<script>'));
    assert.ok(html.includes('&lt;script&gt;'));
  });
});
```

**Test helpers**: Create a `src/test/unit/testHelpers.ts` with factory functions like `createTestHeader()` and `createTestDisassemblyResult()` that return valid default objects which individual tests can override. This avoids large object literals in every test.

### Tier 2: Mock-Based Integration Tests

**What**: Test components that depend on VS Code APIs or the LSP client by using mocks/stubs, still without launching a full VS Code instance.

**Why**: Covers the integration logic (compiler discovery, client lifecycle, editor provider state machine) that pure unit tests cannot reach, but without the overhead and flakiness of a real VS Code instance.

**Key components to test**:

#### 2a. `findCompiler` logic

Extract `findCompiler` into a module where filesystem checks and environment access are injectable:

```typescript
// compilerDiscovery.ts
export interface CompilerEnvironment {
  platform: string;
  existsSync: (path: string) => boolean;
  getEnv: (name: string) => string | undefined;
  getConfig: (key: string) => string | undefined;
}

export function findCompilerPath(env: CompilerEnvironment): string | undefined {
  // Same logic as current findCompiler, but using env instead of globals
}
```

Test cases:
- `findCompilerPath_when_config_path_set_and_exists_then_returns_config_path`
- `findCompilerPath_when_env_var_set_and_exists_then_returns_env_path`
- `findCompilerPath_when_macos_homebrew_exists_then_returns_homebrew_path`
- `findCompilerPath_when_windows_localappdata_exists_then_returns_windows_path`
- `findCompilerPath_when_nothing_found_then_returns_undefined`
- `findCompilerPath_when_config_path_set_but_missing_then_tries_next`

#### 2b. `IplcEditorProvider.resolveCustomEditor` state machine

The `resolveCustomEditor` method has several distinct paths:
1. Client already running -> send request -> render result
2. Client not running -> wait -> client starts -> send request -> render result
3. Client not running -> wait -> timeout -> show error
4. Client not running -> wait -> client stops -> show error
5. Client running -> request fails -> show error
6. Client running -> request returns error in result -> show error

Each of these paths should be tested with a mock `LanguageClient` that simulates the relevant state transitions:

```typescript
suite('resolveCustomEditor', () => {
  test('resolveCustomEditor_when_client_running_then_sends_disassemble_request', async () => {
    const mockClient = createMockClient({ running: true, response: validDisassemblyResult });
    const mockPanel = createMockWebviewPanel();
    const provider = new IplcEditorProvider(mockClient);

    await provider.resolveCustomEditor({ uri: testUri, dispose: () => {} }, mockPanel);

    assert.ok(mockPanel.webview.html.includes('IPLC Bytecode Viewer'));
  });

  test('resolveCustomEditor_when_client_not_running_and_times_out_then_shows_error', async () => {
    const mockClient = createMockClient({ running: false, neverStarts: true });
    const mockPanel = createMockWebviewPanel();
    const provider = new IplcEditorProvider(mockClient);

    await provider.resolveCustomEditor({ uri: testUri, dispose: () => {} }, mockPanel);

    assert.ok(mockPanel.webview.html.includes('IronPLC compiler not found'));
  });
});
```

**Implementation note**: The `IplcEditorProvider` constructor takes a `LanguageClient`. To make it mockable, define a minimal interface for the methods it actually uses (`isRunning`, `sendRequest`, `onDidChangeState`) and type the constructor parameter as that interface instead of the concrete class. This is a minor refactor.

### Tier 3: VS Code Functional Tests (Expand Existing)

**What**: Expand the existing `@vscode/test-electron` test suite to cover more user-visible behaviors.

**Why**: These tests run in a real VS Code instance and verify that the extension integrates correctly with the VS Code API (commands registered, editors open, error messages appear).

**New test cases to add**:

```
- createNewStructuredTextFile_when_executed_then_opens_editor (existing)
- detects_ST_extension_as_61131_3_st (existing)
- does_not_detect_non_ST_extension_as_61131_3_st (existing)
- extension_when_activated_then_registers_iplcViewer_custom_editor
- extension_when_activated_then_registers_createNewStructuredTextFile_command
- openIplcFile_when_no_compiler_then_shows_error_in_editor (needs test .iplc file)
```

**Limitations**: These tests cannot easily control whether the compiler binary exists. They primarily verify command registration and VS Code API integration. The heavy lifting for error paths belongs in Tier 2.

**Test naming**: Follow BDD convention: `function_when_condition_then_result`.

### Tier 4: End-to-End Smoke Test (Improve Existing)

**What**: Improve the existing Windows-only smoke test to be more robust and informative.

**Why**: The current test only checks for log file existence. It has no visibility into whether the extension rendered correctly, showed errors, or actually processed files.

**Improvements**:

1. **Check log file content, not just existence**: After waiting for VS Code to start, read the log file and verify it contains expected LSP initialization messages (e.g., "initialized", no error-level entries).

2. **Reduce flakiness**: The 30-second fixed sleep is a race condition. Instead, poll for the log file with a timeout (check every 2 seconds for up to 60 seconds).

3. **Document the test's limitations**: The end-to-end test is a last-resort safety net. It runs late in the release pipeline, on one platform, and gives minimal diagnostic information when it fails. The real coverage must come from Tiers 1-3.

**Not recommended for now**:
- Cross-platform end-to-end tests (the ROI is low; Tiers 1-2 are platform-independent)
- Browser-based VS Code testing (VS Code web is a different runtime with different issues)
- Screenshot comparison (brittle, hard to maintain)

## Implementation Order

Implement in this order to maximize value delivered per unit of effort:

### Phase 1: Tier 1 (Pure Unit Tests)

This is the highest-value change. It requires:

1. Extract pure functions and interfaces from `iplcEditorProvider.ts` into `src/iplcRendering.ts`
2. Create `src/test/unit/` directory with a standalone mocha runner (no `@vscode/test-electron`)
3. Write tests for all rendering and utility functions
4. Add `test:unit` npm script
5. Add `test-unit` justfile target
6. Add `test:unit` to the `ci` justfile target (runs before functional tests)
7. Verify the CI workflow picks this up through the existing `just ci` call

**Estimated scope**: ~150-200 lines of new test code, ~30 lines of refactoring in `iplcEditorProvider.ts`.

### Phase 2: Tier 2 (Mock-Based Tests)

1. Define a `LanguageClientLike` interface for the methods `IplcEditorProvider` uses
2. Extract `findCompiler` into `src/compilerDiscovery.ts` with injectable dependencies
3. Write mock-based tests for both components
4. Add to the `test:unit` runner (these don't need VS Code either)

**Estimated scope**: ~200 lines of new test code, ~50 lines of refactoring.

### Phase 3: Tier 3 (Expand Functional Tests)

1. Add `.iplc` test resource file
2. Add functional tests for custom editor registration
3. Add functional test for missing-compiler error display

**Estimated scope**: ~50 lines of new test code.

### Phase 4: Tier 4 (Improve Smoke Test)

1. Replace `Start-Sleep -s 30` with polling loop
2. Add log content verification
3. Add comments documenting limitations

**Estimated scope**: ~20 lines changed in the justfile.

## Test Runner Configuration

### Unit tests (Tier 1 and Tier 2)

These tests must NOT use `@vscode/test-electron`. They run with plain mocha:

```json
// In package.json scripts
"test:unit": "mocha --require ts-node/register 'src/test/unit/**/*.test.ts'"
```

Or, if using the compiled output:

```json
"test:unit": "mocha 'out/test/unit/**/*.test.js'"
```

The `pretest:unit` script should compile first:

```json
"pretest:unit": "npm run compile"
```

### Justfile integration

```justfile
# Run unit tests (fast, no VS Code needed).
test-unit:
  npm run test:unit

# The CI target runs unit tests before functional tests.
ci:
  just compile
  just lint
  just test-grammar
  just test-unit
  just test
```

Unit tests run before functional tests because they are faster and catch problems earlier.

## Coverage

There is no formal coverage threshold for the VS Code extension today (unlike the compiler's 85% requirement). Introducing one is not part of this spec, but the Tier 1 and Tier 2 tests should aim for complete branch coverage of the extracted modules.

## What This Does NOT Cover

- **Compiler-side LSP tests**: The compiler's handling of `ironplc/disassemble` requests is tested in the Rust codebase under the compiler's own coverage requirements.
- **VS Code API correctness**: We trust that `vscode.window.registerCustomEditorProvider` works. We test that we call it correctly.
- **Visual appearance**: We do not test that the HTML looks correct visually. We test that it contains the right data and structure.
- **Cross-browser/cross-platform rendering**: The webview uses VS Code's built-in Chromium. We trust it renders standard HTML/CSS correctly.
