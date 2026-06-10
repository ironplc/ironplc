# Compute `max_call_depth` in Codegen and Validate at VM Load

## Goal

Tighten the artificial `MAX_CALL_DEPTH = 32` bound landed in
`compiler/vm/src/vm.rs` to each program's actual worst case:

1. Codegen builds the static call graph during emission, runs a
   3-color DFS to detect cycles, and writes the longest path to
   the already-reserved `FileHeader.max_call_depth` field (bytes
   194–195, currently always 0 — see `header.rs:67`, `header.rs:153`).
2. `Vm::load` rejects containers whose `header.max_call_depth`
   exceeds the embedder's frame-stack capacity, returning a typed
   load error rather than waiting for runtime `CallStackOverflow`.

Both halves are small. The PR ships a real safety improvement
(fail-fast on a buggy container, plus codegen-side cycle detection
as a backstop) and closes the loose end the Phase 2 plan explicitly
deferred.

## Why now

The Phase 2 PR (`specs/plans/2026-05-25-iterative-vm-dispatch.md`)
deferred this work intentionally: it would have inflated the
dispatch rewrite. With the rewrite landed, the field is reserved
in the header and unused; computing and validating it is now a
self-contained follow-up.

It also unblocks Phase 3 (DAP debugger): every paused VM needs a
trustworthy upper bound on how many frames `stackTrace` can be
asked to walk.

## Scope

In:

- `compiler/codegen/src/`: collect call-graph edges during emission;
  compute longest path; emit error on cycles; pass depth to builder.
- `compiler/container/src/builder.rs`: opt-in `.max_call_depth(u16)`
  setter that propagates to `FileHeader.max_call_depth`.
- `compiler/vm/src/vm.rs`: validate `header.max_call_depth ≤
  frames.len()` in `Vm::load`, returning a typed error.

Out (deferred):

- Sizing `VmBuffers.frames` from `header.max_call_depth`. The buffer
  stays at fixed `MAX_CALL_DEPTH = 32` so this PR doesn't break
  embedder-side memory budgeting. Tightening the buffer is a
  follow-up that depends on `VmBuffers::from_container` reading
  the header field.
- Any container-format change. The `max_call_depth` field is
  already reserved (`header.rs:67`); no version bump.
- Updating hand-built test containers in `compiler/vm/tests/it/` to
  set `max_call_depth`. They keep the default (0), which means "not
  computed" and disables the load-time check. This preserves all
  existing VM tests — including the
  `execute_when_call_recursion_exceeds_max_depth_then_traps_call_stack_overflow`
  test, which depends on the runtime `Trap::CallStackOverflow`
  firing at frame 33.

## Architecture

### `ContainerBuilder::max_call_depth(u16)`

Trivial pass-through. The existing `max_stack_depth` setter
(`builder.rs:161`) is the precedent. Default stays 0.

```rust
pub fn max_call_depth(mut self, n: u16) -> Self {
    self.max_call_depth = n;
    self
}
```

`build()` writes the value into the `FileHeader` it constructs at
`builder.rs:365`.

### Call-graph collection during emission

The two emission sites are:

- `compile_call.rs:316` — user function `CALL` (has `function_id`).
- `compile_stmt.rs:495–530` — user-FB `FB_CALL` (has `type_id`; the
  callee's `function_id` is in `ctx.user_fb_types[name].function_id`,
  available at the same site).

Both sites already run inside `CompileContext`. Add:

```rust
pub struct CompileContext {
    // ... existing fields ...

    /// Adjacency: caller function id -> set of callee function ids.
    /// Populated by `emit_call` and the FB_CALL emission site.
    pub call_graph: HashMap<FunctionId, HashSet<FunctionId>>,

    /// Function currently being emitted. Pushed by `compile_function`
    /// (and the FB-body equivalent), popped after the body emits.
    pub current_function: Option<FunctionId>,
}
```

Edge insertion is a one-liner at each emit site:

```rust
if let Some(caller) = ctx.current_function {
    ctx.call_graph
       .entry(caller)
       .or_default()
       .insert(callee);
}
```

### Longest-path + cycle detection

After every function/FB body is emitted, run a 3-color DFS from the
entry function id (`FunctionId::SCAN`):

```rust
fn longest_path_or_cycle(
    graph: &HashMap<FunctionId, HashSet<FunctionId>>,
    entry: FunctionId,
) -> Result<u16, FunctionId /* node in the cycle */> {
    // Color: 0 = white, 1 = gray (on stack), 2 = black (done).
    let mut color: HashMap<FunctionId, u8> = HashMap::new();
    let mut depth: HashMap<FunctionId, u16> = HashMap::new();

    fn dfs(
        node: FunctionId,
        graph: &HashMap<FunctionId, HashSet<FunctionId>>,
        color: &mut HashMap<FunctionId, u8>,
        depth: &mut HashMap<FunctionId, u16>,
    ) -> Result<u16, FunctionId> {
        match color.get(&node).copied().unwrap_or(0) {
            1 => return Err(node),        // back edge -> cycle
            2 => return Ok(depth[&node]), // memoized
            _ => {}
        }
        color.insert(node, 1);
        let mut max_child = 0u16;
        if let Some(callees) = graph.get(&node) {
            for &c in callees {
                max_child = max_child.max(dfs(c, graph, color, depth)?);
            }
        }
        let d = max_child + 1;
        depth.insert(node, d);
        color.insert(node, 2);
        Ok(d)
    }

    dfs(entry, graph, &mut color, &mut depth)
}
```

Depth semantics: `dfs(entry)` returns the number of stack frames at
the deepest point, counting the entry frame. A leaf returns 1; a
chain of N calls returns N+1.

The recursion ban in semantic analysis already rejects cyclic
programs, so the codegen check is a backstop. If a cycle ever
slips through, emit a `Diagnostic` with code `P0XYZ` (new) and
position pointing at the function declaration whose body
introduced the back edge. Failing closed is correct — a cyclic
program would loop forever in this DFS otherwise.

### Codegen integration point

Right before `ContainerBuilder::build()` is called in
`compile.rs:244–255`:

```rust
let depth = longest_path_or_cycle(&ctx.call_graph, FunctionId::SCAN)
    .map_err(|cycle_node| {
        Diagnostic::new(
            "P0XYZ",
            format!("call graph contains a cycle at function {}", cycle_node),
            // ... source position ...
        )
    })?;
builder.max_call_depth(depth);
let mut container = builder.build();
```

The exact `builder.build()` call site is inside
`compile_program_with_functions`. Wiring the depth call has to
happen wherever the builder is finalized.

### `Vm::load` validation

Today `Vm::load` is infallible:

```rust
pub fn load<'a>(self, container: &'a Container, bufs: &'a mut VmBuffers) -> VmReady<'a>
```

The cheapest change is to **leave the signature alone** and add the
validation to `VmReady::start()`, which already returns
`Result<VmRunning<'a>, FaultContext>` (`vm.rs:147`). The check runs
before any init bytecode executes:

```rust
pub fn start(mut self) -> Result<VmRunning<'a>, FaultContext> {
    let declared = self.container.header.max_call_depth as usize;
    if declared > 0 && declared > self.frames.len() {
        return Err(FaultContext {
            trap: Trap::ProgramExceedsCallDepth {
                required: self.container.header.max_call_depth,
                capacity: self.frames.len() as u16,
            },
            task_id: TaskId::DEFAULT,
            instance_id: InstanceId::DEFAULT,
        });
    }
    // ... existing init-execution loop ...
}
```

The `declared > 0` guard preserves backward compatibility with
hand-built test containers that don't populate the field.

Adds one Trap variant:

```rust
// in compiler/vm/src/error.rs
ProgramExceedsCallDepth { required: u16, capacity: u16 },
```

with a `Display` impl mentioning both numbers so the embedder
knows whether to upgrade the buffer or recompile with fewer calls.

### `FunctionId::SCAN` and entry resolution

`FunctionId::SCAN` is the entry by convention. The depth computation
uses it directly. If a future container ever changes the entry,
read it from `task_table.programs[0].entry_function_id` instead —
mention this in a code comment so the future move is obvious.

## File map

Modified:

- `compiler/codegen/src/compile.rs` — add `call_graph` /
  `current_function` to `CompileContext`; wire depth computation
  into the `compile_program_with_functions` finalization path.
- `compiler/codegen/src/compile_call.rs` — record edge on `emit_call`.
- `compiler/codegen/src/compile_stmt.rs` — record edge on user-FB
  `FB_CALL` emission.
- `compiler/codegen/src/lib.rs` — re-export new helper if extracted
  to its own module (see below).
- `compiler/container/src/builder.rs` — add `max_call_depth` field
  + setter + propagation in `build()`.
- `compiler/vm/src/vm.rs` — add `max_call_depth` validation in
  `VmReady::start()`.
- `compiler/vm/src/error.rs` — add `ProgramExceedsCallDepth` Trap
  variant with `Display` impl.
- `docs/compiler/problems/` — new problem doc for the cycle
  diagnostic (per CLAUDE.md problem-code policy).

New (optional, only if the helper grows past ~30 lines):

- `compiler/codegen/src/call_graph.rs` — `longest_path_or_cycle` and
  any related types kept out of `compile.rs` to manage module size.

Not modified:

- Container format / wire encoding. The header field already exists.
- `compiler/vm/src/buffers.rs`. `VmBuffers::from_container` still
  sizes frames at `MAX_CALL_DEPTH`. Tightening the buffer to the
  header value is a follow-up.
- `compiler/vm/src/frame_stack.rs`. Phase 2's FrameStack works as-is.
- VM trap behavior on runtime overflow. `Trap::CallStackOverflow`
  still fires from `FrameStack::push` if (against the header's
  declaration) a program tries to push past the buffer.

## Migration

Single commit. The work is mechanical:

1. Add `Trap::ProgramExceedsCallDepth` and its `Display`.
2. Add `ContainerBuilder::max_call_depth` setter.
3. Add `CompileContext.call_graph` and `current_function`; push/pop
   `current_function` around each body emission.
4. Insert one-line edge records at the two emit sites.
5. Implement `longest_path_or_cycle`.
6. Wire it into the builder finalization.
7. Add `VmReady::start()` validation.
8. Add tests (below).
9. `cd compiler && just`.

If any existing test fails, that's the signal to investigate — most
likely a hand-built test container that does set `max_call_depth`
but has bytecode going deeper. Resolve by either fixing the test's
declared depth or fixing the bytecode.

## Tests

Codegen (`compiler/codegen/tests/it/`):

- `compile_when_program_has_no_calls_then_max_call_depth_is_one`
  — a program whose `SCAN` only does arithmetic. Depth = 1.
- `compile_when_program_calls_one_function_then_max_call_depth_is_two`
  — `SCAN → FOO`. Depth = 2.
- `compile_when_call_chain_three_deep_then_max_call_depth_is_four`
  — `SCAN → A → B → C`. Depth = 4.
- `compile_when_diamond_call_graph_then_max_call_depth_takes_longest`
  — `SCAN → {A, B}; A → C; B → D → E`. Depth = 4 (SCAN→B→D→E).
- `compile_when_program_calls_user_fb_then_fb_body_counted`
  — `SCAN → FB.body`. Depth = 2.
- `compile_when_user_fb_calls_user_fb_then_both_counted`
  — `SCAN → outerFB.body → innerFB.body`. Depth = 3.
- `compile_when_call_graph_has_cycle_then_codegen_returns_diagnostic`
  — synthesized cycle (semantic analysis is bypassed via a unit
  test that injects a fake edge into the graph, since real cycles
  are rejected earlier).

VM (`compiler/vm/tests/it/`):

- `start_when_container_declares_call_depth_exceeding_buffer_then_returns_program_exceeds_call_depth`
  — build a container with `max_call_depth = 64` via the builder
  setter, default `VmBuffers` (frames = 32). `start()` returns the
  new Trap variant with `required = 64, capacity = 32`.
- `start_when_container_declares_zero_call_depth_then_no_validation_runs`
  — legacy container path: no `.max_call_depth` set → default 0 →
  start succeeds (back-compat).
- `start_when_container_declares_call_depth_within_buffer_then_succeeds`
  — `.max_call_depth(16)` with default 32-cap buffer → start
  succeeds.

Container (`compiler/container/`):

- `builder_when_max_call_depth_set_then_propagates_to_header` — the
  setter end-to-end.

Conformance (`compiler/container/src/spec_conformance.rs`): no
changes — the existing field-offset test already covers bytes
194–195.

Problem doc (`docs/compiler/problems/`):

- New `P0XYZ.rst` for the cycle diagnostic. Title, description,
  example, "how to fix" sections per the CLAUDE.md template.

## Out of scope

- Resizing `VmBuffers.frames` from the container header. Stays
  fixed at `MAX_CALL_DEPTH = 32`. Tightening the buffer to the
  header value is a follow-up and unlocks per-program memory
  budgeting on embedded targets.
- Reporting depth in a CLI or debug-info dump. Useful but not
  required.
- Adding the depth check to `VmReady::resume()` (a separate
  start-from-saved-state path). `resume()` doesn't run init and
  the validation can move into it later if/when its use grows.
- Tracking which FB/function declaration introduced the deepest
  path for diagnostic purposes. Today the trap reports numbers,
  not call paths.
