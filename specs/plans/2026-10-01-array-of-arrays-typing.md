# Plan: Type and Compile Elements of Arrays of Arrays

Issue: #1925

## Goal

`rows[1][2]`, where `rows : ARRAY[1..2] OF Row` and
`Row : ARRAY[1..3] OF DINT`, has the type `DINT`, so assigning,
comparing or passing it type-checks. Code generation compiles and runs
reads and writes of such elements. A construct code generation cannot
compile is refused by the analyzer, so `check` reports it.

## Architecture

### Analyzer: the type a variable selection names

`xform_resolve_expr_types` types an array element by taking the element type
of the chain's base variable, whatever the chain: every bracket of
`rows[1][2]` strips the same single level, giving `Row`. A field of an array
element (`recs[1].v[2]`) is not typed at all.

A new module, `selection_type`, walks a variable selection from its base
variable outwards and answers the type it names:

- a view of a type is either a type **name** (kept as long as it is known,
  so an element of an enumeration, subrange or structure type keeps its
  nominal identity), an **array** part way through its subscripts, a
  **reference**, or a structural **shape** (a field, whose declared type
  name the type environment does not keep);
- a bracket consumes subscripts from the array it is in; the element type is
  reached once every dimension has a subscript, so `m[1][2]` and `m[1, 2]`
  on an `ARRAY[1..2, 1..3]` both select an element, while `rows[1]` selects
  a `Row`;
- named array types expand through their declarations (collected from the
  library, since the type environment keeps only the element's shape).

`resolve_variable_type` uses it for subscripts, fields and dereferences.

### Code generation: arrays of arrays are laid out flat

An `ARRAY[1..2] OF Row` occupies the same storage, row-major, as an
`ARRAY[1..2, 1..3] OF DINT`, and code generation already flattens the
subscripts of a chain into one index. The array specification of a
variable, a structure field, and a field of an array-of-structures element
flattens nested array element types into extra dimensions.

### Analyzer: refuse what is not compiled

A selection that names a whole inner array (`rows[1]` as a value, target or
argument) needs a copy of part of an array, which code generation does not
do. A new rule reports it with a dedicated problem code (P9004), and a
follow-up issue tracks the capability.

## Prefactoring

None needed in the resolver: the new walk replaces
`declared_element_type_name`, `resolve_parent_struct_type`,
`resolve_struct_field_array_element_type` and
`resolve_structured_variable_type` rather than growing them, which keeps
`xform_resolve_expr_types.rs` below 1000 lines.

## Design doc reference

`specs/design/expression-type-resolution.md` (Resolution table, Variable
row).

## File map

| File | Change |
|------|--------|
| `compiler/analyzer/src/selection_type.rs` | new: walk of a variable selection |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | use it |
| `compiler/analyzer/src/rule_inner_array_selection.rs` | new rule, P9004 |
| `compiler/analyzer/src/stages.rs`, `lib.rs` | register |
| `compiler/codegen/src/compile_array_nested.rs` | new: flatten nested arrays |
| `compiler/codegen/src/compile_array.rs`, `compile_array_struct.rs` | use it |
| `compiler/codegen/tests/end_to_end_array_of_arrays.rs` | end-to-end tests |
| `compiler/problems/resources/problem-codes.csv` | P9004 |
| `docs/reference/compiler/problems/P9004.rst` | new |
| `docs/reference/language/data-types/derived/array-types.rst` | arrays of arrays |
| `specs/design/expression-type-resolution.md` | Variable row |

## Tasks

- [ ] Failing analyzer tests: `rows[1][2]` is `DINT`; named and inline
      outer arrays; `m[1][2]` and `m[1, 2]`; `recs[1].v[2]`; `s.t[1][2]`;
      comparisons and calls
- [ ] `selection_type` module; resolver uses it
- [ ] Failing end-to-end tests: read, write, expressions, structure fields
- [ ] Flatten nested arrays in codegen
- [ ] Rule and P9004 for a selection naming a whole inner array; docs
- [ ] Issue for whole inner array access
- [ ] Docs, design doc, `cd compiler && just`, `cd specs && just`, docs build
- [ ] `git rm` this plan
