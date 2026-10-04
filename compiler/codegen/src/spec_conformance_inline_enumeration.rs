//! Spec conformance tests for inline enumerations
//! (`specs/design/enumeration-codegen.md`, section 10).
//!
//! Kept apart from `spec_conformance.rs`, which holds the rest of that
//! document's requirements, to keep both modules within the size limit.

use ironplc_analyzer::CleanAnalysis;
use ironplc_container::debug_section::iec_type_tag;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::VmBuffers;
use spec_test_macro::spec_test;

use crate::spec_conformance::{compile_and_run, compile_only};

/// Parse, analyze and compile under `options`.
fn try_compile(
    source: &str,
    options: &CompilerOptions,
) -> Result<ironplc_container::Container, ironplc_dsl::diagnostic::Diagnostic> {
    let library = ironplc_parser::parse_program(source, &FileId::default(), options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library], options).unwrap();
    crate::compile(
        CleanAnalysis::new(&analyzed, &ctx).unwrap(),
        &crate::CodegenOptions::default(),
        &crate::EmptyLookup,
    )
}

/// Run one scan of a container.
fn run(container: &ironplc_container::Container) -> VmBuffers {
    let mut bufs = VmBuffers::from_container(container);
    {
        let mut vm = load_and_start(container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
    }
    bufs
}

/// REQ-EN-codegen-090: inline values number as a named list does,
/// explicit member values included.
#[spec_test(REQ_EN_codegen_090)]
fn inline_enum_spec_req_en_090_values_number_as_named_list() {
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    let source = "
PROGRAM main
  VAR
    a : (RED, GREEN, BLUE) := BLUE;
    b : (X := 1, Y := 5) := Y;
  END_VAR
END_PROGRAM
";
    let bufs = run(&try_compile(source, &options).unwrap());
    assert_eq!(bufs.vars[0].as_i32(), 2);
    assert_eq!(bufs.vars[1].as_i32(), 5);
}

/// REQ-EN-codegen-091: an inline enumeration variable is a DINT slot.
#[spec_test(REQ_EN_codegen_091)]
fn inline_enum_spec_req_en_091_variable_is_dint() {
    let source = "
PROGRAM main
  VAR
    a : (RED, GREEN);
  END_VAR
END_PROGRAM
";
    let container = compile_only(source);
    let debug = container.debug_section.as_ref().unwrap();
    assert_eq!(debug.var_names[0].iec_type_tag, iec_type_tag::DINT);
}

/// REQ-EN-codegen-092: an anonymous enumeration's debug type name is made
/// from its type id, and each declaration has an ENUM_DEF entry of its own.
#[spec_test(REQ_EN_codegen_092)]
fn inline_enum_spec_req_en_092_debug_name_from_type_id() {
    let source = "
PROGRAM main
  VAR
    a : (Red, Green);
    b : (Red, Green);
  END_VAR
END_PROGRAM
";
    let container = compile_only(source);
    let debug = container.debug_section.as_ref().unwrap();
    let a = &debug.var_names[0].type_name;
    let b = &debug.var_names[1].type_name;
    assert!(a.starts_with("(ANONYMOUS ENUMERATION "));
    assert_ne!(a, b);
    for name in [a, b] {
        let def = debug
            .enum_defs
            .iter()
            .find(|e| &e.type_name == name)
            .unwrap();
        assert_eq!(def.values, vec!["RED", "GREEN"]);
    }
}

/// REQ-EN-codegen-093: the initial value resolves against the declared
/// type's own members; without one, the type's default (its first member)
/// applies.
#[spec_test(REQ_EN_codegen_093)]
fn inline_enum_spec_req_en_093_initial_value_uses_own_list() {
    let source = "
TYPE COLOR : (RED, GREEN); END_TYPE
PROGRAM main
  VAR
    a : (GREEN, RED) := GREEN;
    b : (GREEN, RED);
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 0);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

/// REQ-EN-codegen-094: a function-local inline enumeration is
/// re-initialized on every call.
#[spec_test(REQ_EN_codegen_094)]
fn inline_enum_spec_req_en_094_function_local_reinitialized_each_call() {
    let source = "
FUNCTION f : DINT
  VAR
    t : (T0, T1) := T1;
  END_VAR
  IF t = T1 THEN f := 7; ELSE f := 3; END_IF;
  t := T0;
END_FUNCTION

PROGRAM main
  VAR
    a : DINT;
    b : DINT;
  END_VAR
  a := f();
  b := f();
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 7);
    assert_eq!(bufs.vars[1].as_i32(), 7);
}

/// REQ-EN-codegen-095: an unqualified value's ordinal is looked up in the
/// type the analyzer gave it from where it is used.
#[spec_test(REQ_EN_codegen_095)]
fn inline_enum_spec_req_en_095_value_uses_its_expression_type() {
    let source = "
TYPE A : (X, Y); B : (W, X, Z); END_TYPE
PROGRAM main
  VAR
    a : (P, Q);
    b : (Q, P);
    c : B;
  END_VAR
  a := Q;
  b := Q;
  c := X;
END_PROGRAM
";
    let bufs = run(&try_compile(source, &CompilerOptions::default()).unwrap());
    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
    assert_eq!(bufs.vars[2].as_i32(), 1);
}
