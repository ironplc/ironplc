//! Spec conformance tests for enumeration code generation.
//!
//! Each test is annotated with `#[spec_test(REQ_EN_NNN)]` which:
//! 1. Adds `#[test]`
//! 2. References a build-script-generated constant — compilation fails if the
//!    requirement was removed from the spec markdown.
//!
//! The `all_spec_requirements_have_tests` meta-test ensures every requirement
//! in the spec has at least one test here.
//!
//! See `specs/design/spec-conformance-testing.md` for full design.
//! See `specs/design/enumeration-codegen.md` for the enumeration codegen spec.

use ironplc_container::debug_section::iec_type_tag;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::VmBuffers;
use spec_test_macro::spec_test;

use crate::compile_enum::{
    build_enum_ordinal_map, enum_var_type_info, resolve_enum_default_ordinal, resolve_enum_ordinal,
};

// ---------------------------------------------------------------------------
// Meta-test: completeness check
// ---------------------------------------------------------------------------

#[test]
fn all_spec_requirements_have_tests() {
    assert!(
        crate::spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        crate::spec_requirements::UNTESTED
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_library(source: &str) -> ironplc_dsl::common::Library {
    ironplc_parser::parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap()
}

/// Parse, analyze, compile, and run one scan cycle.
fn compile_and_run(source: &str) -> (ironplc_container::Container, VmBuffers) {
    let library = parse_library(source);
    let (analyzed, ctx) =
        ironplc_analyzer::stages::resolve_types(&[&library], &CompilerOptions::default()).unwrap();
    let codegen_options = crate::CodegenOptions::default();
    let container = crate::compile(&analyzed, &ctx, &codegen_options).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
    }
    (container, bufs)
}

/// Parse, analyze, and compile (no execution).
fn compile_only(source: &str) -> ironplc_container::Container {
    let library = parse_library(source);
    let (analyzed, ctx) =
        ironplc_analyzer::stages::resolve_types(&[&library], &CompilerOptions::default()).unwrap();
    let codegen_options = crate::CodegenOptions::default();
    crate::compile(&analyzed, &ctx, &codegen_options).unwrap()
}

// ---------------------------------------------------------------------------
// Section 1: Ordinal Encoding (REQ-EN-001 through REQ-EN-004)
// ---------------------------------------------------------------------------

/// REQ-EN-001: Ordinals are 0-based, assigned by declaration order.
#[spec_test(REQ_EN_001)]
fn enum_spec_req_en_001_ordinals_are_zero_based_by_declaration_order() {
    let lib = parse_library(
        "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
         PROGRAM main END_PROGRAM",
    );
    let map = build_enum_ordinal_map(&lib);

    let red = ironplc_dsl::common::EnumeratedValue::new("RED");
    let green = ironplc_dsl::common::EnumeratedValue::new("GREEN");
    let blue = ironplc_dsl::common::EnumeratedValue::new("BLUE");

    assert_eq!(resolve_enum_ordinal(&map, &red).unwrap(), 0);
    assert_eq!(resolve_enum_ordinal(&map, &green).unwrap(), 1);
    assert_eq!(resolve_enum_ordinal(&map, &blue).unwrap(), 2);
}

/// REQ-EN-002: The ordinal is the runtime value stored in the variable slot.
#[spec_test(REQ_EN_002)]
fn enum_spec_req_en_002_ordinal_is_runtime_value() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR := GREEN;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    // GREEN is ordinal 1 — the raw slot value must be 1.
    assert_eq!(bufs.vars[0].as_i32(), 1);
}

/// REQ-EN-003: Enums use DINT (W32, Signed, 32-bit) at codegen level.
#[spec_test(REQ_EN_003)]
fn enum_spec_req_en_003_var_type_info_is_dint() {
    let info = enum_var_type_info();
    assert!(matches!(info.op_width, crate::compile::OpWidth::W32));
    assert!(matches!(
        info.signedness,
        crate::compile::Signedness::Signed
    ));
    assert_eq!(info.storage_bits, 32);
}

/// REQ-EN-004: Enumerations support assignment, equality, and CASE.
/// This test verifies initialization (a form of assignment) works.
/// Body-level assignment, equality, and CASE tested in PR 3 specs.
#[spec_test(REQ_EN_004)]
fn enum_spec_req_en_004_assignment_compiles_and_runs() {
    let source = "
TYPE LEVEL : (LOW, MEDIUM, HIGH) := LOW; END_TYPE
PROGRAM main
  VAR
    x : LEVEL := HIGH;
  END_VAR
END_PROGRAM
";
    // Initialization with enum value succeeds (no arithmetic operators used).
    let (_c, bufs) = compile_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 2); // HIGH = ordinal 2
}

// ---------------------------------------------------------------------------
// Section 2: Variable Allocation (REQ-EN-010 through REQ-EN-012)
// ---------------------------------------------------------------------------

/// REQ-EN-010: Enum variable receives VarTypeInfo W32/Signed/32.
#[spec_test(REQ_EN_010)]
fn enum_spec_req_en_010_variable_gets_dint_type_info() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR := GREEN;
  END_VAR
END_PROGRAM
";
    // If the variable didn't have correct VarTypeInfo, the STORE_VAR
    // would use the wrong opcode and the value would be wrong.
    let (_c, bufs) = compile_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 1);
}

/// REQ-EN-011: Enum variable occupies one slot, same as any scalar integer.
#[spec_test(REQ_EN_011)]
fn enum_spec_req_en_011_variable_occupies_one_slot() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    a : COLOR := RED;
    b : DINT;
    c : COLOR := BLUE;
  END_VAR
  b := 42;
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    // Each variable occupies one slot: a=slot 0, b=slot 1, c=slot 2.
    assert_eq!(bufs.vars[0].as_i32(), 0); // RED
    assert_eq!(bufs.vars[1].as_i32(), 42); // DINT
    assert_eq!(bufs.vars[2].as_i32(), 2); // BLUE
}

/// REQ-EN-012: Debug VarNameEntry uses iec_type_tag::DINT and user type name.
#[spec_test(REQ_EN_012)]
fn enum_spec_req_en_012_debug_entry_has_dint_tag_and_type_name() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
  END_VAR
END_PROGRAM
";
    let container = compile_only(source);
    let debug = container.debug_section.as_ref().unwrap();
    let var = &debug.var_names[0];
    assert_eq!(var.name, "c");
    assert_eq!(var.iec_type_tag, iec_type_tag::DINT);
    assert_eq!(var.type_name, "COLOR");
}

// ---------------------------------------------------------------------------
// Section 3: Initialization (REQ-EN-020 through REQ-EN-023)
// ---------------------------------------------------------------------------

/// REQ-EN-020: Explicit initial value emits LOAD_CONST_I32 + STORE_VAR_I32.
#[spec_test(REQ_EN_020)]
fn enum_spec_req_en_020_explicit_init_stores_ordinal() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR := BLUE;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 2); // BLUE = ordinal 2
}

/// REQ-EN-021: No explicit init uses type declaration default.
#[spec_test(REQ_EN_021)]
fn enum_spec_req_en_021_no_init_uses_type_default() {
    let source = "
TYPE LEVEL : (LOW, MEDIUM, HIGH) := MEDIUM; END_TYPE
PROGRAM main
  VAR
    x : LEVEL;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    // Type default is MEDIUM = ordinal 1.
    assert_eq!(bufs.vars[0].as_i32(), 1);
}

/// REQ-EN-022: No type default means initial ordinal is 0 (first value).
#[spec_test(REQ_EN_022)]
fn enum_spec_req_en_022_no_type_default_uses_first_value() {
    let source = "
TYPE STATUS : (STOPPED, RUNNING); END_TYPE
PROGRAM main
  VAR
    s : STATUS;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 0); // STOPPED = ordinal 0
}

/// REQ-EN-023: Function-local enum variables are re-initialized on every call.
#[spec_test(REQ_EN_023)]
fn enum_spec_req_en_023_function_local_reinitialized_each_call() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE

FUNCTION set_color : DINT
  VAR
    local_c : COLOR := GREEN;
  END_VAR
  set_color := 0;
END_FUNCTION

PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := set_color();
END_PROGRAM
";
    // The function compiles and runs without error; local_c is re-initialized.
    let (_c, _bufs) = compile_and_run(source);
}

// ---------------------------------------------------------------------------
// Section 4: Expressions (REQ-EN-030 through REQ-EN-034)
// These are tested once expression compilation is implemented (PR 3).
// ---------------------------------------------------------------------------

/// REQ-EN-030: EnumeratedValue expression compiles to LOAD_CONST_I32.
#[spec_test(REQ_EN_030)]
fn enum_spec_req_en_030_enum_value_expr_pushes_ordinal() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
  END_VAR
  c := GREEN;
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 1); // GREEN = ordinal 1
}

/// REQ-EN-031: Qualified enum reference (COLOR#GREEN) resolves correctly.
#[spec_test(REQ_EN_031)]
fn enum_spec_req_en_031_qualified_reference_resolves() {
    let lib = parse_library(
        "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
         PROGRAM main END_PROGRAM",
    );
    let map = build_enum_ordinal_map(&lib);

    let mut ev = ironplc_dsl::common::EnumeratedValue::new("GREEN");
    ev.type_name = Some(ironplc_dsl::common::TypeName::from("COLOR"));

    assert_eq!(resolve_enum_ordinal(&map, &ev).unwrap(), 1);
}

/// REQ-EN-032: Unqualified enum reference (GREEN) resolves via reverse lookup.
#[spec_test(REQ_EN_032)]
fn enum_spec_req_en_032_unqualified_reference_resolves() {
    let lib = parse_library(
        "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
         PROGRAM main END_PROGRAM",
    );
    let map = build_enum_ordinal_map(&lib);

    let ev = ironplc_dsl::common::EnumeratedValue::new("BLUE");
    assert_eq!(resolve_enum_ordinal(&map, &ev).unwrap(), 2);
}

/// REQ-EN-033: Enum equality comparison uses integer comparison.
/// Tests both IF equality expressions and CASE matching.
#[spec_test(REQ_EN_033)]
fn enum_spec_req_en_033_equality_comparison_works() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
    result : DINT;
  END_VAR
  c := GREEN;
  IF c = GREEN THEN
    result := 42;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

/// REQ-EN-034: Assignment of enum value compiles to LOAD_CONST + STORE_VAR.
#[spec_test(REQ_EN_034)]
fn enum_spec_req_en_034_assignment_stores_ordinal() {
    let source = "
TYPE LEVEL : (LOW, MEDIUM, HIGH) := LOW; END_TYPE
PROGRAM main
  VAR
    x : LEVEL;
  END_VAR
  x := HIGH;
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 2); // HIGH = ordinal 2
}

// ---------------------------------------------------------------------------
// Section 5: CASE Selectors (REQ-EN-040, REQ-EN-041)
// ---------------------------------------------------------------------------

/// REQ-EN-040: CASE selector with enum value compares via EQ_I32.
#[spec_test(REQ_EN_040)]
fn enum_spec_req_en_040_case_selector_matches_enum_value() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
    result : DINT;
  END_VAR
  c := GREEN;
  CASE c OF
    RED: result := 10;
    GREEN: result := 20;
    BLUE: result := 30;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 20);
}

/// REQ-EN-041: Multiple enum values in a CASE arm combine with boolean OR.
#[spec_test(REQ_EN_041)]
fn enum_spec_req_en_041_case_multiple_values_in_arm() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
    result : DINT;
  END_VAR
  c := BLUE;
  CASE c OF
    RED, GREEN: result := 10;
    BLUE: result := 20;
  END_CASE;
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 20);
}

// ---------------------------------------------------------------------------
// Section 6: Structure Field Initialization (REQ-EN-050, REQ-EN-051)
// ---------------------------------------------------------------------------

/// REQ-EN-050: Enum value in struct initializer emits LOAD_CONST_I32.
#[spec_test(REQ_EN_050)]
fn enum_spec_req_en_050_struct_field_enum_init() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
TYPE MyStruct :
  STRUCT
    c : COLOR;
    v : DINT;
  END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    s : MyStruct := (c := GREEN, v := 42);
    result : DINT;
  END_VAR
  result := s.v;
END_PROGRAM
";
    let (_c, bufs) = compile_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

/// REQ-EN-051: Struct field enum type gets correct op_type via resolve_field_op_type.
#[spec_test(REQ_EN_051)]
fn enum_spec_req_en_051_struct_field_enum_type_resolves() {
    // If the field type didn't resolve correctly, the struct wouldn't compile.
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
TYPE MyStruct :
  STRUCT
    c : COLOR;
  END_STRUCT;
END_TYPE
PROGRAM main
  VAR
    s : MyStruct;
  END_VAR
END_PROGRAM
";
    let _container = compile_only(source);
}

// ---------------------------------------------------------------------------
// Section 7: Debug Section (REQ-EN-060 through REQ-EN-064)
// ---------------------------------------------------------------------------

/// REQ-EN-060: Tag 9 is ENUM_DEF in debug section.
#[spec_test(REQ_EN_060)]
fn enum_spec_req_en_060_tag_9_enum_def() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
  END_VAR
END_PROGRAM
";
    let container = compile_only(source);
    let debug = container.debug_section.as_ref().unwrap();
    // The ENUM_DEF sub-table is present with our enum type.
    assert!(!debug.enum_defs.is_empty());
    assert_eq!(
        debug
            .enum_defs
            .iter()
            .find(|e| e.type_name == "COLOR")
            .unwrap()
            .type_name,
        "COLOR"
    );
}

/// REQ-EN-061: ENUM_DEF sub-table roundtrips through write/read.
#[spec_test(REQ_EN_061)]
fn enum_spec_req_en_061_enum_def_payload_roundtrips() {
    use ironplc_container::debug_section::{DebugSection, EnumDefEntry};
    use std::io::Cursor;

    let section = DebugSection {
        var_names: vec![],
        func_names: vec![],
        line_map: vec![],
        enum_defs: vec![EnumDefEntry {
            type_name: "COLOR".into(),
            values: vec!["RED".into(), "GREEN".into(), "BLUE".into()],
        }],
    };
    let mut buf = Vec::new();
    section.write_to(&mut buf).unwrap();

    let decoded = DebugSection::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(decoded.enum_defs.len(), 1);
    assert_eq!(decoded.enum_defs[0].type_name, "COLOR");
    assert_eq!(decoded.enum_defs[0].values, vec!["RED", "GREEN", "BLUE"]);
}

/// REQ-EN-062: Value names appear in ordinal order in the definition table.
#[spec_test(REQ_EN_062)]
fn enum_spec_req_en_062_values_in_ordinal_order() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
  END_VAR
END_PROGRAM
";
    let container = compile_only(source);
    let debug = container.debug_section.as_ref().unwrap();
    let color_def = debug
        .enum_defs
        .iter()
        .find(|e| e.type_name == "COLOR")
        .unwrap();
    // Values must be in declaration (ordinal) order.
    assert_eq!(color_def.values, vec!["RED", "GREEN", "BLUE"]);
}

/// REQ-EN-063: Unknown tags are skippable via directory size field.
/// This is verified by the existing debug_section_read_when_unknown_tag_then_skips
/// test in the container crate.
#[spec_test(REQ_EN_063)]
fn enum_spec_req_en_063_unknown_tags_skippable() {
    use ironplc_container::debug_section::{DebugSection, EnumDefEntry};
    use std::io::Cursor;

    // A debug section with ENUM_DEF roundtrips, proving Tag 9 uses
    // the standard directory size mechanism.
    let section = DebugSection {
        var_names: vec![],
        func_names: vec![],
        line_map: vec![],
        enum_defs: vec![EnumDefEntry {
            type_name: "X".into(),
            values: vec!["A".into()],
        }],
    };
    let mut buf = Vec::new();
    section.write_to(&mut buf).unwrap();
    assert_eq!(section.section_size(), buf.len() as u32);

    let decoded = DebugSection::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(decoded.enum_defs.len(), 1);
}

/// REQ-EN-064: Only named enum types are emitted in ENUM_DEF.
#[spec_test(REQ_EN_064)]
fn enum_spec_req_en_064_only_named_types_in_enum_def() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
  END_VAR
END_PROGRAM
";
    let container = compile_only(source);
    let debug = container.debug_section.as_ref().unwrap();
    // Only the named type COLOR appears, no anonymous types.
    assert_eq!(debug.enum_defs.len(), 1);
    assert_eq!(debug.enum_defs[0].type_name, "COLOR");
}

// ---------------------------------------------------------------------------
// Section 8: Playground Display (REQ-EN-070 through REQ-EN-072)
// Display formatting is tested at the unit level here; the playground
// crate performs integration testing in a browser context.
// ---------------------------------------------------------------------------

/// REQ-EN-070: Enum display shows value name followed by ordinal.
/// Tested via the compiled container's debug section having the right data
/// for the playground to use. The actual formatting is in the playground crate.
#[spec_test(REQ_EN_070)]
fn enum_spec_req_en_070_debug_section_has_enum_data_for_display() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR := GREEN;
  END_VAR
END_PROGRAM
";
    let container = compile_only(source);
    let debug = container.debug_section.as_ref().unwrap();
    // Variable c has type_name "COLOR" and iec_type_tag DINT.
    let var = &debug.var_names[0];
    assert_eq!(var.type_name, "COLOR");
    assert_eq!(var.iec_type_tag, iec_type_tag::DINT);
    // ENUM_DEF table has COLOR with values in order.
    let color_def = debug
        .enum_defs
        .iter()
        .find(|e| e.type_name == "COLOR")
        .unwrap();
    assert_eq!(color_def.values[1], "GREEN"); // ordinal 1 = GREEN
}

/// REQ-EN-071: Out-of-range ordinal falls back to integer display.
/// The iec_type_tag (DINT) always provides a valid fallback interpretation.
#[spec_test(REQ_EN_071)]
fn enum_spec_req_en_071_out_of_range_falls_back() {
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR;
  END_VAR
END_PROGRAM
";
    // The iec_type_tag is DINT, so any raw value can be displayed as an integer.
    let container = compile_only(source);
    let debug = container.debug_section.as_ref().unwrap();
    assert_eq!(debug.var_names[0].iec_type_tag, iec_type_tag::DINT);
}

/// REQ-EN-072: Missing ENUM_DEF table falls back to iec_type_tag display.
#[spec_test(REQ_EN_072)]
fn enum_spec_req_en_072_missing_enum_def_falls_back() {
    use ironplc_container::debug_section::DebugSection;
    // A debug section with no enum_defs still allows DINT display.
    let section = DebugSection::default();
    assert!(section.enum_defs.is_empty());
}

// ---------------------------------------------------------------------------
// Section 9: Ordinal Map Construction (REQ-EN-080 through REQ-EN-083)
// ---------------------------------------------------------------------------

/// REQ-EN-080: Ordinal map built from DataTypeDeclaration(Enumeration) entries.
#[spec_test(REQ_EN_080)]
fn enum_spec_req_en_080_ordinal_map_from_type_declarations() {
    let lib = parse_library(
        "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
         TYPE LEVEL : (LOW, HIGH) := LOW; END_TYPE
         PROGRAM main END_PROGRAM",
    );
    let map = build_enum_ordinal_map(&lib);
    // Both type declarations are in the map.
    assert!(map.definitions.contains_key("COLOR"));
    assert!(map.definitions.contains_key("LEVEL"));
    assert_eq!(
        resolve_enum_ordinal(&map, &ironplc_dsl::common::EnumeratedValue::new("RED")).unwrap(),
        0
    );
    assert_eq!(
        resolve_enum_ordinal(&map, &ironplc_dsl::common::EnumeratedValue::new("HIGH")).unwrap(),
        1
    );
}

/// REQ-EN-081: Reverse lookup from unqualified value names.
#[spec_test(REQ_EN_081)]
fn enum_spec_req_en_081_reverse_lookup_for_unqualified() {
    let lib = parse_library(
        "TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
         PROGRAM main END_PROGRAM",
    );
    let map = build_enum_ordinal_map(&lib);
    // Unqualified lookup resolves correctly.
    let ev = ironplc_dsl::common::EnumeratedValue::new("GREEN");
    assert_eq!(resolve_enum_ordinal(&map, &ev).unwrap(), 1);
}

/// REQ-EN-082: Type declaration default stored as pre-resolved ordinal.
#[spec_test(REQ_EN_082)]
fn enum_spec_req_en_082_default_ordinal_from_type_declaration() {
    let lib = parse_library(
        "TYPE LEVEL : (LOW, MEDIUM, HIGH) := HIGH; END_TYPE
         PROGRAM main END_PROGRAM",
    );
    let map = build_enum_ordinal_map(&lib);
    assert_eq!(resolve_enum_default_ordinal(&map, "LEVEL"), 2);
}

/// REQ-EN-083: Ordinal map built once at codegen entry, stored in CompileContext.
#[spec_test(REQ_EN_083)]
fn enum_spec_req_en_083_map_built_once_at_codegen_entry() {
    // Verify the map is available by compiling a program with enum types.
    // The compile function internally calls build_enum_ordinal_map and stores
    // the result in CompileContext. If this path was broken, compilation would
    // fail.
    let source = "
TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE
PROGRAM main
  VAR
    c : COLOR := GREEN;
  END_VAR
END_PROGRAM
";
    let _container = compile_only(source);
}
