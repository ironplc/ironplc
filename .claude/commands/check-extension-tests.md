# Check VS Code Extension Tests

Verify VS Code extension CI gates and structural invariants. See [specs/steering/extension-testing-requirements.md](../../specs/steering/extension-testing-requirements.md) for the full testing requirements.

## Command

```bash
cd integrations/vscode && just ci
```

This runs: compile, lint, and tests.

## Fallback (when `just` is not available)

```bash
cd integrations/vscode && npm run compile && npm run lint && npm test
```

## Structural Invariants

The extension enforces these invariants (build fails if violated):

1. **Every registered language** in `package.json` `contributes.languages` must have a detection test
2. **Every registered command** in `package.json` `contributes.commands` must have a test
3. **Every custom editor** in `package.json` `contributes.customEditors` must have a test

## When Adding New Capabilities

- **New language type**: Add test resource file + functional test asserting the languageId
- **New command**: Add functional test that executes the command and verifies the result
- **New custom editor**: Extract rendering logic to unit-testable module + add functional test
- **New configuration setting**: Add unit tests if it affects logic

## Coverage

Unit-testable modules (files not importing `vscode`) must maintain **80% line coverage**.
