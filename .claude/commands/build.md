# Build the Compiler

Build the IronPLC compiler. See [specs/steering/common-tasks.md](../../specs/steering/common-tasks.md) for the full build reference.

## Command

```bash
cd compiler && just compile
```

## Fallback (when `just` is not available)

```bash
cd compiler && cargo build
```

## Other Components

- **Documentation**: `cd docs && just compile`
- **VS Code Extension**: `cd integrations/vscode && just compile`
- **Playground**: `cd playground && just compile`
