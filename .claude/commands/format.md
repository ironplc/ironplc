# Auto-Fix Formatting and Lint Issues

Auto-fix clippy and rustfmt issues. See [specs/steering/common-tasks.md](../../specs/steering/common-tasks.md) for the full reference.

## Command

```bash
cd compiler && just format
```

This runs:
1. `cargo clippy --fix` - Auto-fix clippy warnings
2. `cargo fmt --all` - Auto-format all Rust code

## Fallback (when `just` is not available)

```bash
cd compiler && cargo clippy --fix && cargo fmt --all
```

## Verify after formatting

After formatting, run the lint check to confirm everything passes:

```bash
cd compiler && just lint
```

Or without just:

```bash
cd compiler && cargo clippy && cargo fmt --all -- --check
```
