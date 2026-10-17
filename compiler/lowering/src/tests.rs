//! Tests of lowering, from analyzed source to the text of the lowered
//! statements.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it. They become `#[spec_test]`s when this crate lists the design in
//! a build script, once every requirement the crate owns is tested.

use crate::test_support::{lower, lower_bodies};
use ironplc_ir::lowered::Owner;

/// REQ-LOW-lowering-034: a program's variable is a field of its instance,
/// reached through the body's instance parameter `v0`.
#[test]
fn lower_statement_when_program_variable_then_field_of_instance() {
    let outcome = lower(
        "PROGRAM main VAR a : DINT; b : DINT; END_VAR
            b := 5;
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["assign v0^.f1 (const i32s 5)"]);
    assert_eq!(outcome.variables[0].name().to_string(), "THIS");
}

/// REQ-LOW-lowering-046: a value stored narrower than its operation width is
/// truncated, and one that already fits is not.
#[test]
fn lower_statement_when_narrow_target_then_truncated_unless_fits() {
    let outcome = lower(
        "PROGRAM main VAR a : INT; b : INT; s : SINT; END_VAR
            a := a + b;
            a := b;
            a := s;
            a := 7;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (truncate 16 (add i32s (read i32s v0^.f0) (read i32s v0^.f1)))",
            "assign v0^.f0 (read i32s v0^.f1)",
            "assign v0^.f0 (read i32s v0^.f2)",
            "assign v0^.f0 (const i32s 7)",
        ]
    );
}

/// REQ-LOW-lowering-045, REQ-LOW-lowering-094: the conversion the analyzer
/// recorded becomes a `Convert`, also where only the signedness changes, and
/// one between types operated alike converts nothing.
#[test]
fn lower_statement_when_recorded_conversion_then_convert() {
    let outcome = lower(
        "PROGRAM main VAR d : DINT; l : LINT; i : INT; r : LREAL; u : USINT; END_VAR
            l := d;
            d := i;
            r := d;
            d := u;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f1 (convert i64s (read i32s v0^.f0))",
            "assign v0^.f0 (read i32s v0^.f2)",
            "assign v0^.f3 (convert f64 (read i32s v0^.f0))",
            "assign v0^.f0 (convert i32s (read i32u v0^.f4))",
        ]
    );
}

/// REQ-LOW-lowering-048: `NOT`, `AND`, `OR` and `XOR` are logical on a
/// `BOOL` and bitwise on a bit string.
#[test]
fn lower_statement_when_logical_operators_then_form_follows_type() {
    let outcome = lower(
        "PROGRAM main VAR x : BOOL; y : BOOL; w : DWORD; v : DWORD; END_VAR
            x := NOT y;
            x := x AND y;
            x := TRUE;
            w := NOT v;
            w := w XOR v;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (bool_not bool (read bool v0^.f1))",
            "assign v0^.f0 (bool_and bool (read bool v0^.f0) (read bool v0^.f1))",
            "assign v0^.f0 (const bool true)",
            "assign v0^.f2 (bit_not i32u (read i32u v0^.f3))",
            "assign v0^.f2 (bit_xor i32u (read i32u v0^.f2) (read i32u v0^.f3))",
        ]
    );
}

/// Inverting a `BYTE` sets the bits above its width, so `NOT` of one is
/// narrowed back to its width at once, and `NOT` of a `DWORD` is not.
#[test]
fn lower_statement_when_not_of_narrow_bit_string_then_truncated() {
    let outcome = lower(
        "PROGRAM main VAR b : BYTE; c : BYTE; x : BOOL; END_VAR
            x := NOT b = c;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["assign v0^.f2 (eq (truncate 8 (bit_not i32u (read i32u v0^.f0))) (read i32u v0^.f1))"]
    );
}

/// `AND_THEN` and `OR_ELSE` short-circuit on a `BOOL`, and are the eager
/// bitwise operators on a bit string.
#[test]
fn lower_statement_when_short_circuit_operator_then_form_follows_type() {
    let outcome = lower(
        "PROGRAM main VAR x : BOOL; y : BOOL; w : DWORD; v : DWORD; END_VAR
            x := x AND_THEN y;
            w := w OR_ELSE v;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (and_then (read bool v0^.f0) (read bool v0^.f1))",
            "assign v0^.f2 (bit_or i32u (read i32u v0^.f2) (read i32u v0^.f3))",
        ]
    );
}

/// REQ-LOW-lowering-043: a comparison yields a `BOOL`.
#[test]
fn lower_statement_when_comparison_then_compare_of_one_type() {
    let outcome = lower(
        "PROGRAM main VAR x : BOOL; a : DINT; b : LINT; END_VAR
            x := a < 3;
            x := a = b;
            x := x AND_THEN (a > 0);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (lt (read i32s v0^.f1) (const i32s 3))",
            "assign v0^.f0 (eq (convert i64s (read i32s v0^.f1)) (read i64s v0^.f2))",
            "assign v0^.f0 (and_then (read bool v0^.f0) (gt (read i32s v0^.f1) (const i32s 0)))",
        ]
    );
}

/// An integer literal at a real type is that number, and a real literal a
/// real constant.
#[test]
fn lower_statement_when_literals_at_real_type_then_real_constants() {
    let outcome = lower(
        "PROGRAM main VAR r : REAL; END_VAR
            r := 2;
            r := 1.5;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (const f32 2.0)",
            "assign v0^.f0 (const f32 1.5)",
        ]
    );
}

/// REQ-LOW-lowering-034: a `VAR_TEMP` is a variable of its own, not a field.
#[test]
fn lower_statement_when_var_temp_then_own_variable() {
    let outcome = lower(
        "FUNCTION_BLOCK block VAR a : DINT; END_VAR VAR_TEMP t : DINT; END_VAR
            t := 1;
            a := t;
         END_FUNCTION_BLOCK
         PROGRAM main VAR b : block; END_VAR b(); END_PROGRAM",
        "block",
    );
    assert_eq!(
        outcome.statements,
        ["assign v1 (const i32s 1)", "assign v0^.f0 (read i32s v1)",]
    );
    assert_eq!(outcome.variables[1].name().to_string(), "t");
}

/// REQ-LOW-lowering-034: a function block's variable is a field of its
/// instance, in the order the block declares its variables.
#[test]
fn lower_statement_when_function_block_variable_then_field_of_instance() {
    let outcome = lower(
        "FUNCTION_BLOCK counter
            VAR_INPUT step : DINT; END_VAR
            VAR_OUTPUT count : DINT; END_VAR
            count := count + step;
         END_FUNCTION_BLOCK
         PROGRAM main VAR c : counter; END_VAR c(step := 1); END_PROGRAM",
        "counter",
    );
    assert_eq!(
        outcome.statements,
        ["assign v0^.f1 (add i32s (read i32s v0^.f1) (read i32s v0^.f0))"]
    );
}

/// REQ-LOW-lowering-170: a function block's array, reference, string and
/// inline enumeration variables are fields of its instance like any other,
/// each at its place in the order the block declares them.
#[test]
fn lower_statement_when_function_block_variable_of_any_type_then_field_in_declared_order() {
    let outcome = lower(
        "FUNCTION_BLOCK holder
            VAR_INPUT src : REF_TO ARRAY[0..4] OF INT; END_VAR
            VAR a : ARRAY[0..4] OF INT; own : REF_TO DINT; n : DINT; s : STRING[8];
                e : (P, Q); END_VAR
            own := REF(n);
            own^ := own^ + 1;
            src^[1] := a[2];
            CASE e OF Q: n := 2; END_CASE;
         END_FUNCTION_BLOCK
         PROGRAM main VAR h : holder; END_VAR h(); END_PROGRAM",
        "holder",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f2 (ref_to v0^.f3)",
            "assign v0^.f2^ (add i32s (read i32s v0^.f2^) (const i32s 1))",
            "assign v0^.f0^[(const i32s 1)] (read i32s v0^.f1[(const i32s 2)])",
            "case (read i32s v0^.f5)\nof 1\n  assign v0^.f3 (const i32s 2)\nend",
        ]
    );
}

/// REQ-LOW-lowering-170: a function block has the same type whether its body
/// is lowered or an instance of it is a field: an output declared after an
/// array is the second field of the instance a program holds.
#[test]
fn lower_statement_when_field_of_block_instance_after_array_then_its_declared_position() {
    let outcome = lower(
        "FUNCTION_BLOCK block VAR a : ARRAY[1..2] OF DINT; END_VAR VAR_OUTPUT o : DINT; END_VAR
         END_FUNCTION_BLOCK
         PROGRAM main VAR b : block; out : DINT; END_VAR out := b.o; END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["assign v0^.f1 (read i32s v0^.f0.f1)"]);
}

/// REQ-LOW-lowering-020: two variables of one name in different scopes are
/// different variables.
#[test]
fn lower_statement_when_same_name_in_two_functions_then_different_variables() {
    let outcome = lower_bodies(
        "FUNCTION f1 : DINT VAR x : DINT; END_VAR x := 1; f1 := x; END_FUNCTION
         FUNCTION f2 : DINT VAR x : DINT; END_VAR x := 2; f2 := x; END_FUNCTION
         PROGRAM main VAR a : DINT; END_VAR a := f1() + f2(); END_PROGRAM",
        &["f1", "f2"],
    );
    // `v1` and `v3` are the result variables of `f1` and `f2`.
    assert_eq!(
        outcome.statements,
        [
            "assign v0 (const i32s 1)",
            "assign v1 (read i32s v0)",
            "assign v2 (const i32s 2)",
            "assign v3 (read i32s v2)",
        ]
    );
    assert_eq!(outcome.variables[0].name().to_string(), "x");
    assert_eq!(outcome.variables[2].name().to_string(), "x");
}

/// REQ-LOW-lowering-022: a `VAR_EXTERNAL` names the global it refers to, so
/// two bodies that reach one global through it reach one variable.
#[test]
fn lower_statement_when_var_external_then_global_variable() {
    let outcome = lower_bodies(
        "CONFIGURATION config
            VAR_GLOBAL g : DINT; END_VAR
            RESOURCE res ON PLC
              TASK t (INTERVAL := T#100ms, PRIORITY := 1);
              PROGRAM p1 WITH t : first;
              PROGRAM p2 WITH t : second;
            END_RESOURCE
         END_CONFIGURATION
         PROGRAM first VAR_EXTERNAL g : DINT; END_VAR g := 1; END_PROGRAM
         PROGRAM second VAR_EXTERNAL g : DINT; END_VAR g := 2; END_PROGRAM",
        &["first", "second"],
    );
    assert_eq!(
        outcome.statements,
        ["assign v1 (const i32s 1)", "assign v1 (const i32s 2)"]
    );
    assert_eq!(outcome.variables[1].owner(), Owner::Global);
    assert_eq!(outcome.variables[1].name().to_string(), "g");
}

/// What lowering does not support yet is unsupported, not a defect.
#[test]
fn lower_statement_when_not_supported_yet_then_unsupported() {
    let outcome = lower(
        "TYPE P : STRUCT x : DINT; END_STRUCT; END_TYPE
         FUNCTION MakeP : P
           MakeP.x := 1;
         END_FUNCTION
         PROGRAM main VAR a : DINT; r : REAL; p : P; END_VAR
            p := MakeP();
            FOR a := 1 TO 3 BY a DO r := r; END_FOR;
         END_PROGRAM",
        "main",
    );
    assert!(
        outcome.statements.iter().all(|stmt| stmt == "unsupported"),
        "{:?}",
        outcome.statements
    );
}

/// REQ-LOW-lowering-082: the crate denies wildcard arms over the lowered
/// program's enums. A test cannot observe a lint that is not configured, so
/// it checks the crate root.
#[test]
fn crate_root_when_read_then_denies_wildcard_enum_match_arm() {
    assert!(include_str!("lib.rs").contains("#![deny(clippy::wildcard_enum_match_arm)]"));
}
