//! Tests of how a statement is compiled from the lowered program
//! (`specs/design/lowered-program.md`, "Delivery Constraints").

use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, compile_analyzed, parse, Snapshot};

/// A statement lowering does not support yet, a structure copied into an
/// element of an array of structures, is a P9999.
#[test]
fn compile_when_statement_unsupported_then_not_implemented() {
    let source = "
TYPE P : STRUCT x : DINT; END_STRUCT; END_TYPE
PROGRAM main
  VAR arr : ARRAY[1..2] OF P; one : P; END_VAR
  arr[1] := one;
END_PROGRAM
";
    let options = CompilerOptions::default();
    let (library, context) = parse(source, &options);
    let code = compile_analyzed(&library, &context, &options)
        .err()
        .map(|diagnostic| diagnostic.code);
    assert_eq!(code, Some("P9999".to_string()));
}

/// Every control-flow statement lowers, and runs as the source says.
#[test]
fn compile_when_control_flow_then_runs() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE); END_TYPE
PROGRAM main
  VAR i : DINT; s : DINT; c : DINT; w : DINT; r : DINT; k : INT; n : DINT;
      e : COLOR; ce : DINT; END_VAR
  FOR i := 1 TO 10 DO
    IF i = 3 THEN CONTINUE; END_IF;
    IF i > 6 THEN EXIT; END_IF;
    s := s + i;
  END_FOR;
  CASE s OF
    1..10: c := 1;
    18, 20: c := 2;
  ELSE
    c := 3;
  END_CASE;
  WHILE w < 5 DO w := w + 2; END_WHILE;
  REPEAT r := r + 3; UNTIL r > 7 END_REPEAT;
  FOR k := 10 TO 1 BY -3 DO n := n + 1; END_FOR;
  e := BLUE;
  CASE e OF RED: ce := 1; BLUE: ce := 2; END_CASE;
END_PROGRAM
";
    let options = CompilerOptions {
        allow_continue: true,
        ..CompilerOptions::default()
    };
    let (library, context) = parse(source, &options);
    let container = compile_analyzed(&library, &context, &options).unwrap();
    let snapshot = Snapshot::run_compiled(container);
    let read = |name| snapshot.read_as::<i64>(name);
    assert_eq!(read("s"), 18);
    assert_eq!(read("c"), 2);
    assert_eq!(read("w"), 6);
    assert_eq!(read("r"), 9);
    assert_eq!(read("n"), 4);
    assert_eq!(read("ce"), 2);
}

/// REQ-LOW-codegen-124: a lowered `CASE` evaluates its selector once, and
/// tests each label against a copy of it.
#[test]
fn compile_when_case_lowered_then_selector_evaluated_once() {
    let source = "
PROGRAM main
  VAR x : DINT; y : DINT; END_VAR
  CASE x OF
    1, 2: y := 10;
  ELSE
    y := 20;
  END_CASE;
END_PROGRAM
";
    let options = CompilerOptions::default();
    let (library, context) = parse(source, &options);
    let container = compile_analyzed(&library, &context, &options).unwrap();
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert_bytecode!(
        bytecode,
        [
            bc::load_var_i32(0), // the selector, once
            bc::dup(),
            bc::load_const_i32(0), // 1
            bc::eq_i32(),
            bc::jmp_if_not(3), // to the next label
            bc::jmp(8),        // to the arm
            bc::dup(),
            bc::load_const_i32(1), // 2
            bc::eq_i32(),
            bc::jmp_if_not(10), // to the next arm
            bc::pop(),          // the arm
            bc::load_const_i32(2),
            bc::store_var_i32(1),
            bc::jmp(7), // to the end
            bc::pop(),  // the ELSE
            bc::load_const_i32(3),
            bc::store_var_i32(1),
            bc::ret_void(),
        ]
    );
}
