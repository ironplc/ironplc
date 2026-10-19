//! Tests of lowering the prologue of a function or method: the statements
//! at the head of its body that give each variable that starts again on
//! every call its value.

use crate::test_support::lower_whole;
use crate::PartialProgram;

/// `main` calls whatever `source` declares; one task runs it.
fn whole(source: &str) -> PartialProgram {
    lower_whole(&format!(
        "{source}
CONFIGURATION config
  RESOURCE res ON PLC
    TASK t(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM inst WITH t : main;
  END_RESOURCE
END_CONFIGURATION
"
    ))
}

/// The text of each statement of the prologue of the POU named `name`.
fn prologue(program: &PartialProgram, name: &str) -> Vec<String> {
    let pous = program.pous();
    let pou = pous
        .ids()
        .find(|pou| pous.name(*pou).is_some_and(|n| n.to_string() == name))
        .unwrap_or_else(|| panic!("no POU {name}"));
    program
        .prologue(pou)
        .expect("the prologue lowers")
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// REQ-LOW-lowering-195: a function's `VAR`, `VAR_TEMP` and `VAR_OUTPUT`
/// start again on every call, in declaration order, then its result; an
/// input does not, and an output is stored through the reference the call
/// passes for it.
#[test]
fn lower_program_when_function_has_locals_then_prologue_stores_each_then_result() {
    let program = whole(
        "
FUNCTION F : INT
  VAR_INPUT x : INT := 9; END_VAR
  VAR_OUTPUT q : INT := 4; END_VAR
  VAR n : INT := 3; b : BOOL; END_VAR
  VAR_TEMP t : DINT := 7; END_VAR
  F := x + n;
END_FUNCTION
PROGRAM main
  VAR y : INT; z : INT; END_VAR
  y := F(x := 1, q => z);
END_PROGRAM
",
    );

    assert_eq!(
        prologue(&program, "F"),
        [
            "assign v1^ (const i32s 4)",
            "assign v2 (const i32s 3)",
            "assign v3 (const bool false)",
            "assign v4 (const i32s 7)",
            "assign v5 (const i32s 0)",
        ]
    );
}

/// REQ-LOW-lowering-195: a structure local is stored field by field, a
/// string result is stored through the result parameter, and a member
/// initializer that is an expression is lowered in the body it runs in.
#[test]
fn lower_program_when_aggregate_local_and_string_result_then_prologue_stores_them() {
    let program = whole(
        "
TYPE Point : STRUCT a : INT := 1; b : INT; END_STRUCT; END_TYPE
FUNCTION S : STRING[8]
  VAR_INPUT n : INT; END_VAR
  VAR p : Point := (b := n + 1); END_VAR
  S := 'x';
END_FUNCTION
PROGRAM main
  VAR s : STRING[8]; END_VAR
  s := S(n := 2);
END_PROGRAM
",
    );

    assert_eq!(
        prologue(&program, "S"),
        [
            "assign v1.f0 (const i32s 1)",
            "assign v1.f1 (truncate 16 (add i32s (read i32s v0) (const i32s 1)))",
            "assign-str v2^ (literal string[0] \"\")",
        ]
    );
}

/// REQ-LOW-lowering-195: a method's locals and result start again on every
/// call; a program's or function block's body has no prologue.
#[test]
fn lower_program_when_method_then_prologue_stores_locals_and_result() {
    let program = whole(
        "
FUNCTION_BLOCK Fb
  VAR k : INT := 5; END_VAR
  k := k;
  METHOD M : INT
    VAR w : INT := 2; END_VAR
    M := w + k;
  END_METHOD
END_FUNCTION_BLOCK
PROGRAM main
  VAR b : Fb; y : INT; END_VAR
  y := b.M();
END_PROGRAM
",
    );

    assert_eq!(
        prologue(&program, "M"),
        ["assign v2 (const i32s 2)", "assign v3 (const i32s 0)"]
    );
    assert!(prologue(&program, "Fb").is_empty());
    assert!(prologue(&program, "main").is_empty());
}
