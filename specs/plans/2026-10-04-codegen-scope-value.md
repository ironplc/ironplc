# Plan: Introduce a `Scope` value for a body's variable mappings

## Goal

Give "the variable mappings one body is compiled against" a type of its own.
Today those mappings are eight loose fields on `CompileContext`, saved and
restored by hand at three sites in `compiler/codegen`. This change adds a
`Scope` value that holds them, a `CompileContext::swap_scope` that installs one
wholesale, and two pure constructors that build the scope a function body and a
function block body see. Two of the three hand-written sites move onto it.

This plan delivers **one prefactor PR** and nothing else. It is behaviour
preserving: emitted bytecode is byte-for-byte identical, and no existing test is
edited. The change it prepares the ground for has its own plan; this one does not
describe it and does not depend on it.

## Architecture

A new module `compiler/codegen/src/scope.rs` holds:

- `Scope` (`#[derive(Default)]`) with exactly the eight collections that live on
  `CompileContext` today, with unchanged key and value types: `variables`,
  `var_types`, `string_vars`, `array_vars`, `struct_vars`, `struct_array_vars`,
  `fb_instances`, `in_out_params`.
- `impl CompileContext { pub(crate) fn swap_scope(&mut self, scope: Scope) -> Scope }`,
  which `std::mem::replace`s all eight fields and returns the previous contents.
  It lives in `scope.rs`, so `compile.rs` (already over the 1000-line limit) does
  not grow. **The fields stay on `CompileContext` and no access site changes**:
  `ctx.variables`, `ctx.string_vars` and the rest are untouched everywhere else.
- `Scope::for_function_body(program: &Scope, num_globals: u16) -> Scope` and
  `Scope::for_fb_body(program: &Scope, num_globals: u16) -> Scope`. Both are pure.
  `num_globals` is `ctx.variables.len()` after the globals are assigned
  (`compile_program_with_functions`, just after `assign_variables` for the
  globals).

### The semantics, pinned

The two sites are not the same, and the constructors reproduce each as written.
They are not unified.

| | `variables`, `var_types`, `string_vars`, `struct_vars`, `struct_array_vars` | `array_vars` | `fb_instances` | `in_out_params` | Restore |
|---|---|---|---|---|---|
| `for_function_body` (`compile_user_function`) | globals only (see below) | empty | clone of the program-level map | empty, then filled by the body's `VAR_IN_OUT` declarations | at the end of the function |
| `for_fb_body` (`compile_user_function_block`) | globals only (see below) | empty | empty | clone of the program-level set | not by the function; the driver restores after the type's methods are compiled |

"Globals only": `variables` keeps the entries whose index is `< num_globals`.
Each of the other four keeps the entries whose *id* maps, in the program-level
`variables`, to an index `< num_globals`. An entry whose id is absent from
`variables` is dropped.

Where today's code leaves a collection untouched (`fb_instances` at the function
site, `in_out_params` at the FB site), the body sees the program-level one, so the
constructor clones it. `swap_scope` swaps all eight, so each constructor must put
into the `Scope` exactly what the body sees today.

### Rewriting the sites

A constructor needs the program-level `&Scope`, which only exists once it has been
swapped out. Each site therefore does:

```rust
let program = ctx.swap_scope(Scope::default());
ctx.swap_scope(Scope::for_function_body(&program, num_globals));
// ... assign slots, compile the body ...
ctx.swap_scope(program);
```

The second call returns the empty default scope, which is dropped. Nothing runs
between the two calls, so the intermediate empty state is unobservable.

- `compile_user_function`: the take/re-insert block (~47 lines) and the restore
  block (8 lines) become the three lines above.
- `compile_user_function_block`: the same take/re-insert block becomes
  `swap_scope(Scope::default())` plus `swap_scope(Scope::for_fb_body(..))`. It
  returns `(CompiledFunction, Scope)` where the `Scope` is the program-level one,
  in place of `SavedFbScope`. The driver in `compile_program_with_functions`
  replaces its seven-line restore with `ctx.swap_scope(program_scope)`.
- `SavedFbScope` is deleted.

On an `Err` return, both old and new code leave `ctx` holding the body's mappings
and drop the program-level ones. Compilation aborts on that error, so this is
unchanged and not observable.

### What was checked

- **Nothing outside the two paths inserts into `fb_instances` or `in_out_params`.**
  `grep` over `compiler/` finds `fb_instances.insert` only in `compile_setup.rs`
  (both inside `assign_variables`), and `assign_variables` is called only from
  `compile_program_with_functions` for the globals and for the program locals,
  both before any function or FB body is compiled. `in_out_params.insert` occurs
  only in `compile_user_function`. So replacing "untouched" with "cloned, then
  swapped back at the end" cannot drop an insertion that exists today.
- `compile_user_fb_methods` clones and restores `variables` and `var_types` around
  each method. It runs between the FB body and the driver's restore, while the FB
  scope is installed in the fields exactly as today. It is not touched.
- `FbInstanceInfo` has no `Clone` derive. `for_function_body` needs one to clone
  the program-level `fb_instances`, so the PR adds `#[derive(Clone)]` to it (one
  line in `compile.rs`). `ArrayVarInfo` stays non-`Clone`: `array_vars` is never
  copied.

### Quirks reproduced, not fixed

These are existing behaviour. The PR description lists them and changes none.

- `array_vars` is never copied into either kind of body, so a global array is not
  visible inside a function or FB body.
- The global filter is by the index recorded in `variables`, not by where the
  entry was declared, and `num_globals` is the *length* of the `variables` map.
- A function body sees the program-level `fb_instances`, including instances
  declared in the program's own `VAR` block, while its `variables` holds only the
  globals.
- An FB body sees the program-level `in_out_params`, not an empty set.

## Prefactoring

None beyond this change itself: it *is* the prefactor. It is bounded by what it
leaves alone: the three `ctx.<map>` access patterns, `compile_user_fb_methods`,
`UserFunctionInfo`, `UserFbTypeInfo`, `UserMethodInfo`, `ParamPassing`,
`data_region::reserve`, slot assignment, and the collection types are all
unchanged. The two constructors share one private helper that copies the five
globals-filtered collections, because the "globals copied back" column of the table
is identical for both; the columns that differ (`fb_instances`, `in_out_params`)
stay explicit in each constructor, so the sites are still not unified.

## Design doc reference

None. This is an internal reshaping of codegen state with no externally visible
requirement, so no `REQ-` IDs apply. Rationale that is local to the code (what each
constructor carries over, and why the FB scope is returned rather than restored)
goes in doc comments on `Scope` and its constructors, not in `specs/`.

## File map

| File | Change |
|------|--------|
| `compiler/codegen/src/scope.rs` | New: `Scope`, `swap_scope`, the two constructors, the shared global filter, unit tests |
| `compiler/codegen/src/lib.rs` | Register `mod scope;` |
| `compiler/codegen/src/compile_fn.rs` | Both sites use `Scope`; the take/re-insert/restore blocks go; `compile_user_function_block` returns `(CompiledFunction, Scope)` |
| `compiler/codegen/src/compile.rs` | Delete `SavedFbScope`; driver restore becomes `ctx.swap_scope(..)`; `#[derive(Clone)]` on `FbInstanceInfo`; net lines go down |

Not touched: `compile_method.rs`, `compile_setup.rs`, every `ctx.<map>` access site,
and everything under `compiler/codegen/tests/`.

## Tasks

### Prefactor PR: `Scope` for a body's variable mappings

- [ ] Add `scope.rs` with `Scope`, `CompileContext::swap_scope`, `for_function_body`,
      `for_fb_body`, and the private globals filter; register the module
- [ ] Derive `Clone` on `FbInstanceInfo`
- [ ] Unit tests in `scope.rs`, BDD-named, one scenario each. The program scope holds
      entries below *and above* `num_globals`, including one exactly at
      `num_globals` and an entry in `var_types` whose id is absent from `variables`:
  - [ ] `for_function_body`: globals kept and non-globals dropped in each of the five
        filtered collections; `array_vars` empty although the program has one;
        `fb_instances` cloned unfiltered; `in_out_params` empty although the program
        has entries
  - [ ] `for_fb_body`: the same five filtered collections; `array_vars` empty;
        `fb_instances` empty although the program has entries; `in_out_params`
        cloned unfiltered
  - [ ] `swap_scope`: installs all eight collections, returns the previous eight,
        and swapping back restores the original
- [ ] Rewrite `compile_user_function` onto `Scope`; delete its take/re-insert and
      restore blocks
- [ ] Rewrite `compile_user_function_block` onto `Scope`; return the program-level
      `Scope`; delete its take/re-insert block and `SavedFbScope`
- [ ] Replace the driver's restore with `ctx.swap_scope(..)`
- [ ] Confirm `compile.rs` and `compile_fn.rs` both shrink (`git diff --stat`) and
      `scope.rs` is under 1000 lines
- [ ] Prove the bytecode is identical beyond the test suite: build the compiler from
      `main` and from the branch, compile every `.st` file in the repository with
      each, and `cmp` the produced containers. First compile twice on `main` to
      establish that the output is deterministic; if the container embeds anything
      that varies per run, strip it from both sides before comparing
- [ ] `cd compiler && just` passes, including 85% coverage; `cd specs && just` passes
- [ ] PR description lists the quirks above and anything else noticed but not changed

### Core change PRs

None. The change this prefactor serves is planned separately.

## For the reviewer to confirm

1. The sites use the two-call form (`swap_scope(Scope::default())`, then
   `swap_scope(constructor(&program, ..))`), because the task fixes the
   constructors as taking the program-level `&Scope`. A fused helper would
   save a line per site but adds an API nobody asked for.
2. The two constructors share a private helper for the five filtered collections.
   This is the one place they are written together; the differing collections are
   still spelled out in each.
3. `#[derive(Clone)]` on `FbInstanceInfo` is the only change outside the stated
   scope, and it is required by "clone the program-level map".
