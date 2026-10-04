# Show the Type of Every Expression as Annotated Structured Text

## Goal

`ironplcc echo --types` writes the analyzed program as Structured Text with a
comment after every expression naming the type the analyzer recorded for it,
and every implicit conversion as `FROM -> TO`, so the type annotation on the
AST can be inspected by reading it. A value the compiler knows (a literal) is
marked `CONSTANT`, and a type the analyzer left for codegen to decide is
marked `?`.

## Background

Every expression carries the type of its value in `Expr::expr_type`
([Expression Type Resolution](../design/expression-type-resolution.md),
ADR-0055), and the analyzer records the conversions the language makes as
`ExprKind::ImplicitConversion` nodes (ADR-0056). Nothing shows either:

- `ironplcc echo` renders each file as parsed, before analysis, so no
  expression has a type yet.
- plc2plc writes an `ImplicitConversion` as the operand it converts, so the
  conversion is invisible even in an analyzed library.
- `expr_type` holds a `TypeId`, which only the analyzer's `TypeEnvironment`
  can name.

The work on [#2050](https://github.com/ironplc/ironplc/issues/2050) moves conversion decisions from codegen into the analyzer
one context at a time. Each step is reviewed by reading tests that assert on
individual nodes; there is no way to look at a whole program and see what was
recorded.

## What it looks like

Produced by a throwaway prototype on `main` at 9dbb7e5, in the format this
plan proposes (declarations omitted):

```
FUNCTION Scale : LREAL
VAR_INPUT raw : DINT; gain : LREAL; END_VAR
    Scale := raw * gain;
END_FUNCTION

PROGRAM main
VAR
    count : INT; total : DINT; big : LINT; level : REAL;
    out : LREAL; flag : BOOL; arr : ARRAY[1..4] OF INT; i : INT;
END_VAR
    count := count + 1;
    flag := big > total;
    level := count;
    out := Scale(raw := total, gain := 0.5);
    FOR i := 1 TO 4 DO
        arr[i] := MAX(arr[i], count);
    END_FOR;
    IF NOT flag AND total < 10 THEN
        big := ABS(big) + total;
    END_IF;
END_PROGRAM
```

renders as

```
Scale := ( raw (* DINT -> LREAL *) * gain (* LREAL *) ) (* LREAL *) ;
...
count := ( count (* INT *) + 1 (* CONSTANT INT *) ) (* INT *) ;
flag := ( big (* LINT *) > total (* DINT -> LINT *) ) (* BOOL *) ;
level := count (* INT -> REAL *) ;
out := Scale ( total (* DINT *) , 0.5 (* CONSTANT REAL -> LREAL *) ) (* LREAL *) ;
FOR i := 1 (* CONSTANT INT *) TO 4 (* CONSTANT INT *) DO
   arr[ i (* INT *) ] := MAX ( arr[ i (* INT *) ] (* INT *) , count (* INT *) ) (* INT *) ;
END_FOR ;
IF ( ( NOT flag (* BOOL *) ) (* BOOL *) AND ( total (* DINT *) < 10 (* CONSTANT DINT *) ) (* BOOL *) ) (* BOOL *) THEN
   big := ( ABS ( big (* LINT *) ) (* LINT *) + total (* DINT -> LINT *) ) (* LINT *) ;
END_IF ;
```

and, from two more programs:

```
c := Green (* CONSTANT Color *) ;
b := ( c (* Color *) = Red (* CONSTANT Color *) ) (* BOOL *) ;
v := ( ( r (* REF_TO INT *)^ ) (* INT *) + 1 (* CONSTANT INT *) ) (* INT *) ;
b := ( 1 (* CONSTANT ? ANY_INT *) < 2 (* CONSTANT ? ANY_INT *) ) (* BOOL *) ;
name := CONCAT ( name (* STRING *) , 'abc' (* CONSTANT STRING *) ) (* STRING *) ;
v := 12 (* CONSTANT INT *) ;

via_param := Pass ( 0.1 (* CONSTANT REAL -> LREAL *) ) (* LREAL *) ;
direct := 0.1 (* CONSTANT LREAL *) ;
```

The output is still Structured Text: the prototype's output re-parses, and
`ironplcc check` accepts it.

The prototype already shows things worth seeing:

- **A literal argument is converted at run time.** The `0.1` passed to an
  `LREAL` parameter is a constant `REAL` converted to `LREAL`, where the
  same literal assigned to an `LREAL` is an `LREAL`. Codegen loads the `REAL`
  constant and converts it at run time (`compile_expr.rs`), so the parameter
  receives `0.10000000149011612`, not `0.1` (checked on the VM; filed as
  [#2109](https://github.com/ironplc/ironplc/issues/2109)).
- **Two literals compared are still undecided**, `? ANY_INT`, so codegen
  still decides their type (the step after
  [#2110](https://github.com/ironplc/ironplc/pull/2110)).
- A `STRING[20]` variable's expression type is `STRING`.
- `v := INT#5 + 7` was folded to the constant `12`.

## Architecture

### plc2plc: an annotated rendering

Add `write_to_string_with_types(lib, type_name: &dyn Fn(TypeId) -> String)`.
plc2plc is handed a function that names a type, so it does not depend on the
analyzer.

`LibraryRenderer` holds the function as an `Option` and overrides
`visit_expr`. With no function, `visit_expr` recurses as it does today, so
`write_to_string` output is unchanged byte for byte. With one:

- After each expression, write `(* T *)`, where `T` comes from `expr_type`:
  `Concrete(id)` is `type_name(id)` and `Null` is `NULL`.
- **A literal is marked `CONSTANT`.** A literal (`ExprKind::Const`, including
  one constant folding produced), an enumerated value and `NULL` have values
  the compiler knows, so their comment starts with `CONSTANT`:
  `1 (* CONSTANT INT *)`. A variable, a call or an operation produces its value
  at run time and has no prefix. A variable declared `CONSTANT` has no prefix
  either: codegen loads it at run time, and constant folding substitutes only
  literals.
- **An undecided type is marked `?`.** An expression with no type is
  `(* ? *)`. One whose type is still a generic category is `(* ? ANY_INT *)`:
  the analyzer has not decided its type, so codegen does, which is what
  [#2050](https://github.com/ironplc/ironplc/issues/2050) removes. Once #2050
  is done no literal should be left at a category, and searching the output
  for `(* ?` or `? ANY_` finds what is still undecided.
- An `ImplicitConversion` is written as its operand without the operand's own
  comment, then `(* FROM -> TO *)`. A converted literal keeps its prefix:
  `0.5 (* CONSTANT REAL -> LREAL *)`. `CONSTANT` says the value is known
  when compiling. It does not say that codegen converts it then, and today it
  does not.
- **A `CONSTANT FROM -> TO` comment is a conversion that could be removed.**
  The compiler knows the value, so it could give the literal the type `TO`
  and emit no widening or narrowing at run time. Searching the output for
  `CONSTANT` followed by `->` lists every such conversion.
- A unary operation and a dereference are parenthesised, so the operand's
  comment and the operator's do not sit side by side (`NOT flag (* BOOL *)
  (* BOOL *)` is ambiguous).

The text of a comment is built in a new module, `plc2plc/src/type_comment.rs`.
`renderer.rs` gains only the field and the `visit_expr` override.
`renderer.rs` is already 1975 lines, over the 1000-line limit. Splitting it is
much larger than this change, so it is left for its own change (see
Follow-ups).

### analyzer: a type's spelling

Add `value_type::spelling(types, id) -> String`, the type as Structured Text
writes it: an elementary type in upper case (`DINT`), a named type by its
declared name (`Color`), and an anonymous type by the shape `describe`
already gives (`ARRAY[1..4] OF INT`, `REF_TO INT`). `describe` words an inline
enumeration or subrange as prose (`an enumeration`, `a subrange of INT`), and
`spelling` keeps that wording for now.

`describe` is not changed. Its diagnostics say `dint`, and changing that is a
separate decision.

### CLI: `echo --types`

`ironplcc echo --types FILES` runs analysis (`project.semantic()`) and renders
`analyzed_library()` with `spelling` over `semantic_context().types()`.
`--library` becomes valid on `echo` together with `--types`, so a program that
uses a compatibility library can be analyzed.

- **Only the user's declarations are rendered.** The analyzed library also
  holds the declarations of every activated library. Elements are kept when
  the file their name was declared in is one of the project's sources.
- **Diagnostics are reported as usual**, and the program is rendered whenever
  analysis built a context. A program with semantic errors still shows what
  was resolved, and `?` marks what was not.
- **The output is the analyzed program, not the source.** Declarations come in
  dependency order, named arguments are positional, constant expressions are
  folded, and every file is merged into one output. The docs say so.
- **Not annotated:** assignment targets, `FOR` control variables and `CASE`
  labels. They are `Variable`s and labels, not `Expr`s, and their types are
  in the declarations.

## Alternatives considered

| Option | Why not (now) |
|---|---|
| A comment after each statement listing each subexpression and its type | Keeps the code line clean, but matching a table row to a subexpression is the reader's job, and each subexpression's text is written twice |
| A tree dump (`clang -ast-dump` style) of node kind, span and `TypeId` | Complete, but not Structured Text; it can be added later as a second view if one is needed |
| Write conversions as explicit functions (`DINT_TO_LINT(total)`) | Hides the fact that the conversion is implicit, and a subrange, alias or reference target has no conversion function |
| Editor inlay hints through the language server | The best view for everyday editing, but larger, and its output cannot be diffed or put in a test. The annotated rendering is also what a VS Code command could show (see Follow-ups) |
| Annotate only compound expressions, not variables and literals | Less noise, but a literal's type is one of the things most worth seeing (`0.1 (* CONSTANT REAL -> LREAL *)`) |
| Mark variables as well as literals | A variable's type is fixed by its declaration, but its value is loaded at run time, and `CONSTANT` on it would read as the ST keyword, a constant variable. An unprefixed comment already means "computed at run time" |
| Write a generic category as an ordinary type (`(* ANY_INT *)`) | A literal left at a category is one whose type codegen still decides. Writing it like any other type hides the work #2050 has left |

## Design doc

Add a section, *Inspecting the annotation*, to
`specs/design/expression-type-resolution.md` that records the format above.
That document has no requirement IDs yet. The new section adds:

- **REQ-ETR-plc2plc-001** The annotated rendering writes `(* T *)` after every expression, where `T` is the type recorded for it.
- **REQ-ETR-plc2plc-002** An implicit conversion is written as its operand followed by `(* FROM -> TO *)`, and the operand has no comment of its own.
- **REQ-ETR-plc2plc-003** A literal, an enumerated value and `NULL` are annotated with `CONSTANT` before the type, and no other expression is: `1 (* CONSTANT INT *)` but `count (* INT *)`.
- **REQ-ETR-plc2plc-004** An expression whose type the analyzer did not decide is annotated with `?`: `(* ? *)` with no recorded type, and `(* ? ANY_INT *)` at a generic category.
- **REQ-ETR-plc2plc-005** A unary operation and a dereference are parenthesised in the annotated rendering.
- **REQ-ETR-plc2plc-006** The annotated rendering re-parses to the same library as the plain rendering of the same library.
- **REQ-ETR-analyzer-001** `spelling` writes an elementary type in upper case, a named type by its declared name, and an anonymous type by its shape.

Register the document in `plc2plc/build.rs` and `analyzer/build.rs`.

## Prefactoring

None needed:

- The renderer already visits every `Expr` through one method, so the
  annotation is one override, not a branch in each `visit_*_expr`.
- `create_project` and `finish` already serve every CLI command, and the new
  `echo_types` uses them as `check` does.
- `spelling` reuses `describe` for every non-elementary type, so the walk over
  a type's representation is not copied.

## File map

| File | Change |
|------|--------|
| `compiler/plc2plc/src/lib.rs` | `write_to_string_with_types` |
| `compiler/plc2plc/src/renderer.rs` | Optional type-name function; `visit_expr` override |
| `compiler/plc2plc/src/type_comment.rs` | New: comment text for an expression's type and for a conversion |
| `compiler/plc2plc/src/tests/expression_types.rs` | New: tests of the annotated rendering |
| `compiler/plc2plc/src/spec_conformance_expression_types.rs` | New: REQ-ETR-plc2plc conformance tests |
| `compiler/plc2plc/resources/test/expression_types.st`, `expression_types_rendered.st` | New: golden source and annotated rendering |
| `compiler/plc2plc/build.rs` | Register `expression-type-resolution.md` |
| `compiler/analyzer/src/value_type.rs` | `spelling` and its tests |
| `compiler/analyzer/build.rs` | Register `expression-type-resolution.md` |
| `compiler/ironplc-cli/bin/main.rs` | `--types` and `--library` on `Echo` |
| `compiler/ironplc-cli/src/cli.rs` | `echo_types`; filter to the user's elements |
| `compiler/ironplc-cli/tests/cli.rs` | `echo --types` integration tests |
| `specs/design/expression-type-resolution.md` | *Inspecting the annotation* section |
| `docs/reference/compiler/ironplcc.rst` | `echo --types`, `--library` on `echo`, and an example |

## Tasks

One core change PR.

- [ ] `value_type::spelling`, with tests for an elementary, a named, an anonymous array and a reference type
- [ ] `type_comment.rs` and the `visit_expr` override; plain rendering unchanged
- [ ] plc2plc tests, analyzing the source with `ironplc_analyzer::stages::analyze` (already a dev-dependency):
  - [ ] `write_to_string_with_types_when_binary_expr_then_comment_after_operands_and_result`
  - [ ] `write_to_string_with_types_when_implicit_conversion_then_comment_shows_from_and_to`
  - [ ] `write_to_string_with_types_when_literal_or_enumerated_value_then_constant_prefix`
  - [ ] `write_to_string_with_types_when_variable_then_no_prefix`
  - [ ] `write_to_string_with_types_when_converted_literal_then_constant_prefix_and_from_and_to`
  - [ ] `write_to_string_with_types_when_generic_category_then_question_mark_and_category`
  - [ ] `write_to_string_with_types_when_no_type_then_question_mark`
  - [ ] `write_to_string_with_types_when_unary_or_deref_then_parenthesised`
  - [ ] Golden file for a program covering each expression kind, re-parsed and compared with the plain rendering
- [ ] Design section and conformance tests for each REQ-ETR requirement
- [ ] `echo --types` and `--library` in the CLI; render only the user's elements
- [ ] CLI tests: `echo_when_types_then_writes_conversion_comment`, `echo_when_types_and_library_then_library_declarations_not_rendered`, `echo_when_types_and_semantic_error_then_renders_and_err`
- [ ] Update `docs/reference/compiler/ironplcc.rst`
- [ ] `cd compiler && just`

## Verification

- The existing plc2plc golden files and round-trip tests pass untouched:
  the plain rendering does not change.
- `ironplcc echo --types` on the sample above produces the output shown, and
  `ironplcc check` accepts that output.

## Follow-ups

Not part of this plan:

- A VS Code command that shows the annotated rendering of the open file, via
  a language server request that reuses `write_to_string_with_types`.
- A *Types* output tab in the playground.
- Split `plc2plc/src/renderer.rs` below the 1000-line limit.
- Golden-file tests of `xform_insert_implicit_conversions` built on the
  annotated rendering, in place of tests that walk to a single node.
- [#2109](https://github.com/ironplc/ironplc/issues/2109): give an untyped
  literal argument the parameter's type instead of converting its default
  type at run time, so `Pass(0.1)` on an `LREAL` parameter passes `0.1`.
- Remove the other conversions of constants that the annotated output shows
  (`CONSTANT FROM -> TO`), each as its own correction, since each one changes
  generated code.

## Open questions

1. A `--types` flag on `echo`, or a separate subcommand? The output is the
   analyzed program rather than the parsed one, which is a case for a separate
   name.
2. Is a design section with requirement IDs right for a diagnostic output, or
   are doc comments and the CLI reference enough?
