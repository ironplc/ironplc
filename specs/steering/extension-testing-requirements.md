# VS Code Extension Testing Requirements

This document defines the CI gates and structural invariants that prevent untested code from shipping in the VS Code extension. The goal is to make the build fail when test coverage is missing, rather than relying on manual review.

## Coverage Threshold

The extension must enforce a minimum test coverage threshold, similar to the compiler's 85% line coverage requirement.

### What to Measure

Coverage is measured on the **unit-testable modules** only (files in `src/` that do not import `vscode`). These are the files where pure logic lives and where coverage measurement is straightforward with standard Node.js tooling (c8 or nyc/istanbul).

Files that import `vscode` (like `extension.ts` and `iplcEditorProvider.ts`) are excluded from the coverage threshold because they run inside `@vscode/test-electron` where coverage instrumentation is unreliable. These files are covered instead by the structural invariants below.

### Threshold

Set the initial threshold at **80% line coverage** on unit-testable modules. This is deliberately lower than the compiler's 85% to account for the extension being a smaller codebase where a single uncovered branch has a larger percentage impact.

### Enforcement

The `test:unit` npm script must fail if coverage drops below the threshold. Use `c8` (the Node.js coverage tool):

```json
"test:unit": "c8 --check-coverage --lines 80 mocha 'out/test/unit/**/*.test.js'"
```

The `just ci` target must include `test-unit` and must run it before functional tests so that a coverage failure is caught early.

## Structural Invariants

These are checks that verify the extension's declared capabilities (languages, commands, custom editors) each have corresponding test coverage. They run as part of CI and fail the build when a new capability is added without a test.

### Invariant 1: Every Registered Language Has a Detection Test

`package.json` declares languages under `contributes.languages`. Each language with a file extension must have a functional test that opens a file with that extension and verifies the `languageId` is set correctly.

**Current state**: Only `61131-3-st` (`.st`) is tested. `twincat-pou` (`.TcPOU`), `twincat-gvl` (`.TcGVL`), `twincat-dut` (`.TcDUT`) are not tested. `plcopen-xml` uses `firstLine` detection (no extension), which is harder to test but should still have a test.

**Enforcement**: A CI script reads `contributes.languages` from `package.json`, extracts language IDs that have file extensions, and checks that each language ID appears in at least one test file (`extension.test.ts`). If a language ID is declared but not tested, the build fails.

Implementation:

```bash
# check-language-tests.sh
# Extracts language IDs from package.json and checks they appear in test files.
# Fails if any language ID with extensions is not referenced in tests.
```

Add this as a justfile target (`check-invariants`) that runs during `ci`.

### Invariant 2: Every Registered Command Has a Test

`package.json` declares commands under `contributes.commands`. Each command must appear in at least one test file.

**Current state**: `ironplc.createNewStructuredTextFile` is tested.

**Enforcement**: Same approach as Invariant 1 — a script extracts command IDs from `package.json` and checks they appear in test files.

### Invariant 3: Every Custom Editor Has a Test

`package.json` declares custom editors under `contributes.customEditors`. Each custom editor's `viewType` must appear in at least one test file (either unit tests for its rendering logic, or functional tests for its registration).

**Current state**: `ironplc.iplcViewer` has zero test coverage.

**Enforcement**: Same script-based approach.

### Invariant Script Design

Rather than three separate scripts, implement a single `check-test-coverage-invariants.ts` (or `.js`) script that:

1. Reads `package.json`
2. Extracts all language IDs (from `contributes.languages`), command IDs (from `contributes.commands`), and custom editor viewTypes (from `contributes.customEditors`)
3. Searches all `*.test.ts` files for references to each ID
4. Reports any IDs that have no test reference
5. Exits with code 1 if any are missing

This script runs as part of `just ci` via a `check-invariants` target:

```justfile
check-invariants:
  node out/test/checkInvariants.js
```

### Adding Exceptions

Some capabilities may be intentionally untested (e.g., a language that uses `firstLine` detection and is genuinely difficult to test in an automated way). The script should support an exceptions list in a comment or config, but each exception must include a justification.

## CI Pipeline Order

The `just ci` target for the extension should run checks in this order:

```
compile → lint → check-invariants → test-grammar → test-unit → test
```

Rationale:
1. **compile** first — no point running anything if it doesn't build
2. **lint** — fast static checks
3. **check-invariants** — fast structural check, catches missing tests before running any tests
4. **test-grammar** — syntax highlighting snapshots
5. **test-unit** — fast unit tests with coverage threshold
6. **test** — slow functional tests that require VS Code electron

## Rules for Adding New Extension Capabilities

When adding any of the following, tests are mandatory before the PR can merge:

### New Language Type
1. Add the language to `contributes.languages` in `package.json`
2. Add a test resource file with the appropriate extension (e.g., `valid.TcPOU`)
3. Add a functional test that opens the file and asserts the language ID
4. If the language has a TextMate grammar, add grammar snapshot test files
5. The invariant check will fail CI if steps 2-3 are skipped

### New Command
1. Add the command to `contributes.commands` in `package.json`
2. Add a functional test that executes the command and verifies the result
3. The invariant check will fail CI if step 2 is skipped

### New Custom Editor
1. Add the editor to `contributes.customEditors` in `package.json`
2. Extract the editor's rendering logic into a unit-testable module
3. Add unit tests for the rendering logic
4. Add a functional test that verifies the editor is registered
5. The invariant check will fail CI if steps 2-4 are skipped

### New Configuration Setting
1. Add the setting to `contributes.configuration` in `package.json`
2. If the setting affects compiler discovery or client creation, add a unit test for the affected logic
3. Configuration settings are not currently covered by the invariant check (they are low-risk declarative metadata)

## What This Does NOT Enforce

- **Visual correctness** of rendered HTML — only structural/data correctness via unit tests
- **End-to-end integration** with the real compiler — that is covered by the existing Windows smoke test
- **Cross-platform behavior** — the invariant checks and unit tests are platform-independent; platform-specific issues are caught by CI running on multiple OS targets
- **Performance** — no performance regression testing for the extension
