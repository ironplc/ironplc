# Bind `CALL_DEPTH_EXCEEDED` to a spec-conformance requirement

## Goal

Close out the remaining spec-test half of issue #962. The VM already honours
the per-program `container.max_call_depth` (the hardcoded `MAX_CALL_DEPTH`
constant is gone and a zero depth is rejected at load), but the
`CALL_DEPTH_EXCEEDED` trap contract in `runtime-execution-model.md` still has
no `REQ-` ID and no `#[spec_test]` binding. This plan adds that binding so the
per-program call-depth limit is enforced bidirectionally by the
spec-conformance machinery.

## Architecture

Wire the `ironplc-vm` crate into the existing spec-conformance pipeline
(`spec_requirements_gen` + `spec_test_macro`), following the `vm-cli` pattern
where the `#[spec_test]` lives in the integration-test binary:

1. Add a single testable requirement `REQ-RT-vm-001` to the Trap Sources table
   in `runtime-execution-model.md` (new area code `RT` = runtime, owning crate
   slug `vm`), via a dedicated `Requirement` first column (matching the
   partially-filled-column house style in `bytecode-container-format.md`).
2. Have `vm/build.rs` list `runtime-execution-model.md` through
   `ironplc_spec_requirements_gen::generate`, and add the `spec_requirements`
   module + completeness meta-test to the `it` integration-test binary.
3. Convert the existing self-recursion trap test to `#[spec_test(REQ_RT_vm_001)]`,
   parameterised over `max_call_depth` so it verifies the per-program contract
   at several depths rather than one hardcoded value.

The workspace orphan guard (`compiler/test/tests/spec_conformance_guard.rs`)
already passes for this shape: `vm/build.rs` listing the doc claims
`(vm, runtime-execution-model.md)`, which matches the `REQ-RT-vm-001` slug.

## Design doc reference

- `specs/design/runtime-execution-model.md` (Trap Sources / trap contract)
- `specs/design/cross-crate-spec-conformance.md` (ID grammar, ownership)
- `specs/design/spec-conformance-testing.md` (enforcement mechanism)

## File map

- `specs/design/runtime-execution-model.md` — add `Requirement` column to the
  Trap Sources table; fill `REQ-RT-vm-001` on the `CALL_DEPTH_EXCEEDED` row.
- `compiler/vm/build.rs` — call `ironplc_spec_requirements_gen::generate`.
- `compiler/vm/Cargo.toml` — add `ironplc-spec-requirements-gen` build-dep and
  `spec_test_macro` dev-dep.
- `compiler/vm/tests/it/main.rs` — add `spec_requirements` module + meta-test.
- `compiler/vm/tests/it/execute_stack_overflow.rs` — convert the recursion test
  to `#[spec_test(REQ_RT_vm_001)]`, parameterised over depth.

## Tasks

- [ ] Add `REQ-RT-vm-001` to the Trap Sources table.
- [ ] Wire `vm/build.rs` + `vm/Cargo.toml` dependencies.
- [ ] Add `spec_requirements` module and meta-test to the `it` binary.
- [ ] Convert + parameterise the recursion spec test.
- [ ] `cargo test -p ironplc-vm` green; full `cd compiler && just` green.
