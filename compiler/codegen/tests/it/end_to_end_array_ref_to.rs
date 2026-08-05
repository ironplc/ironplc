//! End-to-end integration tests for ARRAY OF REF_TO support.
//! Compiles ST programs with arrays of reference types and runs them through the VM.

use crate::common::parse_and_compile;
use ironplc_parser::options::{CompilerOptions, Dialect};

// x is at var index 1 (data is var 0, x is var 1)
e2e_i32_with!(
    end_to_end_when_array_of_ref_to_declared_then_compiles_and_runs,
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    "PROGRAM main VAR data : ARRAY[0..3] OF REF_TO BYTE; x : INT := 42; END_VAR END_PROGRAM",
    &[(1, 42)],
);

// val=0, refs=1, result=2
e2e_i32_with!(
    end_to_end_when_array_of_ref_to_store_ref_then_roundtrips,
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    "PROGRAM main VAR val : INT := 77; refs : ARRAY[0..2] OF REF_TO INT; result : INT; END_VAR refs[0] := REF(val); result := refs[0]^; END_PROGRAM",
    &[(2, 77)],
);

#[test]
fn end_to_end_when_array_of_ref_to_int_then_compiles() {
    let source = "
PROGRAM main
  VAR
    refs : ARRAY[1..5] OF REF_TO INT;
    marker : INT := 99;
  END_VAR
END_PROGRAM
";
    let _container = parse_and_compile(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
}

#[test]
fn end_to_end_when_array_of_ref_to_type_decl_then_compiles() {
    let source = "
TYPE RefArray : ARRAY[0..2] OF REF_TO DINT; END_TYPE

PROGRAM main
  VAR
    arr : RefArray;
    result : DINT := 7;
  END_VAR
END_PROGRAM
";
    let _container = parse_and_compile(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
}
