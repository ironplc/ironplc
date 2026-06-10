# Convert VM Dispatch from Recursive to Iterative

## Goal

Replace the VM's recursive `CALL` / `FB_CALL` dispatch in
`compiler/vm/src/vm.rs` with an iterative loop driven by an explicit,
embedder-allocated `FrameStack`. Each PLC call becomes a frame push;
each return becomes a frame pop. The Rust call stack is no longer
consumed proportionally to PLC call depth.

Scope is restricted to the dispatch-shape change. No new debugger
features, no DAP, no pause/resume — those land later phases of
`specs/design/debugger-support.md`. Public behavior is bit-for-bit
identical to today: every existing VM test passes unchanged.

## Why now

1. **Stack hygiene.** Today every PLC `CALL` consumes one Rust stack
   frame (`vm.rs:1096`, `vm.rs:1918`). The `MAX_CALL_DEPTH = 32` cap
   (`vm.rs:31`) exists *only* to keep the recursion well clear of
   the Rust thread-stack limit. The bound is artificial — it has
   nothing to do with the program's semantic needs. With an explicit
   frame stack, the bound becomes a real resource limit on a real
   buffer.
2. **Wasm / `no_std` correctness.** Rust-stack recursion behaves
   differently on wasm32 and on Arduino-class targets than it does on
   a desktop host. An explicit frame buffer is the same shape as
   `OperandStack` and `VariableTable` (`compiler/vm/src/stack.rs:5`,
   `variable_table.rs`) and is portable to every target we already
   support.
3. **DAP prerequisite.** Instruction-level pause/resume (Phase 2 of
   `specs/design/debugger-support.md`) requires that a paused VM
   leaves no Rust frames on the stack — otherwise resume cannot
   restore state. This PR is the gate for that work, even though it
   delivers no debugger feature itself.

The verifier and `header.max_call_depth` plumbing described in the
debugger-support spec are **deferred**. Safety is preserved by the
existing `Trap::CallStackOverflow` — the new `FrameStack::push` returns
it on buffer exhaustion exactly the way the current `depth >=
MAX_CALL_DEPTH` check does (`vm.rs:1093`, `vm.rs:1915`). A
malicious or buggy container can only cause a clean trap, never UB.

## Architecture

### `Frame` and `FrameStack`

New file: `compiler/vm/src/frame_stack.rs`.

```rust
/// One PLC call frame. `Copy` so the frame backing slice can live
/// in any contiguous storage, including `[MaybeUninit<Frame>; N]`
/// on no_std targets.
#[derive(Clone, Copy)]
pub struct Frame {
    /// Function whose bytecode this frame is executing.
    pub function_id: FunctionId,
    /// Offset of the next opcode to execute within the function's
    /// bytecode.
    pub pc: usize,
    /// Variable scope (globals view + this frame's locals window).
    pub scope: VariableScope,
    /// Temp-buf allocator's `next` value at frame entry; restored
    /// on RET so caller's slot indices remain valid.
    pub temp_alloc_mark: u16,
    /// If `Some`, this frame was pushed by an FB_CALL on a
    /// user-defined function block. On RET, the dispatch loop runs
    /// the FB copy-out (variables -> data_region) using the saved
    /// instance pointer.
    pub fb_return: Option<FbCallReturn>,
}

#[derive(Clone, Copy)]
pub struct FbCallReturn {
    pub instance_start: usize,
    pub var_offset: u16,
    pub num_fields: u16,
}

/// Bounded-capacity, borrowed-slice frame stack. Same shape as
/// `OperandStack` and `VariableTable`. Allocates nothing.
pub struct FrameStack<'a> {
    slots: &'a mut [Frame],
    len: usize,
}

impl<'a> FrameStack<'a> {
    pub fn new(backing: &'a mut [Frame]) -> Self { ... }

    /// `Trap::CallStackOverflow` on push past capacity.
    pub fn push(&mut self, frame: Frame) -> Result<(), Trap> { ... }
    pub fn pop(&mut self) -> Option<Frame> { ... }
    pub fn top(&self) -> Option<&Frame> { ... }
    pub fn top_mut(&mut self) -> Option<&mut Frame> { ... }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}
```

The backing slice is sized at `MAX_CALL_DEPTH = 32` — the same bound
used today. This is unchanged behavior; tuning the bound from the
container header is deferred.

### Buffer plumbing

- `VmBuffers` (`compiler/vm/src/buffers.rs`) gains
  `frames: Vec<Frame>` of length `MAX_CALL_DEPTH`, alongside the
  existing `stack`, `vars`, etc.
- `Vm::load` builds a `FrameStack` from `&mut bufs.frames` and
  threads it through `VmReady` and `VmRunning` as
  `frames: &'a mut [Frame]` (matching the existing pattern for
  `stack`, `vars`, `data_region`).
- Top-level `execute` calls (init in `VmReady::start`,
  scan/program-instance bodies in `VmRunning::run_round`) build a
  fresh `FrameStack` borrow each time. The frame stack must be empty
  on entry and empty on exit; an assertion at top-of-loop catches
  any leak. No state persists across `execute` calls — this PR is
  not adding pause/resume.

### Restructured `execute_with_hook`

Today's loop (`vm.rs:693`) iterates opcodes within a single
function's bytecode and recurses on `CALL` / `FB_CALL` (user FB
branch). The new loop iterates opcodes off the top frame and uses
frame push/pop to switch functions:

```rust
pub(crate) fn execute_with_hook<H: DebugHook>(
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    data_region: &mut [u8],
    temp_buf: &mut [u8],
    max_temp_buf_bytes: usize,
    frames: &mut FrameStack,
    current_time_us: u64,
    hook: &mut H,
    #[cfg(feature = "profiling")] profile: &mut InstructionProfile,
) -> Result<(), Trap> {
    let mut temp_alloc = string_ops::TempBufAllocator::new(max_temp_buf_bytes);

    // Caller pushed exactly one entry frame before calling us.
    // Run until the frame stack drains.
    while let Some(top) = frames.top_mut() {
        let function_id = top.function_id;
        let bytecode = container
            .code
            .get_function_bytecode(function_id)
            .ok_or(Trap::InvalidFunctionId(function_id))?;

        if top.pc >= bytecode.len() {
            // Running off the end of a function without RET is a bug
            // in codegen, but mirror today's behavior: pop and continue.
            handle_frame_return(&mut temp_alloc, frames, data_region, variables)?;
            continue;
        }

        let op = bytecode[top.pc];
        hook.before_instruction(function_id, top.pc, op);
        top.pc += 1;

        #[cfg(feature = "profiling")]
        profile.record(op);

        // Dispatch. CALL pushes a new frame; RET/RET_VOID pops the
        // current frame (running FB copy-out if needed). All other
        // opcodes operate on the borrowed `top` (mainly `top.pc`)
        // plus the shared `stack`/`variables`/`data_region`.
        match op {
            opcode::CALL => { ... frames.push(callee_frame)?; }
            opcode::FB_CALL => { ... maybe-push for user-FB branch; intrinsics unchanged ... }
            opcode::RET | opcode::RET_VOID => {
                handle_frame_return(&mut temp_alloc, frames, data_region, variables)?;
            }
            // ... every other opcode arm: identical to today except
            // it reads `pc` / advances `pc` via `top.pc` instead of
            // a local `pc` variable ...
        }
    }
    Ok(())
}
```

Two implementation realities:

1. **Borrow shape inside the loop.** `top` is a `&mut Frame`
   borrowed from `frames`. Opcodes that need to read multiple
   operand bytes update `top.pc` repeatedly. Opcodes that push a
   new frame must drop the `top` borrow first — the simplest shape
   is to copy `top.function_id` and `top.scope` into locals at the
   start of each iteration, then write back via a separate `top_mut`
   call at the point a frame push/pop happens. The dispatch arms
   are roughly the same length as today; the bookkeeping
   reorganizes around a per-iteration borrow rather than a single
   long-lived `&mut` to a stack-local `pc: usize`.

2. **`read_u16_le` / `read_u8` adapt to take `&mut Frame`** or take
   `&[u8]` plus `&mut usize` as today and read against `&mut
   top.pc`. Either works; the second changes less. The plan keeps
   the existing helper signature and passes `&mut top.pc`.

### `CALL` handler (was: vm.rs:1065)

Today:
1. Decode `func_id`, `var_offset`.
2. Pop arguments into the callee's parameter slots.
3. Recurse `execute_with_hook` with the callee's bytecode/scope.

New:
1. Decode `func_id`, `var_offset` (advancing `top.pc`).
2. Pop arguments into the callee's parameter slots (unchanged).
3. Build `Frame { function_id, pc: 0, scope, temp_alloc_mark:
   temp_alloc.next(), fb_return: None }` and `frames.push(...)`.
4. Continue the loop. The new top is the callee; its `pc` starts
   at 0.

`Trap::CallStackOverflow` now comes from `FrameStack::push`, not from
the `depth >= MAX_CALL_DEPTH` check — same trap, same observable
behavior.

### `FB_CALL` handler (was: vm.rs:1788)

Built-in FB intrinsics (TON / TOF / TP / CTU / CTD / CTUD / SR / RS /
R_TRIG / F_TRIG) are unchanged — they execute directly against the
data region, with no recursion. Only the user-FB branch
(`vm.rs:1871-1942`) involves a call.

Today the user-FB branch:
1. Copies data-region fields into variable slots (copy-in).
2. Recurses `execute_with_hook` on the FB's body.
3. Copies variable slots back to data-region fields (copy-out).

New:
1. Copy-in (unchanged).
2. Build `Frame { ..., fb_return: Some(FbCallReturn {
   instance_start, var_offset, num_fields }) }` and push.
3. Loop continues; FB body runs against the pushed frame.
4. On RET / RET_VOID, the pop handler sees `frame.fb_return.is_some()`
   and runs the copy-out before returning to the caller frame.

The copy-out logic is the same as today (`vm.rs:1936-1941`), just
moved into the return path so it's run unconditionally on RET no
matter how the body returned.

### `RET` / `RET_VOID` handler (`handle_frame_return`)

```rust
fn handle_frame_return(
    temp_alloc: &mut TempBufAllocator,
    frames: &mut FrameStack,
    data_region: &mut [u8],
    variables: &mut VariableTable,
) -> Result<(), Trap> {
    let popped = frames.pop().expect("non-empty by while-let");
    // Restore caller's temp-buf allocator state, so the caller's
    // previously-pushed temp-buf indices remain valid.
    temp_alloc.rewind_to(popped.temp_alloc_mark);
    // FB copy-out, if this frame was an FB_CALL frame.
    if let Some(fbr) = popped.fb_return {
        for i in 0..(fbr.num_fields as usize) {
            let offset = fbr.instance_start + i * 8;
            let val = variables.load(VarIndex::new(fbr.var_offset + i as u16))?;
            data_region[offset..offset + 8]
                .copy_from_slice(&val.as_i64().to_le_bytes());
        }
    }
    Ok(())
}
```

When `frames.pop()` empties the stack, the outer `while let Some(top)`
loop exits naturally — that's how the top-level call returns to its
caller (`execute()`, then back to `VmReady::start` or
`VmRunning::run_round`).

### Temp-buffer allocator preservation

`TempBufAllocator` (`compiler/vm/src/string_ops.rs:24`) gains a
`next()` getter and a `rewind_to(mark: u16)` method:

```rust
impl TempBufAllocator {
    pub fn next(&self) -> u16 { self.next }
    pub fn rewind_to(&mut self, mark: u16) { self.next = mark; }
}
```

Today the allocator is constructed fresh inside each recursive
`execute_with_hook` call, so the inner frame's allocations live in
its own scope and don't disturb the caller. In the iterative loop
the allocator persists across frames; save/restore matches the
old semantic: on CALL, record `top.temp_alloc_mark = temp_alloc.next()`;
on RET, `temp_alloc.rewind_to(popped.temp_alloc_mark)`.

This preserves exact behavior — if today's recursive design happens
to corrupt aliased temp slots, the iterative version will corrupt
them in the same way. We are not fixing temp-buf semantics in this
PR.

## File map

Modified:

- `compiler/vm/src/buffers.rs` — add `frames: Vec<Frame>` (cap =
  `MAX_CALL_DEPTH`).
- `compiler/vm/src/vm.rs` — the rewrite. `Vm::load` threads the
  frames buffer; `VmReady` / `VmRunning` carry `frames: &'a mut
  [Frame]`; `execute` / `execute_with_hook` lose the `depth` and
  `bytecode` parameters (bytecode is looked up per-iteration from
  the top frame), gain a `frames: &mut FrameStack` parameter; the
  dispatch loop is the iterative form above; the `MAX_CALL_DEPTH`
  constant stays (it's now the buffer-sizing constant rather than
  the recursion-depth check).
- `compiler/vm/src/string_ops.rs` — add `next()` / `rewind_to(mark)`
  on `TempBufAllocator`.
- `compiler/vm/src/lib.rs` — re-export `Frame`, `FrameStack`,
  `FbCallReturn`.

New:

- `compiler/vm/src/frame_stack.rs` — `Frame`, `FbCallReturn`,
  `FrameStack`.

Not modified:

- `compiler/vm/src/debug_hook.rs` — the trait signature stays
  `before_instruction(function_id, pc, op)`. The DAP-grade
  `HookAction`/`PauseReason` extension lands in a future PR.
- Container format, codegen, anything in `compiler/codegen`. This
  PR is VM-internal.
- `MAX_CALL_DEPTH` constant value (still 32). It moves from a
  recursion-depth check to a frame-buffer capacity, but the bound
  is unchanged so no program that runs today will trap that didn't
  before.

## Migration

Per `specs/design/debugger-support.md` §Migration path, the spec
proposes a three-commit sequence with a parallel frame stack and
runtime assertions cross-checking Rust recursion depth. **This PR
collapses to a single implementation commit** on a feature branch
because:

1. The existing test suite — `execute_when_nested_call_then_correct`
   (`tests/it/execute_call_ret.rs:107`),
   `execute_when_user_fb_call_then_executes_body_and_persists_state`
   (`tests/it/execute_fb_ops.rs:106`),
   `execute_when_user_fb_call_then_internal_state_persists_across_rounds`
   (`tests/it/execute_fb_ops.rs:179`),
   `execute_when_call_recursion_exceeds_max_depth_then_traps_call_stack_overflow`
   (`tests/it/execute_stack_overflow.rs:54`), plus every other
   `execute_*` integration test that exercises a CALL anywhere in
   its bytecode — is the regression net. The parallel-stack assertion
   doesn't catch anything these tests don't.
2. Most opcode arms touch `pc` only through `read_u16_le(bytecode,
   &mut pc)`. Once the loop is converted to read against `&mut
   top.pc`, the per-arm changes are mechanical.
3. Keeping a parallel implementation alive for an intermediate
   commit means keeping two CALL paths working — that doubles the
   surface area to debug rather than halving it.

The work order inside the single commit:

1. Add `Frame`, `FrameStack`, `FbCallReturn` in
   `frame_stack.rs`; export from `lib.rs`. Compile in isolation.
2. Add `frames` to `VmBuffers`; thread `&mut [Frame]` through
   `VmReady` / `VmRunning`. Adjust top-level `execute` call sites
   to build a `FrameStack` and push the entry frame.
3. Add `next()` / `rewind_to()` on `TempBufAllocator`.
4. Rewrite `execute_with_hook` to the iterative form. Convert each
   opcode arm to read/write against `top.pc`. Replace the two
   recursive call sites (`CALL`, user-FB `FB_CALL`) with frame
   pushes. Replace the two `return Ok(())` sites (`RET`, `RET_VOID`)
   with `handle_frame_return`. Drop the `depth` parameter chain.
5. Delete the now-unused `depth >= MAX_CALL_DEPTH` checks (the
   trap moves into `FrameStack::push`).
6. `cd compiler && just`. Every existing test must pass.

If any unit/integration test fails, that's the signal to investigate
before claiming the rewrite is correct.

## Tests

Existing tests are the primary regression net. No bytecode changes,
no container-format changes, no public-API behavior changes.

New tests (added in the same commit):

- `frame_stack.rs` inline unit tests:
  - `frame_stack_push_when_at_capacity_then_traps_call_stack_overflow`
  - `frame_stack_pop_when_empty_then_returns_none`
  - `frame_stack_when_pushed_and_popped_then_top_tracks_correctly`
- `vm` integration:
  - `execute_when_call_depth_at_max_then_traps_call_stack_overflow`
    (replaces the existing recursion-depth-trap test if needed,
    asserting the trap now comes from frame-stack capacity).
  - `execute_when_user_fb_returns_via_ret_void_then_copy_out_runs`
    — a user-FB body that hits RET_VOID directly; verify
    data-region fields reflect the variable-slot values.
  - `execute_when_user_fb_calls_user_fb_then_both_copy_outs_run_in_order`
    — nested user FB calls; verify both data regions updated.
- Optional sanity bench (`criterion` if available, otherwise skip):
  noop-hook dispatch loop within 1% of pre-PR throughput on the
  existing benchmark suite. Iterative dispatch costs one extra
  bytecode lookup per opcode (via `frames.top()`); the goal is to
  confirm that's noise.

## Out of scope

- DAP server, breakpoints, stepping, pause/resume, `DebuggerHook`,
  `HookAction`, `PauseReason`. Phase 3 of
  `specs/design/debugger-support.md`.
- `header.max_call_depth` computation, codegen plumbing, and the
  acyclicity verifier. The buffer stays sized at `MAX_CALL_DEPTH =
  32` for now. Tightening the bound to the program's actual worst
  case is a follow-up that can land independently.
- `ExecuteOutcome::{Completed, Paused}` enum. Returning `Result<(),
  Trap>` is sufficient for this PR; `Paused` is added when pause/
  resume lands.
- Trait-signature changes on `DebugHook`. `before_instruction` keeps
  its current signature and continues to monomorphize away under
  `NoopDebugHook`.
- Fixing any latent temp-buffer aliasing bug between caller and
  callee frames. The iterative version preserves today's behavior
  exactly via save/restore of the allocator's `next` field.
