# Plan: Check the Type a Bit Access Selects From

Issue: #1851

## Goal

`ironplcc check` rejects a bit access (`x.n`, `x.%Xn`) or a partial access
(`x.%Bn`, `x.%Wn`, `x.%Dn`, `x.%Ln`) on a variable whose type has no bits to
select: `REAL`, `STRING`, `TIME`, a structure, a function block instance, a
whole array, an enumeration, a `BOOL`. It also range-checks a bit access on an
array declared in place (`a : ARRAY[1..2] OF DINT`), whose size the rule could
not see.

## Architecture

`rule_bit_and_partial_access_range` already resolves the accessed variable's
type through `variable_type::of`. It only asked how wide the type is. It now
asks first whether the type admits bit access at all:

- **Admits it:** a bit string (`BYTE`, `WORD`, `DWORD`, `LWORD`), an integer
  (`SINT` .. `ULINT`), or a subrange of an integer. The width bounds the index
  as before (P4025).
- **Does not:** every other type. The rule reports the new **P4069**
  (`BitAccessTypeInvalid`) on the accessed variable, and does not range-check
  the index.

Decisions:

- **`BOOL` is refused.** IEC 61131-3:2013 defines partial access for the bit
  strings `BYTE`, `WORD`, `DWORD` and `LWORD`; CODESYS and TwinCAT allow it on
  integers too, never on `BOOL`. The rule used to accept `b.0` .. `b.7` on a
  `BOOL`, because a `BOOL` takes one byte of storage.
- **Integers stay accepted in every dialect.** The documentation already
  promises bit access on integers without a flag, and the existing end-to-end
  tests rely on it. Gating it behind a dialect flag would be a breaking change
  and is out of scope; the pull request says so.
- **A new problem code rather than P4025.** "Index out of range" is the wrong
  message for `r.3` on a `REAL`: no index would be in range. P4061 is reserved
  by the `VAR_IN_OUT` design, P4064 and P4066--P4068 are taken by open pull
  requests, so the code is P4069.

`variable_type::of` resolves a named variable by the type id its declaration
carries (ADR-0055) before falling back to the initializer. The id of an inline
array is an anonymous array type *with* its dimensions, so `a.70` on an inline
array is refused like `a.3` rather than skipped. `of` also follows a
dereference to the referenced type: `p^` has the type `p` references, not
`REF_TO`, so `p^.3` on a `REF_TO BYTE` keeps being accepted.

## Prefactoring

Both checks in the rule turn the accessed type into a width with their own
copy of `size_in_bytes`. Extract one `selectable_bits` helper that both call,
behaviour unchanged, so the type test lands in one place.

## Design doc reference

`specs/design/partial-access-bit-syntax.md` -- add a requirement for the
type check (`REQ-PAB-analyzer-123`).

## File map

- `compiler/analyzer/src/rule_bit_and_partial_access_range.rs` -- helper, type
  check, tests
- `compiler/analyzer/src/variable_type.rs` -- resolve by type id, follow `^`
- `compiler/problems/resources/problem-codes.csv` -- P4069
- `docs/reference/compiler/problems/P4069.rst` -- new page
- `docs/reference/language/structured-text/bit-access.rst` -- valid base types
- `specs/design/partial-access-bit-syntax.md` -- requirement

## Tasks

- [ ] Prefactor: one `selectable_bits` helper for both checks
- [ ] Tests: inline array bit access out of range; `p^.3` on `REF_TO BYTE`
- [ ] `variable_type::of`: resolve by type id, follow dereference
- [ ] Tests: P4069 for `REAL`, `LREAL`, `STRING`, `TIME`, structure, function
      block, whole array, enumeration, `BOOL`, for `.n` and `.%Bn`; accepted
      for integer subrange
- [ ] P4069 in the CSV, the rule, the docs page, the bit access page
- [ ] Design requirement and conformance test
- [ ] `cd compiler && just`, `cd specs && just`, docs build
- [ ] `git rm` this plan
