//! End-to-end tests for which declaration a name refers to: names that
//! differ in case from their declaration, and one name declared in more than
//! one scope.

use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;

use crate::common::{drive_fb, expect, run, FbStep};

/// Every reference differs in case from its declaration: a variable, a
/// structure field, a function and its parameter, a function block type, its
/// input and output, an enumeration value and a type name.
const REFERENCES_DIFFER_IN_CASE: &str = "
TYPE Color : (Red, Green, Blue); END_TYPE
TYPE Point : STRUCT Xval : DINT; END_STRUCT; END_TYPE

FUNCTION AddOne : DINT
  VAR_INPUT Val : DINT; END_VAR
  ADDONE := VAL + 1;
END_FUNCTION

FUNCTION_BLOCK Accumulator
  VAR_INPUT Inp : DINT; END_VAR
  VAR_OUTPUT Total : DINT; END_VAR
  TOTAL := total + INP;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    Counter : DINT; point : POINT; sum : accumulator; shade : COLOR;
    field : DINT; called : DINT; summed : DINT; matched : BOOL;
  END_VAR
  COUNTER := counter + 1;
  POINT.XVAL := 4;
  field := point.xval;
  called := addone(val := 1) + ADDONE(VAL := 2);
  SUM(INP := 3);
  sum(inp := 4);
  summed := Sum.total;
  shade := GREEN;
  matched := SHADE = green;
END_PROGRAM
";

/// `x` is a global, a local of one function and a field of a function block.
/// Each body sees its own, and a function without one sees the global.
const NAME_IN_THREE_SCOPES: &str = "
VAR_GLOBAL
  x : DINT := 1000;
END_VAR

FUNCTION own_local : DINT
  VAR x : DINT; END_VAR
  x := 5;
  own_local := x;
END_FUNCTION

FUNCTION the_global : DINT
  the_global := x;
END_FUNCTION

FUNCTION_BLOCK OwnField
  VAR x : DINT; END_VAR
  VAR_OUTPUT seen : DINT; END_VAR
  x := x + 7;
  seen := x;
END_FUNCTION_BLOCK

PROGRAM main
  VAR local : DINT; global : DINT; field : DINT; block : OwnField; END_VAR
  local := own_local();
  global := the_global();
  block();
  field := block.seen;
END_PROGRAM
";

/// The caller's variables have the names of the callee's parameters, and are
/// passed the other way round.
const ARGUMENTS_NAMED_AS_OTHER_PARAMETER: &str = "
FUNCTION difference : DINT
  VAR_INPUT x : DINT; y : DINT; END_VAR
  difference := x - y;
END_FUNCTION

PROGRAM main
  VAR x : DINT := 10; y : DINT := 3; positional : DINT; named : DINT; END_VAR
  positional := difference(y, x);
  named := difference(x := y, y := x);
END_PROGRAM
";

/// Two function block types declare `i`, `o` and `n` at different types, and
/// the program declares all three names as well.
const FIELD_NAMES_SHARED_BY_TWO_TYPES: &str = "
FUNCTION_BLOCK Whole
  VAR_INPUT i : DINT; END_VAR
  VAR_OUTPUT o : DINT; END_VAR
  VAR n : DINT; END_VAR
  n := n + i;
  o := n;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Fraction
  VAR_INPUT i : REAL; END_VAR
  VAR_OUTPUT o : REAL; END_VAR
  VAR n : REAL; END_VAR
  n := n + i;
  o := n * 2.0;
END_FUNCTION_BLOCK

PROGRAM main
  VAR whole : Whole; fraction : Fraction; i : DINT := 3; o : REAL; n : DINT; END_VAR
  whole(i := i);
  fraction(i := 1.5);
  whole(i := i);
  n := whole.o;
  o := fraction.o;
END_PROGRAM
";

/// Two functions declare a local `t` and a parameter `a`, and one calls the
/// other while its own are live.
const LOCALS_SHARED_BY_NESTED_CALLS: &str = "
FUNCTION inner : DINT
  VAR_INPUT a : DINT; END_VAR
  VAR t : DINT; END_VAR
  t := a * 2;
  inner := t;
END_FUNCTION

FUNCTION outer : DINT
  VAR_INPUT a : DINT; END_VAR
  VAR t : DINT; END_VAR
  t := inner(a) + 1;
  outer := t + inner(t);
END_FUNCTION

PROGRAM main
  VAR r : DINT; END_VAR
  r := outer(inner(1)) + inner(outer(1));
END_PROGRAM
";

/// A reference a body takes to its own field refers to the field of the
/// instance that is running, for each of two instances.
const REFERENCE_TO_OWN_FIELD: &str = "
FUNCTION_BLOCK Counted
  VAR n : DINT; own : REF_TO DINT; END_VAR
  VAR_OUTPUT calls : DINT; END_VAR
  own := REF(n);
  own^ := own^ + 1;
  calls := n;
END_FUNCTION_BLOCK

PROGRAM main
  VAR twice : Counted; once : Counted; two : DINT; one : DINT; END_VAR
  twice();
  once();
  twice();
  two := twice.calls;
  one := once.calls;
END_PROGRAM
";

fn ref_to_options() -> CompilerOptions {
    CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    }
}

#[rstest]
#[case::references_differ_in_case(
    REFERENCES_DIFFER_IN_CASE,
    CompilerOptions::default(),
    &[run(0), expect("counter", 1), expect("field", 4), expect("called", 5),
      expect("summed", 7), expect("matched", 1)]
)]
#[case::name_in_three_scopes(
    NAME_IN_THREE_SCOPES,
    CompilerOptions::from_dialect(Dialect::Rusty),
    &[run(0), expect("local", 5), expect("global", 1000), expect("field", 7), expect("x", 1000)]
)]
#[case::arguments_named_as_other_parameter(
    ARGUMENTS_NAMED_AS_OTHER_PARAMETER,
    CompilerOptions::default(),
    &[run(0), expect("positional", -7), expect("named", -7)]
)]
#[case::field_names_shared_by_two_types(
    FIELD_NAMES_SHARED_BY_TWO_TYPES,
    CompilerOptions::default(),
    &[run(0), expect("n", 6), expect("o", 3.0f32), expect("i", 3)]
)]
#[case::locals_shared_by_nested_calls(
    LOCALS_SHARED_BY_NESTED_CALLS,
    CompilerOptions::default(),
    &[run(0), expect("r", 33)]
)]
#[case::reference_to_own_field(
    REFERENCE_TO_OWN_FIELD,
    ref_to_options(),
    &[run(0), expect("two", 2), expect("one", 1)]
)]
fn end_to_end_when_name_declared_in_several_places_then_resolves_to_nearest_declaration(
    #[case] source: &str,
    #[case] options: CompilerOptions,
    #[case] steps: &[FbStep],
) {
    drive_fb(source, &options, steps);
}
