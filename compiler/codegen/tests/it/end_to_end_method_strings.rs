//! End-to-end tests of `STRING` and `WSTRING` variables declared in a
//! `METHOD`. Each is held in a run of the data region of its own, sized to
//! its declared capacity, as a function's is: a local or output starts again
//! from its initial value at every call, and a call copies a string input
//! into its parameter's run.

use ironplc_parser::options::CompilerOptions;

use crate::common::assert_run_with;

fn options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

/// Runs a program that calls `M` of `FB` once, returning the result in
/// `r`, where `M` is a method with `declarations` and `body`, and fails
/// unless `r` is `expected`.
fn assert_method_returns(declarations: &str, body: &str, expected: i64) {
    let source = format!(
        "FUNCTION_BLOCK FB
           METHOD M : DINT
             {declarations}
             {body}
           END_METHOD
         END_FUNCTION_BLOCK
         PROGRAM main VAR f : FB; r : DINT; END_VAR
           r := f.M();
         END_PROGRAM"
    );
    assert_run_with(&source, &options(), &[("r", expected)]);
}

/// A string assigned a longer value holds its declared capacity, as a
/// function's or program's does.
#[test]
fn end_to_end_when_method_string_assigned_longer_value_then_cut_to_capacity() {
    assert_method_returns(
        "VAR s : STRING[8]; w : WSTRING[4]; END_VAR",
        "s := 'abcdefghij'; w := \"abcdef\"; M := LEN(s) * 10 + LEN(w);",
        84,
    );
}

/// Two strings of one method are held apart: writing the second leaves the
/// first as it was.
#[test]
fn end_to_end_when_method_has_two_strings_then_each_keeps_its_own_value() {
    assert_method_returns(
        "VAR s1 : STRING[8]; s2 : STRING[8]; x : DINT; END_VAR",
        "s1 := 'a'; x := LEN(s1); s2 := 'bb'; M := LEN(s1) * 10 + LEN(s2);",
        12,
    );
}

/// A string assigned on every iteration of a loop needs no more storage
/// than one assignment does.
#[test]
fn end_to_end_when_method_string_assigned_in_loop_then_compiles_and_runs() {
    assert_method_returns(
        "VAR s : STRING[8]; i : DINT; END_VAR",
        "FOR i := 1 TO 1000 DO s := 'a'; END_FOR; M := LEN(s);",
        1,
    );
}

/// A string local and a string output start again from their initial
/// values at every call.
#[test]
fn end_to_end_when_method_called_twice_then_string_local_starts_from_initial_value() {
    let source = "
        FUNCTION_BLOCK FB
          METHOD M : DINT
            VAR_OUTPUT o : STRING[8]; END_VAR
            VAR s : STRING[8] := 'q'; END_VAR
            M := LEN(s) * 10 + LEN(o);
            s := 'abc';
            o := 'xyz';
          END_METHOD
        END_FUNCTION_BLOCK
        PROGRAM main VAR f : FB; r1 : DINT; r2 : DINT; END_VAR
          r1 := f.M();
          r2 := f.M();
        END_PROGRAM";
    assert_run_with(source, &options(), &[("r1", 10i64), ("r2", 10i64)]);
}

/// A call copies a string argument into the method's string input, cut to
/// the input's capacity: a literal, a variable and a `WSTRING`.
#[test]
fn end_to_end_when_method_has_string_input_then_call_copies_argument() {
    let source = "
        FUNCTION_BLOCK FB
          METHOD Len2 : DINT
            VAR_INPUT v : STRING[4]; w : WSTRING[8]; END_VAR
            Len2 := LEN(v) * 10 + LEN(w);
            IF v = 'ab' THEN Len2 := Len2 + 100; END_IF;
          END_METHOD
        END_FUNCTION_BLOCK
        PROGRAM main
          VAR f : FB; t : STRING := 'ab'; u : WSTRING := \"xyz\"; r1 : DINT; r2 : DINT; END_VAR
          r1 := f.Len2(v := 'hello', w := \"pq\");
          r2 := f.Len2(t, u);
        END_PROGRAM";
    assert_run_with(source, &options(), &[("r1", 42i64), ("r2", 123i64)]);
}

/// A name a method declares as a string is no string in another method of
/// the block, which declares it as a `DINT`.
#[test]
fn end_to_end_when_sibling_method_reuses_string_name_then_it_is_its_own_type() {
    let source = "
        FUNCTION_BLOCK FB
          METHOD A : DINT
            VAR s : STRING[2]; END_VAR
            s := 'xyz';
            A := LEN(s);
          END_METHOD
          METHOD B : DINT
            VAR s : DINT; END_VAR
            s := 5;
            B := s;
          END_METHOD
        END_FUNCTION_BLOCK
        PROGRAM main VAR f : FB; ra : DINT; rb : DINT; END_VAR
          ra := f.A();
          rb := f.B();
        END_PROGRAM";
    assert_run_with(source, &options(), &[("ra", 2i64), ("rb", 5i64)]);
}
