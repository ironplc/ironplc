# Run Tests

Run the IronPLC test suite. See [specs/steering/common-tasks.md](../../specs/steering/common-tasks.md) for the full testing reference.

## Command

```bash
cd compiler && just test
```

## Fallback (when `just` is not available)

```bash
cd compiler && cargo test --all-targets
```

## Run tests for a specific crate

```bash
cd compiler && cargo test --package <crate-name>
```

## Run tests with coverage (enforces 85% line coverage)

```bash
cd compiler && just coverage
```

### Coverage fallback

```bash
cd compiler && cargo llvm-cov --ignore-filename-regex "cargo|dsl_macro_derive|rustup" --workspace --fail-under-lines 85 --show-missing-lines --lcov --output-path lcov.info
```
