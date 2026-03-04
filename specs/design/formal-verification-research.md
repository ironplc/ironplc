# Formal Verification Research for IronPLC

## Context

IronPLC is a Rust-based IEC 61131-3 PLC compiler targeting safety-critical industrial automation. The project already has strong formal verification foundations:

- **ADR-0005**: Safety-first design principle — encode type info in opcodes, not runtime tags
- **ADR-0006**: Bytecode verification requirement — all bytecode must be verified on-device or signature-validated before execution
- **24 bytecode verifier rules** (R0001–R0602): abstract interpretation over the bytecode covering structural validity, type metadata, stack discipline, control flow, FB protocol, and domain-specific enforcement
- **15 semantic analyzer rules**: covering function calls, variable declarations, type checking, POU hierarchy, subranges, enumerations, etc.
- **Defense-in-depth**: compile-time analysis → bytecode verifier → VM runtime checks → cryptographic signatures

The project explicitly chose verification and safety as core principles, making it one of the few PLC compilers with formal verification in its architecture DNA. This research identifies the highest-value formal verification additions.

## Current Verification Gaps

| Area | Current State | Gap |
|------|--------------|-----|
| Bytecode verifier | 24 rules specified, not yet implemented | No proof that verifier rules are sound (reject all bad bytecode) and complete (accept all good bytecode) |
| Semantic rules | 15 rules implemented as visitor patterns | No formal specification of what each rule guarantees; no proof of coverage against IEC 61131-3 |
| Codegen | Integration tests with exact bytecode assertions | No proof that codegen preserves semantics (source → bytecode equivalence) |
| Type system | Intermediate representation with runtime validation | No formal type soundness proof |
| Parser | Pest grammar + AST construction | No grammar equivalence proof against IEC 61131-3 BNF |
| Test coverage | 85% minimum, BDD-style unit tests | No property-based testing or fuzzing |

## Recommended Formal Verification Approaches (Priority Order)

### 1. Property-Based Testing with `proptest` (High Value, Low Effort)

**What**: Add proptest generators for bytecode sequences, AST nodes, and IEC 61131-3 source programs to find edge cases the verifier/analyzer miss.

**Why**: The bytecode verifier spec (24 rules) and semantic analyzer (15 rules) have complex interactions. Manual test cases cannot cover all combinations. This is the fastest path to higher confidence.

**Scope**:
- Generator for random valid/invalid bytecode containers → fuzz the verifier
- Generator for random IEC 61131-3 source → fuzz the parser + analyzer pipeline
- Generator for random AST mutations → test that the analyzer rejects all invalid programs
- Shrinking finds minimal failing cases automatically

**Key files to modify**:
- `compiler/Cargo.toml` — add proptest dev-dependency
- `compiler/analyzer/src/` — add proptest tests alongside existing rule tests
- `compiler/codegen/tests/` — add proptest tests for bytecode generation
- New: `compiler/analyzer/src/test_generators.rs` — shared AST/source generators

**Estimated effort**: 2-3 days for initial generators; ongoing expansion

### 2. Bytecode Verifier Fuzzing with `cargo-fuzz` (High Value, Low Effort)

**What**: Fuzz the bytecode verifier with random bytecode inputs to ensure it never crashes, panics, or accepts malformed bytecode.

**Why**: ADR-0006 explicitly calls out fuzzing as a confirmation method: "Fuzzing the verifier with millions of random bytecode inputs — it must never crash, and must reject all malformed inputs." The verifier is a security boundary — it must be robust against adversarial input.

**Scope**:
- `cargo-fuzz` target that feeds random bytes to the verifier entry point
- Coverage-guided fuzzing to reach deep code paths
- Crash/panic/timeout detection
- Corpus of known-good and known-bad bytecode containers

**Key files**:
- New: `compiler/fuzz/` directory with fuzz targets
- New: `compiler/fuzz/Cargo.toml`
- New: `compiler/fuzz/fuzz_targets/verify_bytecode.rs`

**Estimated effort**: 1 day setup, run continuously in CI

### 3. Semantic Rule Formalization as Pre/Post Conditions (Medium Value, Medium Effort)

**What**: For each of the 15 semantic rules, write formal preconditions and postconditions as executable assertions that can be checked with property-based tests.

**Why**: Currently, rules are implemented as visitor-pattern code that emits diagnostics. There's no formal statement of what property each rule establishes. Formalizing these properties enables:
- Proving rule completeness (no valid program is rejected)
- Proving rule necessity (every rejected program violates IEC 61131-3)
- Regression detection when rules are modified

**Scope**:
- Document each rule's formal property (e.g., `rule_pou_hierarchy` ensures: "For all call chains C in a valid program, if C contains a PROGRAM, then no FUNCTION appears before the PROGRAM in C")
- Express as proptest properties
- Add to `specs/design/` as a formal semantic rules specification

**Key files**:
- New: `specs/design/semantic-rule-properties.md` — formal property catalog
- `compiler/analyzer/src/rule_*.rs` — add property assertions to existing tests

### 4. Codegen Correctness via Differential Testing (Medium Value, Medium Effort)

**What**: For each IEC 61131-3 construct, verify that the compiled bytecode produces the same result as a reference interpreter running the source directly.

**Why**: The codegen tests currently check exact bytecode byte sequences. This proves a specific encoding but not semantic equivalence. A differential test proves the *meaning* is preserved.

**Scope**:
- Reference interpreter that evaluates IEC 61131-3 AST directly (subset: arithmetic, assignments, conditionals)
- Property: for all valid programs P and all inputs I, `eval_ast(P, I) == run_vm(compile(P), I)`
- Use proptest to generate random programs and inputs

**Key files**:
- `compiler/codegen/tests/` — differential test harness
- `compiler/vm/` — already has execution capability
- New: `compiler/analyzer/src/reference_eval.rs` or separate test crate

### 5. Bytecode Verifier Soundness Proof with Kani (High Value, High Effort)

**What**: Use the Kani model checker (Rust-native bounded model checking) to prove that the bytecode verifier correctly implements its 24 rules — specifically, that no bytecode accepted by the verifier can cause the VM to enter an undefined state.

**Why**: The verifier is the primary security boundary (ADR-0006). A bug in the verifier allows malicious bytecode to reach the interpreter. Kani can prove bounded correctness for Rust code without rewriting in a proof language.

**Scope**:
- Prove each verifier rule individually (bounded verification per rule)
- Focus on critical rules first: R0202 (no underflow), R0203 (no overflow), R0300 (type correctness), R0400 (jump validity)
- Kani harnesses that symbolically execute the verifier with arbitrary bytecode inputs

**Key files**:
- `compiler/Cargo.toml` — add kani-verifier as dev-dependency
- New: `compiler/proofs/` directory with Kani harnesses per rule
- `specs/design/bytecode-verifier-rules.md` — add formal invariants alongside existing rules

**Estimated effort**: 2-4 weeks for critical rules; ongoing for full coverage

### 6. Type System Soundness (Future, High Effort)

**What**: Formally prove that the IronPLC type system (intermediate types + semantic rules) is sound — well-typed programs don't get stuck.

**Why**: This is the gold standard for compiler verification. However, it requires significant formal methods expertise and likely a proof assistant (Coq, Lean, or Isabelle).

**Scope**: Future work. The prerequisite is a formal grammar of the type system, which items 3 and 4 help build toward.

## Implementation Plan

### Phase 1: Quick Wins (Week 1)
1. Set up `cargo-fuzz` for the bytecode verifier
2. Add `proptest` dependency and initial generators for bytecode containers
3. Write first property-based tests for verifier rules R0001 (valid opcodes) and R0202 (no underflow)

### Phase 2: Semantic Coverage (Week 2-3)
4. Write formal property catalog for all 15 semantic rules (`specs/design/semantic-rule-properties.md`)
5. Add proptest generators for IEC 61131-3 source fragments
6. Add property-based tests for semantic rules (focus on type checking and call hierarchy)

### Phase 3: Codegen Verification (Week 3-4)
7. Build minimal reference evaluator for AST expressions
8. Add differential tests for arithmetic, assignment, and conditional compilation
9. Expand proptest program generators to cover more language constructs

### Phase 4: Model Checking (Month 2+)
10. Evaluate Kani for bounded verification of verifier rules
11. Write initial Kani harnesses for stack discipline rules (R0200-R0203)
12. Expand to control flow rules (R0400-R0404)

## Key References

- `specs/adrs/0005-safety-first-design-principle.md` — design philosophy
- `specs/adrs/0006-bytecode-verification-requirement.md` — verification requirement
- `specs/design/bytecode-verifier-rules.md` — 24 verifier rules specification
- `compiler/analyzer/src/rule_*.rs` — 15 semantic analysis rules
- `compiler/codegen/` — bytecode generation
