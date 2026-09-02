# Diagnose out-of-range constants during analysis

Implements the fix for
[issue #1527](https://github.com/ironplc/ironplc/issues/1527). The
prefactoring it depends on has landed separately; the issue is the record
across the two pull requests.

## Goal

A constant that does not fit the type it is stored into is a defect, not a
request to wrap. Report it as `P2026` during semantic analysis, so
`ironplcc check` and the language server flag it in the editor instead of the
compiler silently wrapping it.

```iecst
PROGRAM main
  VAR
    a : USINT := 300;   (* stored 44 -- now P2026 *)
    b : USINT;
    c : SINT;
  END_VAR
  b := 300;             (* stored 44 -- now P2026 *)
  c := 200;             (* stored -56 -- now P2026 *)
END_PROGRAM
```

Today nothing reports these. `ironplcc check` exits 0 and `compile` wraps the
values through the VM's two's-complement `TRUNC_*` semantics.

## Architecture

The only range check on a literal today lives in `codegen`
(`compile_constant`), which sees an `OpType` — a width and a signedness — so
`SINT`, `INT` and `DINT` all collapse to `(W32, Signed)` and only a value
outside `i32` is caught. The declared type, which is what actually bounds the
value, is known to the analyzer. A defect caught there reaches every entry
point (`check`, the LSP, `compile`) rather than only the ones that generate
code.

### One statement of a type's range

`value_range` states the values a type can hold, from the same
`IntermediateType` record codegen already projects onto `VarTypeInfo`. Codegen
derives its operation width from the value width, so a value that fits a type
fits the width it is operated on — by construction, not coincidence.

`narrow_type_range` in `codegen/src/compile_stmt.rs` derives a range from the
same facts for `FOR`-loop `TRUNC` elision. It takes its numbers from
`value_range` rather than restating them.

### Where the rule checks a constant

The rule walks each place the program declares what type a value is stored or
compared as, and pushes that expected type down to the literals beneath it.

| Where the constant appears | Expected type |
|---|---|
| Variable initializer (`a : USINT := 300`) | the declared type |
| Assignment (`b := 300`) | the target variable's resolved type |
| Operand of an operator (`b := 300 + 1`) | the type pushed into the operator |
| Comparison (`IF c = 200 THEN`) | the other operand's resolved type |
| `CASE` label | the selector's type |

Pushing the type through operators is what mirrors codegen: codegen passes one
`OpType` into both operands, so `b := 300 + 0` compiles the literal exactly as
`b := 300` does.

A negated literal needs no special handling: `xform_fold_constant_expressions`
folds `-200` into one signed constant before any rule runs, so the sign
reaches the check with the value.

Assignment targets resolve through field accesses and subscripts, so
`s.count := 300` and `readings[i] := 300` are covered, and a global target
resolves through the scoped table's base scope.

A bit or partial access target is where a value range and an index range part
company: `x.3 := v` writes a `BOOL` and `w.%B1 := v` writes a byte, so the
rule resolves those targets itself rather than taking `variable_type::of`'s
answer, which is the accessed variable's type.

### Which types are range-checked

| Type | Checked | Why |
|---|---|---|
| `SINT`, `INT`, `DINT`, `LINT`, `USINT`, `UINT`, `UDINT`, `ULINT` | yes | Numbers. A value outside the range is a defect. |
| Subrange (`INT(0..10)`) | yes | The declaration states the range, so a constant outside it cannot be stored. Nothing checks this today either. |
| `BYTE`, `WORD`, `DWORD`, `LWORD` | no | Bit strings are patterns, not numbers, and wrapping them is a legitimate thing to want (`end_to_end_when_byte_overflow_then_wraps` asserts it). |
| `REAL`, `LREAL`, and everything else | no | Not integer storage. |

Only a decimal integer literal is checked. A radix-prefixed literal (`16#FF`,
`2#1010`) is a bit pattern in the same sense that `BYTE` is, so it stays out —
consistent with the bit-string row above.

A literal that contradicts its own prefix (`x : DINT := INT#40000`) is
[issue #1545](https://github.com/ironplc/ironplc/issues/1545).

### Codegen keeps reporting P2026

Codegen cannot tell a constant the rule checked from one the analyzer could
never type, so it cannot honestly call either a compiler defect. Its check
stays as it is. If the rule misses a context, codegen behaves exactly as it
does today; nothing degrades into an internal error about a program that is
merely wrong.

## Prefactoring

Landed already, in its own pull request: variable-type resolution extracted to
`variable_type.rs` and moved onto the analyzer's scoped table, and codegen's
elementary type table replaced by a projection of the analyzer's. Nothing
further is needed here — the rule adds a module and a check rather than
another branch in an existing one.

## File map

Created:

* `compiler/analyzer/src/value_range.rs` — the values a type can hold
* `compiler/analyzer/src/rule_constant_range.rs` — the rule

Modified:

* `compiler/analyzer/src/lib.rs`, `stages.rs` — register the module and rule
* `compiler/codegen/src/compile_stmt.rs` — `narrow_type_range` from
  `value_range`
* `docs/reference/compiler/problems/P2026.rst` — document the diagnostic

## Tasks

- [ ] Add `value_range`, with the boundary of every integer type tested
- [ ] Point `narrow_type_range` at it
- [ ] Add `rule_constant_range`: initializers and assignments first, then the
      operator, comparison and `CASE` contexts
- [ ] Resolve a bit or partial access target to what it writes
- [ ] Register the rule in `stages::semantic`
- [ ] Tests: each context, each type's boundaries, the bit-string and radix
      exclusions, and the issue's program end to end
- [ ] Document `P2026`, including what is deliberately not checked
- [ ] `cd compiler && just`
- [ ] `git rm` this plan
