# Implementation Plan: Extension Error Code Consolidation

**Design:** [Extension Error Code Consolidation](../design/extension-error-code-consolidation.md)

## Build Pipeline Integration

The generate step must run before **all** build paths. Update `package.json` scripts:

```json
{
  "generate-problems": "node scripts/generate-problems.js",
  "precompile": "npm run generate-problems",
  "compile": "tsc -p ./",
  "esbuild-base": "npm run generate-problems && esbuild ./src/extension.ts --bundle --outfile=out/extension.js --external:vscode --format=cjs --platform=node",
  "vscode:prepublish": "npm run esbuild-base -- --minify"
}
```

This ensures `generate-problems` runs before:
- `compile` (tsc, used for tests/development) — via the `precompile` hook
- `esbuild-base` (bundling) — via explicit chaining
- `vscode:prepublish` (production packaging) — transitively via `esbuild-base`

## Extension Code Changes

Replace hardcoded message strings with calls to the generated module.

**extension.ts — E0001 (compiler not found on activation):**

Before:
```typescript
vscode.window.showErrorMessage(
  'E0001 - Unable to locate IronPLC compiler. IronPLC is not installed or not configured.',
  'Open Online Help',
).then(() => {
  openProblemInBrowser('E0001');
});
```

After:
```typescript
import { ProblemCode, formatProblem } from './problems';

vscode.window.showErrorMessage(
  formatProblem(ProblemCode.NoCompiler, 'IronPLC is not installed or not configured.'),
  'Open Online Help',
).then(() => {
  openProblemInBrowser(ProblemCode.NoCompiler);
});
```

**iplcEditorLogic.ts — E0002 (client not ready when opening .iplc):**

Before:
```typescript
return getErrorHtml(
  'E0002 - IronPLC compiler not found. Install the compiler to view .iplc files.',
);
```

After:
```typescript
import { ProblemCode, formatProblem } from './problems';

return getErrorHtml(
  formatProblem(ProblemCode.ViewerCompilerNotFound, 'Install the compiler to view .iplc files.'),
);
```

**iplcEditorLogic.ts — E0003 (disassembly request failed):**

Before:
```typescript
return getErrorHtml(`E0003 - Failed to disassemble .iplc file: ${message}`);
```

After:
```typescript
return getErrorHtml(formatProblem(ProblemCode.DisassemblyFailed, message));
```

Note: E0003 currently uses `: ` as the separator between the primary message and
context, while the new `formatProblem` uses `. `. This is an intentional change
to make all error messages use a consistent format. The period separator reads
better when the context is a complete sentence (e.g., "Connection timed out.").

**extension.ts — `openProblemInBrowser` signature:**

The function currently takes a plain `string`. Update it to accept `ProblemCode`:

```typescript
function openProblemInBrowser(code: ProblemCode) {
  vscode.env.openExternal(vscode.Uri.parse('https://www.ironplc.com/vscode/problems/' + code + '.html'));
}
```

## File Changes

| File | Change |
|---|---|
| `integrations/vscode/resources/problem-codes.csv` | No change (already correct) |
| `integrations/vscode/scripts/generate-problems.js` | **New** — plain JS build script to generate TypeScript from CSV |
| `integrations/vscode/src/problems.ts` | **New** (generated, gitignored) — constants and helper |
| `integrations/vscode/src/extension.ts` | Replace hardcoded E0001 string with `formatProblem()`; update `openProblemInBrowser` signature |
| `integrations/vscode/src/iplcEditorLogic.ts` | Replace hardcoded E0002/E0003 strings with `formatProblem()` |
| `integrations/vscode/package.json` | Add `generate-problems` npm script; wire into `precompile` and `esbuild-base` |
| `integrations/vscode/.gitignore` | Add `src/problems.ts` (generated file) |
| `integrations/vscode/src/test/unit/iplcEditorLogic.test.ts` | Update assertions that match on hardcoded error code strings (see Testing section) |
| `specs/steering/problem-code-management.md` | Update Extension Error Message Pattern section to use `formatProblem()` |

## Testing

### Generator script tests

Add a unit test for the generator that verifies:
- The generated file contains a `ProblemCode` entry for every CSV row
- The generated file contains a `PROBLEM_MESSAGES` entry for every CSV row
- The generator fails when given a malformed CSV

This can be a simple Node.js test that runs the generator against the real CSV
and spot-checks the output, or against a test fixture CSV.

### `formatProblem()` tests

The generated `formatProblem` function is pure and testable. Add unit tests:
- `formatProblem(ProblemCode.NoCompiler)` → `"E0001 - Unable to locate IronPLC compiler"`
- `formatProblem(ProblemCode.NoCompiler, "some context")` → `"E0001 - Unable to locate IronPLC compiler. some context"`

### Existing test updates

Tests in `iplcEditorLogic.test.ts` assert on hardcoded error strings. These tests
will continue to pass because they match on substrings (`'E0003'`, `'E0002'`,
`'connection lost'`, `'compiler not found'`) that are preserved in the new format.
However, the E0003 format changes from `"E0003 - Failed to disassemble .iplc file: connection lost"`
to `"E0003 - Failed to disassemble .iplc bytecode file. connection lost"`. The test
at line 80 (`html.includes('connection lost')`) still passes, but verify this during
implementation.

### Build verification

- Verify the extension compiles with `tsc` and bundles with `esbuild` using the generated module
- Verify Sphinx docs still build and render the same messages

## Steering File Update

After implementation, update `specs/steering/problem-code-management.md` section
"Extension Error Message Pattern" (lines ~120-128) to replace the hardcoded
inline string examples with the new `formatProblem()` pattern:

```typescript
// Before (old pattern documented in steering file):
vscode.window.showErrorMessage('E0001 - Unable to locate IronPLC compiler. ...');

// After (new pattern to document):
import { ProblemCode, formatProblem } from './problems';
vscode.window.showErrorMessage(
  formatProblem(ProblemCode.NoCompiler, 'IronPLC is not installed or not configured.'),
);
```
