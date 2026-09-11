# ADR-0022: IEC 61131-3:2013 Compiler Flag for LTIME and Future Features

status: accepted
date: 2026-03-11
amended: 2026-09-11 (Implementation Approach named a flag, a field and a rule that were never built)

## Context and Problem Statement

IEC 61131-3 Edition 3 (2013) introduced several new language features, including the LTIME data type. IronPLC currently targets Edition 2 (1993). Edition 3 features should not be available by default because:

1. Many PLC programs in the wild target Edition 2 and should be validated against that standard.
2. Enabling all features unconditionally makes it impossible to detect accidental use of Edition 3 constructs in Edition 2 codebases.
3. A clear opt-in mechanism communicates to users which language edition their code targets.

The question is how to gate Edition 3 features and what mechanism to use.

## Decision Drivers

* **Existing pattern** — IronPLC already gates C-style comments via `CompilerOptions` and a post-tokenization validation rule; the mechanism should be consistent
* **Simplicity** — a single flag is easier to understand than per-feature toggles
* **Extensibility** — future IEC 61131-3:2013 features (e.g., LWORD, WSTRING, namespaces) should use the same gate
* **CLI ergonomics** — the flag should be easy to discover and use
* **Playground compatibility** — the playground should be able to enable the 2013 standard without CLI flags

## Considered Options

### Option A: Single `--std=iec-61131-3:2013` flag on the CLI

A `--std` flag on the `ironplcc` subcommands (check, compile) that accepts the standard version. Internally maps to `CompilerOptions { allow_iec_61131_3_2013: true }`. A post-tokenization rule rejects IEC 61131-3:2013 tokens (starting with `LTIME`) when the flag is absent.

* Good, because it follows the established `allow_c_style_comments` pattern for CompilerOptions
* Good, because one flag covers all IEC 61131-3:2013 features — no proliferation of flags
* Good, because it's simple to explain: "add `--std=iec-61131-3:2013` to use LTIME"
* Good, because the playground can set `allow_iec_61131_3_2013: true` directly on `CompilerOptions`
* Good, because the `--std` naming convention is widely understood (C/C++ compilers use it)
* Good, because it naturally extends to future standard versions

### Option B: Per-feature flags (`--allow-ltime`, `--allow-lword`, etc.)

Each Edition 3 feature gets its own CLI flag and `CompilerOptions` field.

* Good, because users can enable exactly the features they need
* Bad, because it creates flag proliferation as Edition 3 features are added
* Bad, because users must learn which features belong to which edition
* Bad, because it doesn't match the existing single-flag pattern for C-style comments

### Option C: Edition-level enum (`--edition 2` / `--edition 3`)

An enum-based CLI argument selecting the target edition.

* Good, because it models the IEC edition concept directly
* Good, because it naturally extends to future editions
* Bad, because "edition 3" is ambiguous — there may be other "version 3" standards
* Bad, because it requires more complex parsing and validation logic

## Decision Outcome

**Option A: `--std=iec-61131-3:2013` flag.**

A `--std` flag is added to the CLI subcommands that process source files (check, compile, echo). The flag accepts the value `iec-61131-3:2013`. Internally, `CompilerOptions` gains an `allow_iec_61131_3_2013: bool` field (default `false`). A new post-tokenization validation rule rejects IEC 61131-3:2013 tokens when the flag is not set.

### Implementation Approach

> **None of the names in this section exist.** There is no `--std` flag, no
> `allow_iec_61131_3_2013` field and no `rule_token_no_std_2013.rs`. The gate
> was built, under different names — see
> [Amendment: the gate is a feature flag and a dialect preset, not a --std switch](#amendment-the-gate-is-a-feature-flag-and-a-dialect-preset-not-a---std-switch-2026-09-11).

**CompilerOptions** (`compiler/parser/src/options.rs`):
```rust
pub struct CompilerOptions {
    pub allow_c_style_comments: bool,
    pub allow_iec_61131_3_2013: bool,
}
```

**Token validation rule** (`compiler/parser/src/rule_token_no_std_2013.rs`):
Follows the `rule_token_no_c_style_comment.rs` pattern. When `allow_iec_61131_3_2013` is false, scans the token stream and rejects `TokenType::Ltime` (and future IEC 61131-3:2013 tokens) with an appropriate diagnostic.

**CLI** (`compiler/plc2x/bin/main.rs`):
The `FileArgs` struct gains a `--std` flag using a `StdVersion` value enum:
```rust
#[derive(clap::ValueEnum, Clone, Debug)]
enum StdVersion {
    #[value(name = "iec-61131-3:2013")]
    Iec6113132013,
}

struct FileArgs {
    files: Vec<PathBuf>,
    #[arg(long = "std")]
    std_version: Option<StdVersion>,
}
```
This flag is threaded through to `CompilerOptions::allow_iec_61131_3_2013` when creating the parser.

**Playground** (`compiler/playground/src/lib.rs`):
The playground sets `allow_iec_61131_3_2013: true` directly on `CompilerOptions` so that LTIME is always available in the interactive environment.

**check_tokens registration** (`compiler/parser/src/lib.rs`):
The new rule is added to the `rules` vector in `check_tokens()`, alongside the existing C-style comment rule.

### Consequences

* Good, because IEC 61131-3:2013 features are gated behind an explicit opt-in
* Good, because the mechanism is identical to the existing C-style comment gating — no new patterns to learn
* Good, because adding future IEC 61131-3:2013 features only requires adding token checks to the existing rule, not new flags
* Good, because the playground can enable the 2013 standard independently of the CLI
* Good, because `--std` naming is familiar to C/C++ developers
* Neutral, because all IEC 61131-3:2013 features are enabled together — no granular control (acceptable for now)
* Bad, because programs using LTIME without the flag get a tokenizer-level error rather than a more descriptive suggestion (can be improved in the diagnostic message)

## More Information

### Relationship to ADR-0021

ADR-0021 defines TIME as 32-bit and LTIME as 64-bit with millisecond precision. This ADR defines the mechanism by which LTIME becomes available to users. The two ADRs are complementary: ADR-0021 covers the data representation, this ADR covers the feature gate.

### Future IEC 61131-3:2013 Features

When additional IEC 61131-3:2013 features are implemented (e.g., LWORD, WSTRING, LREAL if not already present), they should be gated by the same `allow_iec_61131_3_2013` flag and checked in the same `rule_token_no_std_2013.rs` validation rule. This keeps the feature gate centralized and consistent.

### Amendment: Flag Renamed (2026-03-12)

The flag was renamed from `--std=iec-61131-3:2013` to `--std-iec-61131-3=2013`. The flag **name** now identifies the standard (`--std-iec-61131-3`) and the **value** is the publication year (`2013`).

Rationale:
* **Unambiguous values** — Using the publication year (`2013`) avoids confusion with the edition number coinciding with the part number (edition 3 of part 3).
* **Multi-standard support** — Embedding the standard name in the flag allows future standards (e.g., `--std-iec-61499=...`) to coexist as independent flags, enabling simultaneous multi-spec support without comma-separated values or repeated `--std` flags.
* **Cross-shell compatibility** — The `--flag=value` syntax is standard POSIX long-option convention and works across Linux, macOS, and Windows shells.

## Amendment: the gate is a feature flag and a dialect preset, not a `--std` switch (2026-09-11)

The decision holds: an IEC 61131-3:2013 construct is rejected unless the user
asks for Edition 3, and the default is Edition 2. `LTIME` is not accepted by
`ironplcc check` out of the box, which is what this ADR set out to guarantee.
It is accepted on that basis.

Every name in the Implementation Approach above is wrong, because the mechanism
was generalised before it was built.

| This ADR says | What exists |
|---|---|
| `--std=iec-61131-3:2013` CLI flag | `--dialect iec61131-3-ed3`, one of five presets, plus per-feature `--allow-*` flags |
| `CompilerOptions::allow_iec_61131_3_2013: bool` | `CompilerOptions::allow_long_time_types`, declared through `define_compiler_options!` |
| `rule_token_no_std_2013.rs`, a post-tokenization rejection rule | `xform_demote_keywords.rs`, which demotes `LTIME`/`LDATE`/`LTOD`/`LDT` to identifiers when the flag is off |
| `StdVersion` value enum in `FileArgs` | `Dialect`, with `ClapDialect` wrapping it for the CLI |
| playground sets `allow_iec_61131_3_2013: true` | playground selects a dialect; all five are exposed in the UI |

Three things drove the change, each decided by a later ADR:

1. **One flag per edition does not scale to vendors.** ADR-0036 settled that
   IronPLC defines no dialect of its own and that `--allow-*` flags exist to
   *compose* real targets. An edition is then one more bundle of flags, not a
   special case with its own switch. `Dialect::Iec61131_3Ed3` is that bundle.
2. **An edition is not one feature.** Edition 3 brought the long time types,
   `REF_TO`, and more; gating them behind a single boolean would mean a user who
   wants `LTIME` also gets `REF_TO`. Per-feature flags (`--allow-long-time-types`,
   `--allow-ref-to`) let the preset group them while leaving each separable —
   and ADR-0038 decided no flag combination is restricted.
3. **Rejection is the wrong mechanism for a word that can be an identifier.**
   ADR-0040 rule 3 makes this explicit: `LTIME` is a legal variable name in
   Edition 2 code, so a token-rejection rule would break programs that never
   meant the keyword. Demotion keeps them parsing. This is why
   `rule_token_no_std_2013.rs` does not exist while its siblings
   (`rule_token_no_c_style_comment.rs`, `rule_token_no_partial_access_syntax.rs`)
   do — those reject spellings that cannot be identifiers.

`specs/design/time-literals.md` REQ-TL-003 cited
`CompilerOptions::allow_iec_61131_3_2013` and the "post-tokenization validation
rule" until this amendment, six months after neither existed — a reader
implementing that requirement would have gone looking for both. It now names
`allow_long_time_types` and the demotion, and carries the demotion's known cost.
Measured: `t := LTIME#5s;` without the flag reports

```
error[P0002]: Syntax error
  ... Found text '#' that matched token '#'
```

— because `LTIME` demoted to an identifier leaves the `#` with nothing to
introduce. Not "LTIME requires Edition 3". ADR-0040 rule 3 records exactly this
as the accepted price: the program is genuinely ambiguous once the keyword is
identifier-shaped, so the compiler cannot know which was meant.
