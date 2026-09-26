# The parser records a user-typed initializer as written; the type resolver classifies it

status: accepted
date: 2026-09-06

## Context

A variable declaration names its type, and for a user-defined type that name
is an identifier the parser cannot classify: `T` may be a structure, a function
block, an enumeration, or an alias of an elementary type. For a bare
declaration, `x : T;`, the parser has long emitted a placeholder,
`InitialValueAssignmentKind::LateResolvedType`, and
`xform_resolve_late_bound_type_initializer` replaces it with the concrete kind
once the types are known.

For a declaration with an initializer the parser guessed instead. A member
list, `x : T := (a := 1)`, became a `Structure` initializer; a bare identifier,
`x : T := Red`, became an `EnumeratedType` initializer. Both guesses are wrong
often enough to matter: a function-block instance declared with a member
initializer reached every later pass shaped as a structure, so the invocation
rules did not recognise it and code generation failed on it (#1649, #1654);
an alias of an elementary type initialized from a named constant was read as
an enumeration default.

Each consumer that met the mis-shaped declaration was tempted to repair it
locally by asking the type environment a second time, which is how #1649
taught the shared instance lookup to take a "is this a function block"
predicate. That duplicates knowledge the resolver already has, and every new
consumer would have to duplicate it again.

## Decision

The parser records what it saw and decides nothing about a user type. The
placeholder carries the initializer as written: a member list or a bare
identifier. The type resolver is the one place that maps type kind and
initializer shape to a concrete initializer kind:

| Type | Initializer | Resolves to |
|------|-------------|-------------|
| function block | members | `FunctionBlock` with the members as `init` |
| structure | members | `Structure` |
| enumeration | identifier | `EnumeratedType` with the identifier as the value |
| any other known type | identifier | `SimpleExpr`, for the constant-expression fold to evaluate or diagnose |
| unknown | either | the placeholder, with the same "undeclared type" diagnostic as a bare declaration |

The parser still emits `EnumeratedType` for a qualified value (`T := T#Red`)
and `EnumeratedValues` for an inline enumeration, because those are
unambiguous. It never constructs `Structure`, or a named `EnumeratedType` from
a bare identifier, so a downstream pass cannot meet a declaration the parser
guessed about.

## Consequences

- Every pass after the resolver sees a declaration shaped by its type, and
  none needs a second opinion from the type environment about what a
  declaration is.
- The plc2plc renderer prints the placeholder back exactly as written, so a
  round trip through the parser alone is unchanged.
- A member list on a type that takes none, or a value on a type that takes
  none, is diagnosed by the passes that already check initializers against
  declared types, once the resolver has given the declaration the kind its
  type implies.
- Code generation does not yet apply a member initializer to a function-block
  instance. It reports that as not implemented rather than allocating the
  instance and dropping the values silently.

## Alternatives considered

- **Repair the shape in the resolver only** (#1654 as first written): keep the
  parser's `Structure` guess and add a resolver arm that turns a
  structure-shaped initializer on a function-block type into a function-block
  initializer. It fixes the one case but leaves the parser guessing, leaves
  the enumeration guess in place, and leaves the AST claiming knowledge the
  parser did not have.
- **Teach each consumer to classify** (#1649): pass a "is this a function
  block" predicate into the shared instance lookup. Sound for that consumer,
  but the next consumer needs the same predicate, and the source of the
  problem is untouched.
