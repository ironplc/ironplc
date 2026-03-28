//! Bytecode-level integration tests for structure variable allocation.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_compile;

#[test]
fn compile_when_struct_var_with_init_then_allocates_data_region() {
    let source = "
TYPE MyStruct :
  STRUCT
    a : INT;
    b : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MyStruct := (a := 1, b := 2);
  END_VAR
END_PROGRAM
";
    // Should compile without error — the structure variable gets data region space
    let container = parse_and_compile(source, &CompilerOptions::default());
    let type_section = container.type_section.as_ref().unwrap();
    // The type section should have at least one array descriptor (for the struct)
    assert!(!type_section.array_descriptors.is_empty());
}

#[test]
fn compile_when_struct_var_without_init_then_allocates_data_region() {
    let source = "
TYPE MyStruct :
  STRUCT
    a : INT;
    b : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MyStruct;
  END_VAR
END_PROGRAM
";
    // LateResolvedType path — should also compile and allocate data region
    let container = parse_and_compile(source, &CompilerOptions::default());
    let type_section = container.type_section.as_ref().unwrap();
    assert!(!type_section.array_descriptors.is_empty());
}

#[test]
fn compile_when_struct_var_then_registers_descriptor_with_slot_type() {
    let source = "
TYPE MyStruct :
  STRUCT
    a : INT;
    b : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s : MyStruct;
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let type_section = container.type_section.as_ref().unwrap();
    // Find descriptor with element_type = 10 (FieldType::Slot)
    let slot_desc = type_section
        .array_descriptors
        .iter()
        .find(|d| d.element_type == 10);
    assert!(
        slot_desc.is_some(),
        "Expected descriptor with element_type=10 (Slot)"
    );
    // MyStruct has 2 fields (a: INT, b: DINT), each 1 slot → total_elements = 2
    assert_eq!(slot_desc.unwrap().total_elements, 2);
}

#[test]
fn compile_when_two_struct_vars_then_sequential_data_offsets() {
    let source = "
TYPE MyStruct :
  STRUCT
    a : INT;
    b : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    s1 : MyStruct;
    s2 : MyStruct;
  END_VAR
END_PROGRAM
";
    // Both should compile. The data region should have space for both
    // (2 slots * 8 bytes * 2 vars = 32 bytes)
    let container = parse_and_compile(source, &CompilerOptions::default());
    assert!(container.header.data_region_bytes >= 32);
}

#[test]
fn compile_when_nested_struct_var_then_allocates_sum_of_slots() {
    let source = "
TYPE Inner :
  STRUCT
    x : INT;
    y : INT;
  END_STRUCT;
END_TYPE

TYPE Outer :
  STRUCT
    inner : Inner;
    z : DINT;
  END_STRUCT;
END_TYPE

PROGRAM main
  VAR
    o : Outer;
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let type_section = container.type_section.as_ref().unwrap();
    // Outer has inner (2 slots) + z (1 slot) = 3 total slots
    let slot_desc = type_section
        .array_descriptors
        .iter()
        .find(|d| d.element_type == 10 && d.total_elements == 3);
    assert!(
        slot_desc.is_some(),
        "Expected descriptor with 3 total slots for nested struct"
    );
}
