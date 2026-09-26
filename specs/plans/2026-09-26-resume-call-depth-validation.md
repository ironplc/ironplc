# Validate call depth on `VmReady::resume`

Issue: https://github.com/ironplc/ironplc/issues/1526

## Problem

`VmReady::start` rejects a container whose `max_call_depth` is 0 (V9017) or
exceeds the frame buffer (V9016). `VmReady::resume` performs no check, so the
same container yields V9012 at the first push instead. Nothing tests `resume`.

## Approach

Make `resume` fallible (`Result<VmRunning, FaultContext>`) rather than hoisting
into `Vm::load`: `resume` has two callers, `load` has many, and both paths
then report identical traps.

1. **Prefactor**: extract the check in `start` into a private
   `VmReady::validate_call_depth` helper. No behaviour change.
2. **Fix**: call the helper from `resume`; update the two callers
   (`ironplc-cli/src/lsp_runner.rs`, `playground/src/lib.rs`) to report the
   fault like any other trap.
3. **Tests**: add `resume` cases to `vm/tests/it/load_max_call_depth.rs`
   (zero depth, exceeding depth, success keeps the scan count).
4. **Docs/comments**: `buffers.rs`, the test module comment, V9016.rst and
   V9017.rst name both `start` and `resume`.

Out of scope: `call_graph.rs` `saturating_add` clamping (tracked by #1473's class
of issues; noted in the issue as adjacent).
