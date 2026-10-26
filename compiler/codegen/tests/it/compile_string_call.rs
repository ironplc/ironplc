//! Tests of calls of user-defined functions and methods with string inputs,
//! compiled from the lowered program (REQ-LOW-lowering-072). Each program
//! compiles, and leaves every variable with the same values on every backend
//! after one scan.

use crate::compile_body::{assert_compiles, compile_and_run_agreeing};

/// Functions of string inputs: the length of a `STRING[4]` input, which its
/// body then overwrites; whether a `STRING[4]` and a `WSTRING[8]` input hold
/// as many code units; and the length of a `STRING` input as an output, and
/// twice it as the result.
const FUNCTIONS: &str = "
    FUNCTION FIRST4 : DINT
      VAR_INPUT s : STRING[4]; END_VAR
      FIRST4 := LEN(s);
      s := 'changed';
    END_FUNCTION
    FUNCTION SAME_LEN : BOOL
      VAR_INPUT s : STRING[4]; w : WSTRING[8]; END_VAR
      SAME_LEN := LEN(s) = LEN(w);
    END_FUNCTION
    FUNCTION PAD : DINT
      VAR_INPUT s : STRING; END_VAR
      VAR_OUTPUT n : DINT; END_VAR
      n := LEN(s);
      PAD := n * 2;
    END_FUNCTION";

/// REQ-LOW-codegen-056: a string argument is copied into the parameter, cut
/// to the parameter's capacity: a literal, a variable and an element of a
/// string array. The callee's write to its parameter does not reach the
/// caller's variable.
#[test]
fn run_when_string_argument_then_copied_and_cut_to_parameter() {
    let source = format!(
        "{FUNCTIONS}
         PROGRAM main
           VAR t : STRING[10] := 'abcdefg'; names : ARRAY[1..2] OF STRING[10]; i : DINT := 2;
               from_literal : DINT; from_variable : DINT; from_element : DINT;
               from_long : DINT; END_VAR
           names[2] := 'xy';
           from_literal := FIRST4('abc');
           from_variable := FIRST4(t);
           from_element := FIRST4(names[i]);
           from_long := FIRST4('abcdefghij');
         END_PROGRAM"
    );
    assert_compiles(&source);
    let snapshot = compile_and_run_agreeing(&source);
    assert_eq!(snapshot.read_as::<i32>("from_literal"), 3);
    assert_eq!(snapshot.read_as::<i32>("from_variable"), 4);
    assert_eq!(snapshot.read_as::<i32>("from_element"), 2);
    assert_eq!(snapshot.read_as::<i32>("from_long"), 4);
    assert_eq!(snapshot.read("t"), "abcdefg");
}

/// REQ-LOW-lowering-072: a `STRING` and a `WSTRING` parameter of one
/// function are each copied at their own encoding, from variables and from
/// literals.
#[test]
fn run_when_string_and_wstring_parameters_then_each_copied_at_its_encoding() {
    let source = format!(
        "{FUNCTIONS}
         PROGRAM main
           VAR s : STRING := 'ab'; w : WSTRING := \"xy\"; same : BOOL; differ : BOOL; END_VAR
           same := SAME_LEN(s, w);
           differ := SAME_LEN(w := \"x\", s := 'abc');
         END_PROGRAM"
    );
    assert_compiles(&source);
    let snapshot = compile_and_run_agreeing(&source);
    assert_eq!(snapshot.read_as::<i32>("same"), 1);
    assert_eq!(snapshot.read_as::<i32>("differ"), 0);
}

/// REQ-LOW-lowering-092: a string input of a call with outputs is copied
/// before the call, which then stores its output and its result.
#[test]
fn run_when_call_with_outputs_has_string_input_then_output_and_result_stored() {
    let source = format!(
        "{FUNCTIONS}
         PROGRAM main VAR r : DINT; k : DINT; END_VAR
           r := PAD('abc', n => k);
         END_PROGRAM"
    );
    assert_compiles(&source);
    let snapshot = compile_and_run_agreeing(&source);
    assert_eq!(snapshot.read_as::<i32>("r"), 6);
    assert_eq!(snapshot.read_as::<i32>("k"), 3);
}

/// REQ-LOW-lowering-076: a method's string inputs are copied after its
/// receiver is pushed, in a call whose result is used, in a call statement,
/// and in a call statement with an output.
#[test]
fn run_when_method_has_string_input_then_copied_into_its_parameter() {
    let source = "
        FUNCTION_BLOCK Named
          VAR_OUTPUT last : DINT; END_VAR
          METHOD Len2 : DINT
            VAR_INPUT v : STRING[4]; w : WSTRING[8]; END_VAR
            Len2 := LEN(v) * 10 + LEN(w);
          END_METHOD
          METHOD Keep
            VAR_INPUT v : STRING[4]; END_VAR
            last := LEN(v);
          END_METHOD
          METHOD Count
            VAR_INPUT v : STRING; END_VAR
            VAR_OUTPUT n : DINT; END_VAR
            n := LEN(v);
          END_METHOD
        END_FUNCTION_BLOCK
        PROGRAM main
          VAR f : Named; t : STRING := 'ab'; r : DINT; kept : DINT; k : DINT; END_VAR
          r := f.Len2(v := 'hello', w := \"pq\");
          f.Keep(v := t);
          kept := f.last;
          f.Count(v := 'abcd', n => k);
        END_PROGRAM";
    assert_compiles(source);
    let snapshot = compile_and_run_agreeing(source);
    assert_eq!(snapshot.read_as::<i32>("r"), 42);
    assert_eq!(snapshot.read_as::<i32>("kept"), 2);
    assert_eq!(snapshot.read_as::<i32>("k"), 4);
}
