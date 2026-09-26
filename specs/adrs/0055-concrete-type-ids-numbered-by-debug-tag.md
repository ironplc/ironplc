# Concrete Type IDs Numbered by the Debug Type Tag

status: accepted
date: 2026-09-26

## Context and Problem Statement

The analyzer identifies a type by its name. The `TypeEnvironment` maps
`TypeName` to `TypeAttributes`, and an expression's type is an
`Option<TypeName>` (ADR-0013). Anything that has to know what a type *is*
looks the name up again, or matches the string against a list of elementary
names.

Identifying types by name leaves three gaps:

* **A type with no name has no identity.** `a : ARRAY[1..2] OF DINT` and
  `e : (X, Y)` never get a type, so every check skips them. Issue #1761 comes
  from this: `LEN(a)`, `ABS(a)` and `n := a` are all accepted.
* **One type has several spellings.** `TOD` and `TIME_OF_DAY` are the same
  type, and a comparison has to know that.
* **Consumers re-derive meaning from strings.** Codegen matched the name to
  pick a debug type tag, and treated any unknown name as an enumeration. The
  second is how a named `ULINT` subrange came to be compiled as a `DINT`.

The container's debug section already identifies elementary types by number:
`iec_type_tag` (ADR-0019) gives `BOOL` = 0 through `LDT` = 24. Aggregates only
get a category tag (`STRUCT` = 25, `ARRAY` = 26, `FB_INSTANCE` = 27), and
everything else is `OTHER` = 255.

How should the compiler identify a type?

## Decision Drivers

* **Every type needs an identity, named or not**, so that no check has a
  reason to skip an expression.
* **Precision.** An identity names exactly one type. A generic category such
  as `ANY_INT` is a set of types.
* **One numbering across the toolchain**, so a built-in type has the same
  number in the analyzer, codegen and the debugger.
* **Names stay available for people.** Diagnostics and debugging should show
  `MyByte`, not a number, even though comparisons never look at the name.
* **The DSL crate stays independent of the analyzer** (ADR-0013).

## Considered Options

* Numeric `TypeId`, with elementary types numbered by `iec_type_tag`
* Numeric `TypeId`, numbered in the order types are met
* Keep `TypeName` as the identity, and generate names for anonymous types

## Decision Outcome

Chosen option: "Numeric `TypeId`, with elementary types numbered by
`iec_type_tag`", because it gives every type an identity without a spelling,
and it reuses the only type numbering that is already shared with the
debugger.

* **`TypeId`** is an opaque `u32` newtype in `ironplc-dsl`
  (`dsl/src/type_id.rs`). It can therefore sit on an `Expr` without the DSL
  depending on the analyzer.
* **The `TypeEnvironment` allocates every `TypeId`.** It stores entries by
  id, with a separate map from names to ids.
* **An elementary type's id is its `iec_type_tag`.** All spellings of one
  elementary type share it. `analyzer/src/type_id.rs` holds the mapping.
* **Ids 25–255 are never allocated**, so an id cannot be confused with an
  aggregate tag or `OTHER`.
* **Every other type gets an id from 256 up**, in the order the environment
  meets it.
* **Identity is nominal.** A type alias (`TYPE MyByte : BYTE`) is a type of
  its own with its own id.
* **Each entry keeps the name it was declared with.** `name_of(id)` answers
  it for diagnostics and debugging. A type's identity is its id, never its
  name.
* **Generic categories get no `TypeId`.** An untyped literal will be typed as
  a literal of a generic category, not by an id.

Codegen derives a variable's debug tag from its type's id: the id itself for
an elementary type, and `OTHER` otherwise. The container format does not
change.

Two things are decided here and will be built later: expressions carrying a
`TypeId`, and anonymous types getting ids. When expressions carry the id,
ADR-0013's `resolved_type: Option<TypeName>` will be amended.

### Consequences

* Good, because an anonymous type can have an identity, which is what #1761
  needs.
* Good, because comparing two types is comparing two integers. Spellings and
  aliases no longer need string handling.
* Good, because the analyzer, codegen and a debugger number an elementary
  type the same way, so codegen's name-matching to pick a debug tag is gone.
* Good, because `name_of` keeps diagnostics and debugging readable.
* Neutral, because the range 25–255 is permanently unused.
* Bad, because an id means nothing without the environment that allocated
  it. Debug output and tests need the environment to show a name.
* Bad, because ids are per compilation. They are not stable identifiers to
  persist or compare across builds.

### Confirmation

* `analyzer/src/type_id.rs` tests:
  * every elementary type's id is a distinct debug tag;
  * `TIME_OF_DAY` and `TOD` share an id, named by the first spelling;
  * a user type gets an id from 256 up and keeps its name;
  * an alias gets an id and name of its own.
* The codegen debug-name tests pass unchanged, so the same tags are emitted.

## Pros and Cons of the Options

### Numeric `TypeId`, elementary types numbered by `iec_type_tag` (chosen)

* Good, because elementary ids match the debugger's numbering with no
  translation table.
* Good, because the reserved range keeps ids and aggregate tags from being
  confused.
* Bad, because the numbering is tied to ADR-0019's tag values. A tag
  renumbering would renumber elementary ids, though ids are never persisted.

### Numeric `TypeId`, numbered in the order types are met

* Good, because it is the simplest allocation.
* Bad, because codegen would still need a separate table to translate
  elementary types into debug tags.

### Keep `TypeName`, and generate names for anonymous types

* Good, because no data structure changes.
* Bad, because identity stays a string. Generated names must never collide
  with a user's, and distinct anonymous types with the same shape (two
  `(X, Y)` enumerations) need disambiguating suffixes. That is an id written
  as text.
* Bad, because consumers keep re-deriving meaning from names.

## More Information

* ADR-0013: Expression Type Annotation via Wrapper Struct
* ADR-0019: Type Encoding in Debug Variable Names
