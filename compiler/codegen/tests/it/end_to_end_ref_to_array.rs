//! End-to-end integration tests for REF_TO ARRAY support.
//! Compiles ST programs with references to array types and runs them through the VM.

use crate::common::parse_and_compile;
use ironplc_parser::options::{CompilerOptions, Dialect};

#[test]
fn end_to_end_when_ref_to_array_declared_then_compiles() {
    let source = "
PROGRAM main
  VAR
    data : REF_TO ARRAY[1..5] OF INT;
    marker : INT := 42;
  END_VAR
END_PROGRAM
";
    let _container = parse_and_compile(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
}

#[test]
fn end_to_end_when_ref_to_array_type_decl_then_compiles() {
    let source = "
TYPE ArrRef : REF_TO ARRAY[0..3] OF DINT; END_TYPE

PROGRAM main
  VAR
    arr : ArrRef;
    result : DINT := 7;
  END_VAR
END_PROGRAM
";
    let _container = parse_and_compile(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
}

// x is at var index 1 (data is var 0, x is var 1)
e2e_i32_with!(
    end_to_end_when_ref_to_array_declared_then_runs,
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    "
PROGRAM main
  VAR
    data : REF_TO ARRAY[0..3] OF INT;
    x : INT := 99;
  END_VAR
END_PROGRAM
",
    &[(1, 99)],
);

// A REF_TO whose target is a named array type behaves exactly as the inline
// spelling (#1580). var layout: arr=0, pt=1, v=2, i=3
e2e_i32_with!(
    end_to_end_when_ref_to_named_array_type_deref_subscript_read_then_reads_element,
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    "
TYPE ARR4 : ARRAY[0..3] OF INT; END_TYPE

PROGRAM main
  VAR
    arr : ARR4;
    pt : REF_TO ARR4 := REF(arr);
    v : INT;
    i : INT := 1;
  END_VAR
  arr[1] := 77;
  v := pt^[i];
END_PROGRAM
",
    &[(2, 77)],
);

// var layout: arr=0, pt=1, v=2
e2e_i32_with!(
    end_to_end_when_ref_to_named_array_type_deref_subscript_write_then_writes_through_ref,
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    "
TYPE ARR4 : ARRAY[0..3] OF INT; END_TYPE

PROGRAM main
  VAR
    arr : ARR4;
    pt : REF_TO ARR4 := REF(arr);
    v : INT;
  END_VAR
  pt^[2] := 55;
  v := arr[2];
END_PROGRAM
",
    &[(2, 55)],
);

// var layout: arr=0, v=1
e2e_i32_with!(
    end_to_end_when_function_param_ref_to_named_array_type_then_reads_element,
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    "
TYPE ARR4 : ARRAY[0..3] OF INT; END_TYPE

FUNCTION GET_ELEMENT : INT
  VAR_INPUT
    pt : REF_TO ARR4;
    i : INT;
  END_VAR
  GET_ELEMENT := pt^[i];
END_FUNCTION

PROGRAM main
  VAR
    arr : ARR4;
    v : INT;
  END_VAR
  arr[3] := 31;
  v := GET_ELEMENT(pt := REF(arr), i := 3);
END_PROGRAM
",
    &[(1, 31)],
);

// var layout: arr=0, fb=1, check=2
e2e_i32_with!(
    end_to_end_when_fb_local_ref_to_named_array_type_then_writes_through_ref,
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    "
TYPE ARR5 : ARRAY[0..4] OF INT; END_TYPE

FUNCTION_BLOCK COPY_FB
  VAR_INPUT
    src : REF_TO ARR5;
  END_VAR
  VAR
    local_pt : REF_TO ARR5;
  END_VAR
  local_pt := src;
  local_pt^[0] := 99;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    arr : ARR5;
    fb : COPY_FB;
    check : INT;
  END_VAR
  fb(src := REF(arr));
  check := arr[0];
END_PROGRAM
",
    &[(2, 99)],
);

// The dimensions come from the type environment, so strides must be right
// for more than one dimension. var layout: g=0, pt=1, v=2
e2e_i32_with!(
    end_to_end_when_ref_to_named_two_dimensional_array_type_then_reads_element,
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    "
TYPE GRID : ARRAY[0..1, 0..2] OF INT; END_TYPE

PROGRAM main
  VAR
    g : GRID;
    pt : REF_TO GRID := REF(g);
    v : INT;
  END_VAR
  g[1, 2] := 42;
  v := pt^[1, 2];
END_PROGRAM
",
    &[(2, 42)],
);

// A named reference type whose target is itself a named array type reaches
// codegen with the named target, so it takes the same path.
// var layout: arr=0, pt=1, v=2
e2e_i32_with!(
    end_to_end_when_ref_type_alias_of_named_array_type_then_reads_element,
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    "
TYPE
  ARR4 : ARRAY[0..3] OF INT;
  ArrRef : REF_TO ARR4;
END_TYPE

PROGRAM main
  VAR
    arr : ARR4;
    pt : ArrRef;
    v : INT;
  END_VAR
  arr[0] := 12;
  pt := REF(arr);
  v := pt^[0];
END_PROGRAM
",
    &[(2, 12)],
);
