//! End-to-end tests of storage keyed by declaration
//! (`specs/design/variable-binding.md`).
//!
//! The analyzer binds every reference to the declaration it names, and
//! codegen gives each declaration storage of its own. A local that hides a
//! global of the same name therefore has its own storage and its own type,
//! whatever kind of variable either is, and a reference through
//! `VAR_EXTERNAL` reaches the global's storage.

use crate::common::try_parse_and_compile;
use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

// A function local of a subrange type hides a global `INT` of the same name:
// the local keeps its own type, so 70000 is not truncated to an `INT`.
e2e_i32!(
    #[spec_test(REQ_VB_codegen_001)]
    end_to_end_when_function_local_subrange_hides_global_int_then_local_type_is_used,
    "
TYPE BIG : DINT(0..100000); END_TYPE
FUNCTION F : DINT VAR x : BIG; END_VAR x := 70000; F := x; END_FUNCTION
PROGRAM main VAR r : DINT; END_VAR r := F(); END_PROGRAM
CONFIGURATION config
  VAR_GLOBAL x : INT; END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION
",
    &[("r", 70000)],
);

// A function local scalar hides a global structure of the same name.
e2e_i32!(
    #[spec_test(REQ_VB_codegen_002)]
    end_to_end_when_function_local_scalar_hides_global_structure_then_local_is_used,
    "
TYPE POINT : STRUCT px : DINT; py : DINT; END_STRUCT; END_TYPE
FUNCTION F : DINT VAR p : DINT; END_VAR p := 3; F := p; END_FUNCTION
PROGRAM main VAR r : DINT; END_VAR r := F(); END_PROGRAM
CONFIGURATION config
  VAR_GLOBAL p : POINT; END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION
",
    &[("r", 3)],
);

// A function local scalar hides a global string of the same name.
e2e_i32!(
    #[spec_test(REQ_VB_codegen_003)]
    end_to_end_when_function_local_scalar_hides_global_string_then_local_is_used,
    "
FUNCTION F : DINT VAR s : DINT; END_VAR s := 3; F := s; END_FUNCTION
PROGRAM main VAR r : DINT; END_VAR r := F(); END_PROGRAM
CONFIGURATION config
  VAR_GLOBAL s : STRING; END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION
",
    &[("r", 3)],
);

// A function block reads a global array through `VAR_EXTERNAL`: the
// reference is bound to the global, so it reaches the global's storage.
e2e_i32!(
    #[spec_test(REQ_VB_codegen_010)]
    end_to_end_when_function_block_reads_global_array_through_var_external_then_reads_global,
    "
FUNCTION_BLOCK FB
VAR_EXTERNAL g : ARRAY[1..3] OF DINT; END_VAR
VAR_OUTPUT o : DINT; END_VAR
o := g[3];
END_FUNCTION_BLOCK
PROGRAM main VAR q : DINT; b : FB; END_VAR b(); q := b.o; END_PROGRAM
CONFIGURATION config
  VAR_GLOBAL g : ARRAY[1..3] OF DINT := [10, 20, 30]; END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION
",
    &[("q", 30)],
);

// A function block writes a global scalar through `VAR_EXTERNAL`, and the
// program reads it through its own `VAR_EXTERNAL`: both are the global.
e2e_i32!(
    #[spec_test(REQ_VB_codegen_011)]
    end_to_end_when_function_block_writes_global_through_var_external_then_program_reads_it,
    "
FUNCTION_BLOCK FB
VAR_EXTERNAL g : DINT; END_VAR
g := 42;
END_FUNCTION_BLOCK
PROGRAM main VAR_EXTERNAL g : DINT; END_VAR VAR r : DINT; b : FB; END_VAR b(); r := g; END_PROGRAM
CONFIGURATION config
  VAR_GLOBAL g : DINT; END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION
",
    &[("r", 42)],
);

// Two functions declare locals of the same name: each has its own storage.
e2e_i32!(
    #[spec_test(REQ_VB_codegen_005)]
    end_to_end_when_two_functions_declare_same_local_name_then_each_has_its_own,
    "
FUNCTION F : DINT VAR x : DINT; END_VAR x := 1; F := x; END_FUNCTION
FUNCTION G : DINT VAR x : DINT; END_VAR x := 2; G := x + F(); END_FUNCTION
PROGRAM main VAR r : DINT; END_VAR r := G(); END_PROGRAM
",
    &[("r", 3)],
);

/// A function local structure that hides a program's function block
/// instance of the same name is the function's own variable, not the
/// program's instance. Codegen does not give a function-local structure
/// storage yet, so the reference is reported as not implemented rather than
/// reading the instance's field.
#[spec_test(REQ_VB_codegen_004)]
#[test]
fn compile_when_function_local_structure_hides_program_instance_then_not_implemented() {
    let source = "
TYPE REC : STRUCT PT : TIME; END_STRUCT; END_TYPE
FUNCTION F : TIME VAR t : REC; END_VAR F := t.PT; END_FUNCTION
PROGRAM main VAR t : TON := (PT := T#5s); r : TIME; END_VAR r := F(); END_PROGRAM
";

    let result = try_parse_and_compile(source, &CompilerOptions::default());

    assert!(result.is_err_and(|diagnostic| diagnostic.code == "P9999"));
}

/// A derived function block's reference to an inherited field names the
/// base block's field. Its storage is the base type's frame, which the
/// derived body cannot address, so it is reported as not implemented even
/// when the base type's body is compiled first.
#[spec_test(REQ_VB_codegen_020)]
#[test]
fn compile_when_base_compiled_before_derived_reads_inherited_field_then_not_implemented() {
    let source = "
FUNCTION_BLOCK BASE VAR count : INT; END_VAR count := count + 1; END_FUNCTION_BLOCK
FUNCTION_BLOCK DERIVED EXTENDS BASE count := 5; END_FUNCTION_BLOCK
PROGRAM main VAR b : BASE; d : DERIVED; END_VAR b(); d(); END_PROGRAM
";
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };

    let result = try_parse_and_compile(source, &options);

    assert!(result.is_err_and(|diagnostic| diagnostic.code == "P9999"));
}
