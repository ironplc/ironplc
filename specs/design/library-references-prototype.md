# Design Prototype: Library References and External Functions

> **Status: prototype / exploratory.** This is a sketch of an approach to
> pressure-test, not a committed design. It exists to make one idea concrete
> enough to argue about. Details are provisional and several open questions
> (below) would need answering before any of this is specified or built.

## What this is exploring

Vendor toolchains ship their standard functions in *libraries* — Beckhoff's
`Tc2_Standard`, `Tc2_Math`, and so on. A TwinCAT project doesn't enable those
with a compiler switch; it **references** a library. Recent work
([#1217](https://github.com/ironplc/ironplc/pull/1217), part of #1199) instead
adds individual library functions like `LTRUNC`/`LMOD` behind per-function
`--allow-*` flags. That works for one or two functions but feels like the wrong
shape, and this note is an attempt to sketch a shape that might be righter.

The hunch being prototyped:

> **The compiler should grow *primitives*, not *functions*.** IronPLC provides
> the library functions *outside* the compiler (bound to a low-level primitive,
> or written in ST), and exposes just enough opcodes to make that possible. A new
> library function needs a compiler change only when it needs a primitive the VM
> doesn't yet have.

If that hunch holds, the compiler stops caring about vendor function catalogs
entirely — it owns a small primitive vocabulary and a way to reference libraries,
and everything else is content.

## The distinction the sketch rests on

Three kinds of "function," which today all get lumped together:

- **Compiler-only intrinsic** (`SIZEOF`) — reads compile-time info, *cannot* be
  written in ST. Genuinely belongs in the compiler.
- **External library function** (`LTRUNC`, `LMOD`) — a plain operation
  (`LREAL`→`LREAL` truncation; floating-point modulo) that just needs a VM
  primitive. The *function* is a name + signature; the *operation* is an opcode.
- **Source library POU** (most of any real library) — ordinary ST built from
  primitives and other POUs. Needs no compiler involvement at all.

The per-function flag treats the middle category like the first. `SIZEOF`-as-a-
flag is right; `LTRUNC`-as-a-flag puts a plain math function on the intrinsic
side of the line and still ships no implementation (it type-checks, then hits
`Diagnostic::todo` at codegen because there's no opcode behind it).

## A concrete sketch: `LTRUNC` / `LMOD`

Walking the two motivating functions through the idea, to see what it would
actually look like.

**1. Add the primitives (compiler/VM, once).** IronPLC's VM has no float→float
truncation and no float modulo (checked). Under the existing unified `BUILTIN`
opcode ([ADR-0008](../adrs/0008-unified-builtin-opcode.md)) these are new
`func_id`s + VM dispatch arms — not new opcode slots:

- `TRUNC_F64` / `TRUNC_F32` — truncate toward zero, stay float.
- `MOD_F64` / `MOD_F32` — floating remainder, `MOD_F64(400.56, 360.0) = 40.56`.

**2. Provide the functions outside the compiler.** A `Tc2_Math` library
definition declares them as thin bindings to those primitives — roughly:

```
{external := 'TRUNC_F64'}
FUNCTION LTRUNC : LREAL  VAR_INPUT IN : LREAL; END_VAR  END_FUNCTION
```

Codegen sees the `external` marker and inlines the `BUILTIN` at the call site
(the way `compile_trunc` already inlines its conversion) — no `CALL`, no stack
frame. `LTRUNC`/`LMOD` are 1:1 aliases, so there's no ST body to write; a
function *with* logic would instead be plain ST over primitives.

**3. Make it available by reference.** A project that references `Tc2_Math` gets
them in scope; codegen emits the opcode; they run. Adding the next `Tc2_Math`
function that needs float-trunc/mod is then pure library content — no compiler
change, no new flag.

That's the whole idea in miniature: **two primitives + a library alias**, versus
a flag per function.

## Why this seems plausible in IronPLC specifically

The expensive parts already exist, which is part of why the sketch feels worth
pursuing:

- `analyze()` already merges multiple parsed units and resolves them as one set.
- `xform_toposort_declarations` already prunes to what's reachable from PROGRAM
  roots — so a big library can be supplied and only used POUs are emitted.
- The unified `BUILTIN` opcode + `lookup_builtin` name→`func_id` table is already
  "opcodes surfaced to ST by name" (`ABS`, `SQRT`, `SIN`…). Adding a primitive is
  a well-worn path.
- TwinCAT files and `.plcproj` are already parsed.

So the missing pieces are mostly about *identity and scoping*, not linking.

## Open questions (what a prototype would need to resolve)

These are genuinely unresolved and would shape any real design:

1. **Binding mechanism.** External-declaration attribute (`{external := ...}`) vs
   a reserved `__intrinsic` namespace that library ST calls? Aliases favor the
   former; functions-with-logic favor the latter. Maybe both — unclear.
2. **Reference gating.** Today the merged environment is one flat namespace, so a
   symbol is visible whether or not its library was referenced. Getting the
   "used `LMOD` but didn't reference `Tc2_Math`" diagnostic means tagging entries
   with their owning library and gating lookup — how invasive is that in
   `function_environment`?
3. **Where references come from.** `.plcproj` carries `<LibraryReference>` (not
   parsed today); non-TwinCAT projects have no manifest at all. What's the native
   story?
4. **Consuming the user's libraries without shipping them.** Resolving a
   reference against the user's *installed* copy avoids redistribution, but means
   reading `.library` archives (packaged POU XML) or PLCopen XML exports, plus a
   search-path/repository notion and a graceful "not found" path. Protected
   `.compiled-library` files have no source — signatures-only at best. Is this in
   scope or deferred?
5. **Primitive semantics as a contract.** Once library source is written against
   a primitive name, that name is frozen like an ABI. Does `lookup_builtin` need
   to become a declared, versioned table rather than a hardcoded `match`?
6. **Does the alias want to be a `CALL` or an inline?** Inlining is free for
   1:1 aliases but doesn't generalize to logic-bearing POUs; the two tiers may
   need different treatment.

## A first thing to try

If we wanted to validate the hunch cheaply: add `TRUNC_F64`/`MOD_F64` as
primitives, wire up the `external`-binding path in codegen, and express
`LTRUNC`/`LMOD` as a tiny `Tc2_Math` alias definition selected however is easiest
for now (even a temporary dialect toggle). If those two functions **run**
end-to-end via that route — instead of type-checking and dying at codegen —
the shape is worth investing in, and the reference/scoping machinery (questions
2–4) becomes the next thing to prototype. If it's awkward, better to learn that
on two functions than on a reference subsystem.

## Leans on

- [ADR-0008](../adrs/0008-unified-builtin-opcode.md) — unified `BUILTIN` opcode
  (the primitive-extension surface).
- [ADR-0003](../adrs/0003-plc-standard-function-blocks-as-intrinsics.md) —
  intrinsics recognized at dispatch.
- [Beckhoff TwinCAT Dialect](beckhoff-twincat-dialect.md) — parses the files;
  this is a sketch toward the semantic follow-on (#1199).
