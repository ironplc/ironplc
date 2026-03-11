# ADR-0022: IEC 61131-3 Edition 3 Compiler Flag for LTIME and Future Features

status: proposed
date: 2026-03-11

## Context and Problem Statement

IEC 61131-3 Edition 3 (2013) introduced several new language features, including the LTIME data type. IronPLC currently targets Edition 2 (1993). Edition 3 features should not be available by default because:

1. Many PLC programs in the wild target Edition 2 and should be validated against that standard.
2. Enabling all features unconditionally makes it impossible to detect accidental use of Edition 3 constructs in Edition 2 codebases.
3. A clear opt-in mechanism communicates to users which language edition their code targets.

The question is how to gate Edition 3 features and what mechanism to use.

## Decision Drivers

* **Existing pattern** — IronPLC already gates C-style comments via `ParseOptions` and a post-tokenization validation rule; the mechanism should be consistent
* **Simplicity** — a single flag is easier to understand than per-feature toggles
* **Extensibility** — future Edition 3 features (e.g., LWORD, WSTRING, namespaces) should use the same gate
* **CLI ergonomics** — the flag should be easy to discover and use
* **Playground compatibility** — the playground should be able to enable Edition 3 without CLI flags

## Considered Options

### Option A: Single `--edition-3` flag on the CLI

A single boolean flag `--edition-3` on the `ironplcc` subcommands (check, compile). Internally maps to `ParseOptions { allow_edition_3: true }`. A post-tokenization rule rejects Edition 3 tokens (starting with `LTIME`) when the flag is absent.

* Good, because it follows the established `allow_c_style_comments` pattern exactly
* Good, because one flag covers all Edition 3 features — no proliferation of flags
* Good, because it's simple to explain: "add `--edition-3` to use LTIME"
* Good, because the playground can set `allow_edition_3: true` directly on `ParseOptions`
* Neutral, because the flag name is tied to a specific IEC edition number

### Option B: Per-feature flags (`--allow-ltime`, `--allow-lword`, etc.)

Each Edition 3 feature gets its own CLI flag and `ParseOptions` field.

* Good, because users can enable exactly the features they need
* Bad, because it creates flag proliferation as Edition 3 features are added
* Bad, because users must learn which features belong to which edition
* Bad, because it doesn't match the existing single-flag pattern for C-style comments

### Option C: Edition-level enum (`--edition 2` / `--edition 3`)

An enum-based CLI argument selecting the target edition.

* Good, because it models the IEC edition concept directly
* Good, because it naturally extends to future editions
* Bad, because it's over-engineered for the current need (only one additional edition)
* Bad, because it requires more complex parsing and validation logic
* Bad, because it doesn't align with the existing boolean flag pattern

## Decision Outcome

**Option A: Single `--edition-3` flag.**

A boolean `--edition-3` flag is added to the CLI subcommands that process source files (check, compile, echo). Internally, `ParseOptions` gains an `allow_edition_3: bool` field (default `false`). A new post-tokenization validation rule rejects Edition 3 tokens when the flag is not set.

### Implementation Approach

**ParseOptions** (`compiler/parser/src/options.rs`):
```rust
pub struct ParseOptions {
    pub allow_c_style_comments: bool,
    pub allow_edition_3: bool,
}
```

**Token validation rule** (new file `compiler/parser/src/rule_token_no_edition_3.rs`):
Follows the `rule_token_no_c_style_comment.rs` pattern. When `allow_edition_3` is false, scans the token stream and rejects `TokenType::Ltime` (and future Edition 3 tokens) with an appropriate diagnostic.

**CLI** (`compiler/plc2x/bin/main.rs`):
The `FileArgs` struct gains an `--edition-3` flag:
```rust
struct FileArgs {
    files: Vec<PathBuf>,
    #[arg(long)]
    edition_3: bool,
}
```
This flag is threaded through to `ParseOptions::allow_edition_3` when creating the parser.

**Playground** (`compiler/playground/src/lib.rs`):
The playground sets `allow_edition_3: true` directly on `ParseOptions` so that LTIME is always available in the interactive environment.

**check_tokens registration** (`compiler/parser/src/lib.rs`):
The new rule is added to the `rules` vector in `check_tokens()`, alongside the existing C-style comment rule.

### Consequences

* Good, because Edition 3 features are gated behind an explicit opt-in
* Good, because the mechanism is identical to the existing C-style comment gating — no new patterns to learn
* Good, because adding future Edition 3 features only requires adding token checks to the existing rule, not new flags
* Good, because the playground can enable Edition 3 independently of the CLI
* Neutral, because all Edition 3 features are enabled together — no granular control (acceptable for now)
* Bad, because programs using LTIME without the flag get a tokenizer-level error rather than a more descriptive "enable Edition 3" suggestion (can be improved in the diagnostic message)

## More Information

### Relationship to ADR-0021

ADR-0021 defines TIME as 32-bit and LTIME as 64-bit with millisecond precision. This ADR defines the mechanism by which LTIME becomes available to users. The two ADRs are complementary: ADR-0021 covers the data representation, this ADR covers the feature gate.

### Future Edition 3 Features

When additional Edition 3 features are implemented (e.g., LWORD, WSTRING, LREAL if not already present), they should be gated by the same `allow_edition_3` flag and checked in the same `rule_token_no_edition_3.rs` validation rule. This keeps the feature gate centralized and consistent.
