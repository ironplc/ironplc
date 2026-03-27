//! End-to-end integration tests for the dialect system.
//!
//! These tests exercise the full pipeline: parse → semantic analysis → codegen → VM execution.
//! They verify that the RuSTy dialect allows Edition 3 keywords (LDT, LTIME, etc.)
//! as identifiers while still supporting REF_TO syntax.

mod common;
use common::{parse, parse_and_run};
use ironplc_parser::options::{CompilerOptions, Dialect};

#[test]
fn end_to_end_when_rusty_dialect_then_ldt_usable_as_variable_name() {
    // In the RuSTy dialect, LDT is demoted to an identifier so it can be
    // used as a variable name (as OSCAT libraries do).
    let source = "
PROGRAM main
VAR
    LDT : DINT := 42;
    result : DINT;
END_VAR
    result := LDT;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::from_dialect(Dialect::Rusty));
    // var layout: LDT=0, result=1
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_rusty_dialect_then_ref_to_works() {
    // The RuSTy dialect enables REF_TO even though Edition 3 types
    // (LTIME, LDT, etc.) are not keywords.
    let source = "
PROGRAM main
VAR
    counter : DINT := 99;
    r : REF_TO DINT := REF(counter);
    result : DINT;
END_VAR
    result := r^;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::from_dialect(Dialect::Rusty));
    // var layout: counter=0, r=1, result=2
    assert_eq!(bufs.vars[2].as_i32(), 99);
}

#[test]
fn end_to_end_when_rusty_dialect_then_ldt_and_ref_to_coexist() {
    // Core OSCAT scenario: LDT used as a variable name alongside REF_TO.
    let source = "
PROGRAM main
VAR
    LDT : DINT := 42;
    r : REF_TO DINT := REF(LDT);
    result : DINT;
END_VAR
    result := r^;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::from_dialect(Dialect::Rusty));
    // var layout: LDT=0, r=1, result=2
    assert_eq!(bufs.vars[2].as_i32(), 42);
}

#[test]
fn end_to_end_when_rusty_dialect_then_struct_with_ldt_member_parses() {
    // Struct member access is not yet implemented in codegen, but we can
    // verify that parsing and semantic analysis succeed with the RuSTy
    // dialect when a struct uses LDT as a member name.
    let source = "
TYPE MY_STRUCT :
  STRUCT
      LDT : DINT;
      value : REAL;
  END_STRUCT;
END_TYPE
PROGRAM main
VAR
    s : MY_STRUCT;
END_VAR
END_PROGRAM
";
    let (_lib, _ctx) = parse(source, &CompilerOptions::from_dialect(Dialect::Rusty));
    // If we get here, parsing and type resolution succeeded.
}

#[test]
fn end_to_end_when_rusty_dialect_then_oscat_style_struct_with_ldt_member_access() {
    // Full OSCAT scenario: struct with LDT as member name, function that reads
    // the member, and a program that writes and reads through the struct.
    // Struct member access is not yet implemented in codegen, so this test
    // verifies parsing and semantic analysis only.
    let source = "
TYPE MY_STRUCT :
  STRUCT
      LDT : DINT;
      value : REAL;
  END_STRUCT;
END_TYPE
FUNCTION MY_FUNC : DINT
VAR_INPUT
    x : MY_STRUCT;
END_VAR
    MY_FUNC := x.LDT;
END_FUNCTION
PROGRAM main
VAR
    s : MY_STRUCT;
    result : DINT;
END_VAR
    s.LDT := 42;
    result := MY_FUNC(x := s);
END_PROGRAM
";
    let (_lib, _ctx) = parse(source, &CompilerOptions::from_dialect(Dialect::Rusty));
    // If we get here, parsing and type resolution succeeded.
}
