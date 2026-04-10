# Spec-Driven Adoption Plan

## Goal

Pivot IronPLC to spec-driven development (SDD) where specifications are the
source of truth and implementation is verified against specs, not the other way
around.

## Current State Assessment

IronPLC already has strong spec culture:

- **35 ADRs** capturing architectural decisions (why)
- **31 design docs** specifying systems and formats (what)
- **64 implementation plans** with task breakdowns (how)
- **9 steering files** guiding AI-assisted development
- **118 documented problem codes** with examples and fixes

The design docs for the IPLC binary format and instruction set are genuinely
detailed specifications — byte offsets, field types, opcode tables. The
project's workflow already follows a spec-plan-implement cycle.

### What's Missing

The gap is **enforcement**. The spec and code are two independent sources of
truth maintained by hand. Concretely:

1. **Opcode drift** — `specs/design/bytecode-instruction-set.md` defines
   opcodes in markdown tables. `compiler/container/src/opcode.rs` defines them
   as Rust constants. Nothing verifies they match. A developer can add an
   opcode to one without the other.

2. **Header drift** — `specs/design/bytecode-container-format.md` specifies
   the file header layout with byte offsets. `compiler/container/src/header.rs`
   implements the same structure. Nothing verifies the field order, types, or
   offsets match.

3. **Flags drift** — the spec defines `flags` as `Bit 0: has content
   signature; Bit 1: has debug section; Bit 2: has type section`. The code
   defines `FLAG_HAS_SYSTEM_UPTIME = 0x01`. These already disagree — the spec
   and implementation have diverged.

4. **No machine-readable spec layer** — the specs are markdown prose. AI and
   humans read them, but tools cannot validate against them.

5. **IEC 61131-3 is closed** — the foundational spec cannot be distributed,
   making traditional spec-driven approaches (embed the spec, generate from
   it) impossible for language features.

## Spec-Kit Assessment

[Spec-kit](https://github.com/github/spec-kit) is a GitHub-created toolkit for
spec-driven development with AI. After thorough evaluation:

### What spec-kit provides

- `/speckit.specify` — structured spec generation with user stories, acceptance
  criteria, requirements
- `/speckit.plan` — technical planning with constitutional compliance
- `/speckit.tasks` — executable task lists with parallelization markers
- `/speckit.implement` — AI-driven implementation from specs
- Constitution — immutable architectural principles
- 29+ AI agent integrations (including Claude Code)
- 50+ community extensions

### Fit analysis

| Capability | IronPLC already has | Spec-kit adds |
|------------|---------------------|---------------|
| Spec templates | Design docs in `specs/design/` | Standardized templates with user stories, acceptance criteria |
| Planning | Plans in `specs/plans/` | Plan generation with constitutional gates |
| Task breakdown | Plans have checkbox task lists | Parallelization markers, dependency ordering |
| Architectural principles | Steering files + ADRs | Formal "constitution" with compliance checking |
| AI guidance | CLAUDE.md, CURSOR.md, Kiro steering | Unified agent integration layer |
| Spec-code consistency | **Nothing** | **Nothing** (spec-kit doesn't solve this either) |

### Verdict

**Spec-kit is a workflow framework, not a consistency framework.** It
structures how you go from idea to spec to plan to code, which IronPLC already
does well. Spec-kit does not solve the core problem: verifying that code
matches the spec.

Adopting spec-kit would:
- Add a Python dependency to the dev workflow
- Replace established conventions with spec-kit templates (disruption for
  marginal gain)
- Not solve the spec-code divergence problem

**Recommendation: Do not adopt spec-kit.** IronPLC's existing workflow is
already mature and well-suited to the project. The investment should go toward
spec-code consistency, which spec-kit does not address.

### Android / Claude Code compatibility

If spec-kit were adopted, it would work on Android via Claude Code — the
`specify` CLI runs in the terminal that Claude Code has access to. However,
since the recommendation is to not adopt it, this is moot.

The approach below works naturally with Claude Code on any platform: specs are
markdown files that Claude reads before implementing, and spec tests are Rust
tests that run with `cargo test`.

## Adoption Strategy: Machine-Readable Spec Layer

The key insight: **keep the human-readable markdown specs but add a
machine-readable layer that both humans and tools can validate against.**

### Phase 1: Spec Tests for New Development (immediate)

For every new spec going forward, require **spec conformance tests** — tests
that verify the implementation matches specific claims in the spec.

This is the lightest-weight approach and works immediately:

```rust
// In compiler/container/tests/spec_conformance.rs

/// Spec: bytecode-container-format.md § File Header
/// "The header is exactly 256 bytes."
#[test]
fn header_spec_header_size_is_256_bytes() {
    assert_eq!(std::mem::size_of::<FileHeader>(), 256);
    // Or if FileHeader uses dynamic serialization:
    assert_eq!(HEADER_SIZE, 256);
}

/// Spec: bytecode-container-format.md § File Header
/// "magic | u32 | 0x49504C43 ("IPLC" in ASCII)"
#[test]
fn header_spec_magic_is_iplc_ascii() {
    assert_eq!(MAGIC, 0x49504C43);
    assert_eq!(&MAGIC.to_le_bytes(), b"IPLC");
}

/// Spec: bytecode-instruction-set.md § Constants
/// "0x01 | LOAD_CONST_I32"
#[test]
fn opcode_spec_load_const_i32_is_0x01() {
    assert_eq!(LOAD_CONST_I32, 0x01);
}
```

**Convention for new specs:**
- Every design doc that specifies concrete values (opcodes, byte offsets, magic
  numbers, flag bits, encoding tables) must have corresponding spec
  conformance tests
- Test names follow: `{area}_spec_{claim}` (e.g., `header_spec_magic_is_iplc_ascii`)
- Test doc comments reference the spec section they verify

**Why this works for velocity:**
- Zero new tools required
- Tests are written alongside the spec (spec-first) or alongside the
  implementation (same PR)
- Claude Code can read the spec and write the conformance tests
- Works identically on Android, desktop, or web

### Phase 2: Machine-Readable Spec Tables (near-term)

For specs with structured data (opcode tables, header layouts, type mappings),
add a machine-readable companion file alongside the markdown:

```
specs/design/bytecode-instruction-set.md          # Human-readable spec
specs/design/bytecode-instruction-set.opcodes.csv  # Machine-readable opcode table
```

Example `bytecode-instruction-set.opcodes.csv`:
```csv
hex,name,operands,stack_before,stack_after,description
0x01,LOAD_CONST_I32,index:u16,[],[I32],Push 32-bit signed integer from constant pool
0x02,LOAD_CONST_U32,index:u16,[],[U32],Push 32-bit unsigned integer from constant pool
...
```

Then a build-time or test-time check verifies the CSV matches both the
markdown spec and the Rust code:

```rust
#[test]
fn opcode_spec_all_opcodes_match_csv() {
    let csv = include_str!("../../../specs/design/bytecode-instruction-set.opcodes.csv");
    for row in csv.lines().skip(1) {
        let cols: Vec<&str> = row.split(',').collect();
        let hex = u8::from_str_radix(&cols[0].trim_start_matches("0x"), 16).unwrap();
        let name = cols[1];
        // Verify the Rust constant exists and has the right value
        let rust_value = opcode_by_name(name);
        assert_eq!(rust_value, hex, "Opcode {name} mismatch: spec says {hex:#04x}");
    }
}
```

**Why CSV and not JSON/TOML/protobuf:**
- CSV renders nicely in GitHub (and in Claude Code's file viewer)
- Easy to diff in PRs
- Trivially parseable in Rust tests without extra dependencies
- Can be generated from the markdown tables by Claude Code

### Phase 3: Spec-First Workflow Gate (near-term)

Update the steering files and CLAUDE.md to enforce spec-first for new features:

1. **New rule**: Any change that adds or modifies a binary format, opcode,
   type mapping, or wire protocol MUST update the design doc FIRST, then
   implement.

2. **CI check**: A test that verifies every opcode constant in `opcode.rs` has
   a matching row in the CSV (and vice versa). This makes spec drift a CI
   failure.

3. **Steering file update**: Add to `development-standards.md`:
   > For features covered by a design specification in `specs/design/`, the
   > spec MUST be updated before or in the same PR as the implementation.
   > Spec conformance tests MUST be added for any new concrete values
   > (opcodes, byte layouts, flag bits, encoding tables).

### Phase 4: IEC 61131-3 Handling (ongoing)

The closed standard requires a different approach:

1. **IronPLC language reference as the spec** — The 64-page language reference
   in `docs/reference/language/` already documents what IronPLC supports. This
   IS the distributable spec. Treat it as such: update the reference BEFORE
   implementing a new language feature.

2. **Compliance matrix** — The `docs/reference/language/edition-support.rst`
   page already tracks which IEC features are supported. This is the closest
   to an executable spec for language features. Keep it as the definitive list.

3. **Parser spec tests** — For each documented syntax construct in the
   language reference, ensure there's a parser test that verifies the syntax is
   accepted (or rejected, for unsupported features). The test references the
   doc page.

4. **Don't try to reproduce IEC 61131-3** — The project can't distribute the
   standard. Instead, IronPLC's own reference is the spec. When someone
   contributes a feature, they first update the reference to document what
   behavior IronPLC will support, then implement it.

## Summary of Recommendations

| Recommendation | Priority | Effort |
|----------------|----------|--------|
| Do NOT adopt spec-kit | — | — |
| Add spec conformance tests for new specs | Immediate | Low |
| Add `{area}_spec_{claim}` test naming convention | Immediate | Low |
| Update steering files with spec-first rule | Immediate | Low |
| Add machine-readable CSV for opcode table | Near-term | Medium |
| Add CI check for opcode spec-code consistency | Near-term | Medium |
| Treat language reference as the IEC 61131-3 spec | Ongoing | Low |
| Backfill spec conformance tests for existing specs | Later | High |

## What NOT To Do

- **Don't adopt a formal spec language** (TLA+, Alloy, etc.) — these are
  powerful but heavyweight. IronPLC's specs are implementation specs, not
  mathematical models. The spec test approach gets 90% of the value at 10% of
  the cost.

- **Don't try to generate code from specs** — code generation from markdown is
  fragile. The CSV companion + test approach is more robust because it
  validates bidirectionally without coupling the spec format to the code
  structure.

- **Don't backfill everything at once** — the existing specs may have
  diverged. Fix divergence opportunistically (when touching related code) rather
  than in a big-bang audit.

## Impact on Development Velocity

This approach is designed to be **velocity-neutral or positive**:

- No new tools to install or learn
- Specs are still markdown (readable on any device, including Android)
- Spec tests catch drift in CI instead of during debugging
- Claude Code can read the spec and write both implementation and conformance
  tests in the same session
- The CSV companion files are optional scaffolding, not a prerequisite for every
  spec
