# VM engine consumes `ContainerRef` only

Tracked by [issue #1573](https://github.com/ironplc/ironplc/issues/1573)
(finish ADR-0010). This plan is the first of the core changes that issue
lists; the rest (caller-provided buffer slices, an injected clock, `libm`,
the breakpoint table, the `#![no_std]` attribute and the cross-compile gate)
stay on the issue.

## Goal

`ironplc-vm`'s execution engine (`Vm::load`, `VmReady`, `VmRunning`, the
dispatch loop) reads the loaded program through the zero-copy
`ContainerRef` view and never through the owned, `Vec`-backed `Container`.
After this change the engine has no code path that needs the container
crate's `std` feature; what still does (`VmBuffers`, the freewheeling
rewrite, test support) is host-side convenience, and the issue tracks
moving each of those.

## Architecture

`ContainerRef<'a>` already exists in `ironplc-container` as the `no_std`
parse path ADR-0010 designed, but nothing uses it and it exposes only part of
what the engine reads. The engine reads, from the owned container:

| Engine use | Owned accessor today | `ContainerRef` after this change |
|---|---|---|
| Function bytecode | `code.get_function_bytecode(id)` | `get_function_bytecode(id)` (exists) |
| Callee locals/params | `code.get_function(id)` → `FuncEntry` | `function_entry(id)` → `FuncEntry` |
| Constants | `constant_pool.get_{i32,i64,f32,f64,str}`, `char_width` | `get_{i32,i64,f32,f64,str}_constant`, `constant_char_width` |
| Array bounds and strides | `type_section.array_descriptors[i]` | `array_descriptor(i)` |
| User FB dispatch | `type_section.user_fb_types.find(type_id)` | `user_fb_type(type_id)` |
| Task and program tables | `task_table.tasks`, `.programs`, `.shared_globals_size` | `task_entries()`, `program_entries()`, `shared_globals_size()` |
| Runtime parameters and flags | `header.*` | `header().*` (exists) |

`Vm::load` takes the view by value: `load(container: ContainerRef<'a>,
bufs: &'a mut VmBuffers)`. The VM state types own the view for `'a`. A host
that also needs the debug section keeps its owned `Container` alongside, as
the debugger and the `run` MCP tool already do.

A host needs somewhere to keep the bytes and the constant-offset scratch that
`ContainerRef::from_slice` borrows. `ContainerBytes` (container crate, `std`
feature) owns both, validates once at construction, and hands out views with
`container_ref(&self)`; a shared borrow, so a benchmark harness can build a
view per iteration and a session struct can hold it next to its buffers. The
embedded path is unchanged: `ContainerRef::from_slice` over flash-resident
bytes and a caller-sized offsets array.

The fixed-size table entries the engine reads (`FuncEntry`, `TaskEntry`,
`ProgramInstanceEntry`, `ArrayDescriptor`, `UserFbDescriptor`) become the
one definition of their on-disk layout: each gains `SIZE`, `from_bytes` and
`to_bytes`, the owned readers and writers use them, and `ContainerRef`
decodes through them. `TaskEntryRef` and `ProgramEntryRef`, which duplicated
`TaskEntry` and `ProgramInstanceEntry` field for field (and mislabelled the
program entry's `init_function_id` as `reserved`), go away.

## Prefactoring

Behaviour-preserving, in its own commit, before any engine change:

1. Name the header flag bits (`FLAG_HAS_DEBUG_SECTION`,
   `FLAG_HAS_TYPE_SECTION`) next to `FLAG_HAS_SYSTEM_UPTIME`; `Container`
   uses them in place of the `0x02` / `0x04` literals. `ContainerRef` needs
   the type-section bit and must not carry a second literal.
2. Move the fixed-size entry structs out from behind the container crate's
   `std` gate. The modules `task_table`, `code_section` and `type_section`
   compile always; only the `Vec`-bearing tables (`TaskTable`, `CodeSection`,
   `TypeSection`, `FbTypeDescriptor`) and their `Read`/`Write` code stay
   behind `#[cfg(feature = "std")]`. Each entry struct gains `SIZE`,
   `from_bytes` and `to_bytes`; the owned `read_from` / `write_to` call
   them. Existing tests pass unchanged.
3. `ContainerRef::task_entry` / `program_entry` return `TaskEntry` /
   `ProgramInstanceEntry`; delete `TaskEntryRef` and `ProgramEntryRef`. The
   existing `container_ref` tests read the same field names, so they pass
   unchanged.

Signals that called for it: the type-section lookup appears seven times in
the dispatch loop as the same three-call chain; the same byte layout is
parsed in two places (`task_table.rs` and `container_ref.rs`), which is how
the `reserved` / `init_function_id` mismatch slipped in.

## Design doc reference

- [no_std VM Design](../design/no-std-vm.md), updated by this change so the
  embedded usage sketch matches the real `Vm::load` signature and the host
  path is described.
- [ADR-0010](../adrs/0010-no-std-vm-for-embedded-targets.md), Implementation
  Status amended to record what this change closes and what remains.

## File map

Container crate (`compiler/container/src/`):

- `header.rs` — flag constants.
- `container.rs` — use the flag constants.
- `task_table.rs`, `code_section.rs`, `type_section.rs` — un-gate the entry
  structs, add `SIZE` / `from_bytes` / `to_bytes`, readers and writers use
  them.
- `container_ref.rs` — decode via the shared entry types; validate the task
  table fully at `from_slice` (entry counts fit, every task type decodes) so
  `task_entries()` / `program_entries()` are infallible; slice the type
  section; add `function_entry`, the remaining constant getters,
  `array_descriptor`, `user_fb_type`; tests for each.
- `container_bytes.rs` (new, `std`) — `ContainerBytes`.
- `lib.rs` — module gates and re-exports.

VM crate (`compiler/vm/src/`):

- `vm.rs` — `ContainerRef` in the state types and `execute*`; accessor
  substitutions in the dispatch loop; `Vm::load` iterates the view's
  entries.
- `buffers.rs` — `VmBuffers::from_container(&ContainerRef)`.
- `test_support.rs` — `load_and_start(ContainerRef, &mut VmBuffers)`;
  `run_and_read_*` build a `ContainerBytes`; re-export `ContainerBytes`.
- `tests/it/*.rs` and the inline tests in `vm.rs` — mechanical: serialize
  the fixture, take a view, load the view.

Hosts:

- `compiler/vm-cli/src/cli.rs` — read the file bytes once; owned `Container`
  for `--dump-vars` rendering, `ContainerBytes` for the VM.
- `compiler/vm-cli/src/dap/launch.rs`, `dap/server.rs` — serialize the
  (possibly rewritten) container into `ContainerBytes`; `start_vm` takes the
  view.
- `compiler/ironplc-cli/src/lsp_runner.rs`, `compiler/playground/src/lib.rs`
  — sessions keep a `ContainerBytes`; the owned parse remains only where the
  variable renderer needs the debug section.
- `compiler/mcp/src/runner.rs` — serialize after `resolve_cycle_time`.
- `compiler/benchmarks/benches/st_benchmark.rs`, `tests/profile_for_loop.rs`
  — build one `ContainerBytes` per benchmark, a view per iteration.
- `compiler/codegen/src/stack_balance.rs`, `spec_conformance*.rs`,
  `tests/it/common/mod.rs` and the end-to-end tests that size buffers by
  hand.

Specs:

- `specs/design/no-std-vm.md`, `specs/adrs/0010-no-std-vm-for-embedded-targets.md`.

## Tasks

- [ ] Prefactor: flag constants; entry structs shared between the owned
      tables and `ContainerRef`; `TaskEntryRef` / `ProgramEntryRef` removed.
      `cd compiler && just` green.
- [ ] `ContainerRef`: full task-table validation, type section, function
      entries, all constant kinds; `ContainerBytes`. Unit tests for each
      accessor including the out-of-range and wrong-type paths.
- [ ] Engine: `Vm::load` / state types / dispatch loop on `ContainerRef`;
      `VmBuffers::from_container(&ContainerRef)`; test support.
- [ ] Hosts and tests updated; `cd compiler && just` green (compile,
      coverage ≥ 85 %, clippy, fmt, dupes).
- [ ] Spot-check `cargo bench -p ironplc-vm` on one dispatch-heavy benchmark
      against `main` — the function-directory and constant reads now decode
      from bytes per access, and the change must not regress dispatch.
- [ ] Amend ADR-0010 Implementation Status and the design doc's usage sketch.
- [ ] `git rm` this plan.
