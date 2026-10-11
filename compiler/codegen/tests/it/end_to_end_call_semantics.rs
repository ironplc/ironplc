//! End-to-end tests for what a call does to its arguments, its locals and
//! its result: an input is a copy, a function's and a method's locals start
//! again on every call, and a result is assigned only when the call returns.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{drive_fb, expect, run, FbStep};

/// A method's locals, with and without an initial value, start again on
/// every call.
const METHOD_LOCALS: &str = "
FUNCTION_BLOCK Stepper
  METHOD advance : DINT
    VAR zeroed : DINT; preset : DINT := 5; END_VAR
    zeroed := zeroed + 1;
    preset := preset + 1;
    advance := zeroed * 100 + preset;
  END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
  VAR stepper : Stepper; first : DINT; second : DINT; END_VAR
  first := stepper.advance();
  second := stepper.advance();
END_PROGRAM
";

/// Each instance's field is its own, also when two instances are called in
/// one expression.
const METHODS_ON_TWO_INSTANCES: &str = "
FUNCTION_BLOCK Total
  VAR sum : DINT; END_VAR
  METHOD add : DINT
    VAR_INPUT amount : DINT; END_VAR
    sum := sum + amount;
    add := sum;
  END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
  VAR ones : Total; tens : Total; one : DINT; ten : DINT; both : DINT; END_VAR
  one := ones.add(1);
  ten := tens.add(10);
  both := ones.add(1) + tens.add(10);
END_PROGRAM
";

/// A method's result keeps the low bits of a narrow type, and a parameter
/// wider than 32 bits arrives whole.
const METHOD_WIDTHS: &str = "
FUNCTION_BLOCK Widths
  METHOD narrow : SINT
    VAR_INPUT value : SINT; END_VAR
    narrow := value + 100;
  END_METHOD
  METHOD wide : LINT
    VAR_INPUT value : LINT; END_VAR
    wide := value + 1;
  END_METHOD
  METHOD fraction : REAL
    VAR_INPUT value : LREAL; END_VAR
    fraction := LREAL_TO_REAL(value);
  END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
  VAR widths : Widths; narrow : SINT; wide : LINT; fraction : REAL; END_VAR
  narrow := widths.narrow(100);
  wide := widths.wide(4294967296);
  fraction := widths.fraction(2.5);
END_PROGRAM
";

/// A result of an enumeration type is its value, and a result the body never
/// assigns is the type's default, not what the target held.
const FUNCTION_RESULTS: &str = "
TYPE Color : (Red, Green, Blue); END_TYPE

FUNCTION last_color : Color
  last_color := Blue;
END_FUNCTION

FUNCTION never_assigned : DINT
  ;
END_FUNCTION

PROGRAM main
  VAR color : Color; unassigned : DINT := 9; matched : BOOL; END_VAR
  color := last_color();
  matched := color = Blue;
  unassigned := never_assigned();
END_PROGRAM
";

/// A string result is assigned when the call returns, so a call may read the
/// variable its result goes to, and one call may be the argument of another.
const STRING_RESULT_ASSIGNED_TO_ITS_ARGUMENT: &str = "
FUNCTION twice : STRING[20]
  VAR_INPUT text : STRING[20]; END_VAR
  twice := CONCAT(text, text);
END_FUNCTION

PROGRAM main
  VAR same : STRING[20] := 'ab'; nested : STRING[20]; END_VAR
  same := twice(same);
  nested := twice(twice('x'));
END_PROGRAM
";

/// A string input is a copy, cut to the parameter's length: the callee's
/// assignment does not reach the caller.
const STRING_INPUT: &str = "
FUNCTION overwrite : DINT
  VAR_INPUT text : STRING[10]; END_VAR
  text := 'changed';
  overwrite := LEN(text);
END_FUNCTION

FUNCTION first_three : DINT
  VAR_INPUT text : STRING[3]; END_VAR
  first_three := LEN(text);
END_FUNCTION

PROGRAM main
  VAR
    kept : STRING[10] := 'orig'; long : STRING[10] := 'abcdefg';
    written : DINT; from_variable : DINT; from_literal : DINT;
  END_VAR
  written := overwrite(kept);
  from_variable := first_three(long);
  from_literal := first_three('abcdefghij');
END_PROGRAM
";

/// A scalar input is a copy too.
const SCALAR_INPUT: &str = "
FUNCTION increment : DINT
  VAR_INPUT value : DINT; END_VAR
  value := value + 1;
  increment := value;
END_FUNCTION

PROGRAM main
  VAR kept : DINT := 5; result : DINT; END_VAR
  result := increment(kept);
END_PROGRAM
";

/// A function's string locals, with and without an initial value, start
/// again on every call.
const FUNCTION_STRING_LOCALS: &str = "
FUNCTION lengths : DINT
  VAR preset : STRING[10] := 'ab'; empty : STRING[10]; END_VAR
  preset := CONCAT(preset, 'x');
  empty := CONCAT(empty, 'y');
  lengths := LEN(preset) * 10 + LEN(empty);
END_FUNCTION

PROGRAM main
  VAR first : DINT; second : DINT; END_VAR
  first := lengths();
  second := lengths();
END_PROGRAM
";

/// A function block and a function that nothing calls are not generated, so
/// they may use what code generation does not support yet: here a `VAR_TEMP`
/// in a function block and a `VAR_OUTPUT` of a function.
const UNCALLED_DECLARATIONS: &str = "
FUNCTION_BLOCK UncalledBlock
  VAR_TEMP scratch : DINT; END_VAR
  scratch := 1;
END_FUNCTION_BLOCK

FUNCTION uncalled_function : DINT
  VAR_INPUT value : DINT; END_VAR
  VAR_OUTPUT other : DINT; END_VAR
  uncalled_function := value;
END_FUNCTION

PROGRAM main
  VAR ran : DINT; END_VAR
  ran := 1;
END_PROGRAM
";

/// Methods are an extension; nothing else here needs one.
fn options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

#[rstest]
#[case::method_locals_start_again(METHOD_LOCALS, &[run(0), expect("first", 106), expect("second", 106)])]
#[case::methods_on_two_instances(
    METHODS_ON_TWO_INSTANCES,
    &[run(0), expect("one", 1), expect("ten", 10), expect("both", 22)]
)]
#[case::method_widths(
    METHOD_WIDTHS,
    &[run(0), expect("narrow", -56), expect("wide", 4_294_967_297i64), expect("fraction", 2.5f32)]
)]
#[case::function_results(
    FUNCTION_RESULTS,
    &[run(0), expect("matched", 1), expect("unassigned", 0)]
)]
#[case::string_result_assigned_to_its_argument(
    STRING_RESULT_ASSIGNED_TO_ITS_ARGUMENT,
    &[run(0), expect("same", "abab"), expect("nested", "xxxx")]
)]
#[case::string_input_is_a_copy(
    STRING_INPUT,
    &[run(0), expect("kept", "orig"), expect("written", 7),
      expect("from_variable", 3), expect("from_literal", 3)]
)]
#[case::scalar_input_is_a_copy(SCALAR_INPUT, &[run(0), expect("kept", 5), expect("result", 6)])]
#[case::function_string_locals_start_again(
    FUNCTION_STRING_LOCALS,
    &[run(0), expect("first", 31), expect("second", 31)]
)]
#[case::uncalled_declarations_not_generated(UNCALLED_DECLARATIONS, &[run(0), expect("ran", 1)])]
fn end_to_end_when_called_then_inputs_are_copies_and_locals_start_again(
    #[case] source: &str,
    #[case] steps: &[FbStep],
) {
    drive_fb(source, &options(), steps);
}
