# Reject zero `max_call_depth` containers at load time

## Motivation

The VM's call-frame buffer is sized from `container.header.max_call_depth`.
Real codegen always computes a depth of at least 1 (an entry function with
no callees counts as one frame), so a declared depth of `0` only ever means
"not computed" — a hand-built or legacy container.

Today `0` is treated as a sentinel that *disables* the load-time
`ProgramExceedsCallDepth` check and falls back to a hard-coded
`MAX_CALL_DEPTH = 32` when sizing the frame buffer (`buffers.rs`). That is
the last remaining use of `MAX_CALL_DEPTH`, the constant issue #963 wanted
removed.

We make a zero declared depth an explicit load-time error instead. This
makes the RAM budget truthful, lets us delete `MAX_CALL_DEPTH`, and closes
out the open question from #962 (what to do when the header field is 0):
reject, don't fall back.

## Scope of the back-compat change

Rejecting `0` means previously-built zero-depth containers no longer load.
This is acceptable pre-1.0 and only affects hand-built/legacy fixtures — no
codegen output is zero-depth. The frozen golden container
(`vm-cli/resources/test/steel_thread.iplc`, depth 0) is refreshed to declare
depth 1 so it keeps exercising the other frozen header fields.

## Changes

1. **New trap `Trap::ZeroCallDepth`** (`vm/src/error.rs`) + `Display` arm +
   unit test. Problem code `V9017` (internal VM/container category, exit code
   3) in `vm/resources/problem-codes.csv`, documented in
   `docs/reference/runtime/problems/V9017.rst`. Update `V9016.rst` to drop the
   "`required` of 0 is not raised" note.

2. **Load-time rejection** (`vm/src/vm.rs`, `VmReady::start`): reject
   `max_call_depth == 0` with `Trap::ZeroCallDepth` before the existing
   `ProgramExceedsCallDepth` comparison.

3. **Delete `MAX_CALL_DEPTH`** (`vm/src/vm.rs`) and the fallback in
   `vm/src/buffers.rs` — size the frame buffer directly from
   `header.max_call_depth`.

4. **Test fixtures** — declare a real `max_call_depth` on every container that
   is executed (`.start()`-ed): the `vm/tests/it/common` helpers and inline
   containers in the vm integration tests, the `vm/src/vm.rs` unit tests, and
   the `vm-cli` test container writers. Single-frame programs use depth `1`;
   programs with user `CALL`/`FB_CALL` use their true nesting depth.

5. **Rewrite the two zero-depth tests** in
   `vm/tests/it/load_max_call_depth.rs` to assert the new behavior (empty
   frame buffer; `Trap::ZeroCallDepth` at start).

6. **Refresh the golden fixture** `vm-cli/resources/test/steel_thread.iplc`
   (patch `max_call_depth` 0 → 1) and update its test comment.

7. **Spec** (`specs/design/bytecode-container-format.md`): document that
   `max_call_depth` must be ≥ 1 and that `0` is rejected at load.

## Verification

`cd compiler && just` (compile, coverage/tests, clippy, fmt) must pass.
