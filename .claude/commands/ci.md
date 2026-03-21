# Run Full CI Pipeline

Run the complete CI pipeline. **This is REQUIRED before creating any pull request.** See [specs/steering/common-tasks.md](../../specs/steering/common-tasks.md) for the full CI reference.

## Command

```bash
cd compiler && just
```

This runs all three required checks in order:
1. **compile** - `cargo build`
2. **coverage** - Run all tests and verify 85% line coverage threshold
3. **lint** - `cargo clippy` + `cargo fmt --all -- --check`

**All checks must pass before creating a PR.**

## Fallback (when `just` is not available)

Run these three steps in order:

```bash
cd compiler && cargo build
cd compiler && cargo llvm-cov --ignore-filename-regex "cargo|dsl_macro_derive|rustup" --workspace --fail-under-lines 85 --show-missing-lines --lcov --output-path lcov.info
cd compiler && cargo clippy && cargo fmt --all -- --check
```

## Fixing Common Failures

- **Clippy warnings**: Fix the code or run `cd compiler && just format`
- **Format issues**: Run `cd compiler && just format` (or `cd compiler && cargo clippy --fix && cargo fmt --all`)
- **Coverage below 85%**: Add tests for uncovered code paths

## Other Components

- **VS Code Extension**: `cd integrations/vscode && just ci`
- **Documentation**: `cd docs && just`
- **Playground**: `cd playground && just`
- **All components smoke test**: `just devenv-smoke`
