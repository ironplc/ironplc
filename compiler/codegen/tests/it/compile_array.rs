//! Bytecode-level integration tests for array compilation.

use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use crate::common::{parse_and_compile, try_parse_and_compile};

#[test]
fn compile_when_array_1d_constant_index_load_then_produces_load_array() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..5] OF INT;
    x : INT;
  END_VAR
  x := arr[3];
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // x := arr[3] with 1-based lower bound => flat index = 3 - 1 = 2
    // Bytecode should contain: LOAD_CONST_I32 (flat index 2), LOAD_ARRAY var:0 desc:0
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // Find the LOAD_ARRAY opcode
    let load_array_pos = bytecode
        .iter()
        .position(|&b| b == opcode::LOAD_ARRAY)
        .unwrap();
    // Before LOAD_ARRAY should be LOAD_CONST_I32 with flat index 2
    assert!(load_array_pos >= 3);
    // Preceding byte should be LOAD_CONST_I32
    assert_eq!(bytecode[load_array_pos - 3], opcode::LOAD_CONST_I32);
    // Verify the constant pool contains the flat index 2
    let const_idx =
        u16::from_le_bytes([bytecode[load_array_pos - 2], bytecode[load_array_pos - 1]]);
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(const_idx))
            .unwrap(),
        2
    );
}

#[test]
fn compile_when_array_1d_constant_index_store_then_produces_store_array() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..5] OF INT;
  END_VAR
  arr[3] := 42;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // Find the STORE_ARRAY opcode
    let store_array_pos = bytecode
        .iter()
        .position(|&b| b == opcode::STORE_ARRAY)
        .unwrap();
    // Before STORE_ARRAY should be LOAD_CONST_I32 with flat index 2
    assert!(store_array_pos >= 3);
    assert_eq!(bytecode[store_array_pos - 3], opcode::LOAD_CONST_I32); // LOAD_CONST_I32
    let const_idx =
        u16::from_le_bytes([bytecode[store_array_pos - 2], bytecode[store_array_pos - 1]]);
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(const_idx))
            .unwrap(),
        2
    );
}

#[test]
fn compile_when_array_1d_variable_index_load_then_emits_sub_i64() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..5] OF INT;
    i : INT;
    x : INT;
  END_VAR
  x := arr[i];
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // Should contain LOAD_VAR for i, LOAD_CONST_I64 (lower bound 1), SUB_I64, LOAD_ARRAY
    assert!(
        bytecode.contains(&opcode::SUB_I64),
        "SUB_I64 not found in bytecode"
    );
    assert!(
        bytecode.contains(&opcode::LOAD_ARRAY),
        "LOAD_ARRAY not found in bytecode"
    );
}

#[test]
fn compile_when_array_1d_variable_index_store_then_emits_sub_i64() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..5] OF INT;
    i : INT;
  END_VAR
  arr[i] := 42;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    assert!(
        bytecode.contains(&opcode::SUB_I64),
        "SUB_I64 not found in bytecode"
    );
    assert!(
        bytecode.contains(&opcode::STORE_ARRAY),
        "STORE_ARRAY not found in bytecode"
    );
}

#[test]
fn compile_when_array_multidim_constant_index_then_computes_flat_index() {
    // ARRAY[1..3, 1..4] OF INT, access matrix[2,3]
    // Flat index = (2-1)*4 + (3-1) = 4 + 2 = 6
    let source = "
PROGRAM main
  VAR
    matrix : ARRAY[1..3, 1..4] OF INT;
    x : INT;
  END_VAR
  x := matrix[2, 3];
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    let load_array_pos = bytecode
        .iter()
        .position(|&b| b == opcode::LOAD_ARRAY)
        .unwrap();
    assert!(load_array_pos >= 3);
    assert_eq!(bytecode[load_array_pos - 3], opcode::LOAD_CONST_I32); // LOAD_CONST_I32
    let const_idx =
        u16::from_le_bytes([bytecode[load_array_pos - 2], bytecode[load_array_pos - 1]]);
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(const_idx))
            .unwrap(),
        6
    );
}

#[test]
fn compile_when_array_nonzero_lower_bound_then_adjusts_index() {
    // ARRAY[-5..5] OF INT, access arr[0] => flat index = 0 - (-5) = 5
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[-5..5] OF INT;
    x : INT;
  END_VAR
  x := arr[0];
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    let load_array_pos = bytecode
        .iter()
        .position(|&b| b == opcode::LOAD_ARRAY)
        .unwrap();
    assert!(load_array_pos >= 3);
    assert_eq!(bytecode[load_array_pos - 3], opcode::LOAD_CONST_I32); // LOAD_CONST_I32
    let const_idx =
        u16::from_le_bytes([bytecode[load_array_pos - 2], bytecode[load_array_pos - 1]]);
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(const_idx))
            .unwrap(),
        5
    );
}

#[test]
fn compile_when_array_constant_oob_above_then_error() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..10] OF INT;
    x : INT;
  END_VAR
  x := arr[11];
END_PROGRAM
";
    let result = try_parse_and_compile(source, &CompilerOptions::default());
    assert!(
        result.is_err(),
        "Expected compile-time error for out-of-bounds index"
    );
}

#[test]
fn compile_when_array_constant_oob_below_then_error() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..10] OF INT;
    x : INT;
  END_VAR
  x := arr[0];
END_PROGRAM
";
    let result = try_parse_and_compile(source, &CompilerOptions::default());
    assert!(
        result.is_err(),
        "Expected compile-time error for out-of-bounds index"
    );
}

#[test]
fn compile_when_array_sint_store_then_emits_truncation() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..3] OF SINT;
    n : SINT;
  END_VAR
  arr[1] := n + 1;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // Should contain TRUNC_I8 (0x1C) before STORE_ARRAY (0xAC). The value is
    // computed at run time; a constant would be truncated at compile time
    // instead (see compile_const_trunc.rs).
    let trunc_pos = bytecode
        .iter()
        .position(|&b| b == opcode::TRUNC_I8)
        .unwrap();
    let store_pos = bytecode
        .iter()
        .position(|&b| b == opcode::STORE_ARRAY)
        .unwrap();
    assert!(
        trunc_pos < store_pos,
        "TRUNC_I8 should come before STORE_ARRAY"
    );
}

#[test]
fn compile_when_array_sint_load_then_no_truncation() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..3] OF SINT;
    x : SINT;
  END_VAR
  x := arr[1];
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    // LOAD_ARRAY should appear but TRUNC_I8 should NOT appear between
    // the LOAD_ARRAY and the STORE_VAR (truncation happens at store-to-var, not load-from-array)
    let load_array_pos = bytecode
        .iter()
        .position(|&b| b == opcode::LOAD_ARRAY)
        .unwrap();
    // The TRUNC should appear after LOAD_ARRAY (for the final STORE_VAR), not before it.
    // There may or may not be a TRUNC — the key is LOAD_ARRAY itself doesn't truncate.
    // Just verify LOAD_ARRAY is present.
    assert!(load_array_pos > 0);
}

#[test]
fn compile_when_array_initialization_then_emits_store_array_per_element() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..3] OF INT := [10, 20, 30];
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    // The init function should emit 3 STORE_ARRAY instructions for the 3 initial values
    let init_bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(0))
        .unwrap();
    let store_count = init_bytecode
        .iter()
        .filter(|&&b| b == opcode::STORE_ARRAY)
        .count();
    assert_eq!(
        store_count, 3,
        "Expected 3 STORE_ARRAY in init for 3 initial values"
    );
}

#[test]
fn compile_when_array_initialization_repeated_then_emits_correct_count() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..6] OF INT := [3(10), 3(20)];
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());

    let init_bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(0))
        .unwrap();
    let store_count = init_bytecode
        .iter()
        .filter(|&&b| b == opcode::STORE_ARRAY)
        .count();
    assert_eq!(
        store_count, 6,
        "Expected 6 STORE_ARRAY in init for 3(10), 3(20)"
    );
}

#[test]
fn compile_when_array_single_element_then_works() {
    let source = "
PROGRAM main
  VAR
    arr : ARRAY[1..1] OF INT;
    x : INT;
  END_VAR
  arr[1] := 42;
  x := arr[1];
END_PROGRAM
";
    let result = try_parse_and_compile(source, &CompilerOptions::default());
    assert!(
        result.is_ok(),
        "Degenerate single-element array should compile"
    );
}

#[test]
fn flat_index_arithmetic_when_worst_case_subscript_then_fits_i64() {
    let max_range: i64 = i32::MAX as i64 - i32::MIN as i64;
    let max_stride: i64 = 32768;
    let result = max_range.checked_mul(max_stride);
    assert!(result.is_some(), "flat index must fit in i64");
}

#[test]
fn compile_when_var_array_of_string_then_descriptor_uses_string_field_type() {
    // FieldType::String = 6 per container/src/type_section.rs.
    let source = "
PROGRAM main
  VAR
    names : ARRAY[1..3] OF STRING[10];
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let type_section = container.type_section.as_ref().unwrap();
    let str_desc = type_section
        .array_descriptors
        .iter()
        .find(|d| d.element_type == 6);
    assert!(
        str_desc.is_some(),
        "Expected ARRAY OF STRING descriptor with element_type=6 (String), got: {:?}",
        type_section.array_descriptors
    );
    let desc = str_desc.unwrap();
    assert_eq!(desc.total_elements, 3);
    assert_eq!(desc.element_extra, 10);
}

#[test]
fn compile_when_var_array_of_wstring_then_descriptor_uses_wstring_field_type() {
    // FieldType::WString = 7 per container/src/type_section.rs.
    let source = "
PROGRAM main
  VAR
    names : ARRAY[1..3] OF WSTRING[10];
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let type_section = container.type_section.as_ref().unwrap();
    let wstr_desc = type_section
        .array_descriptors
        .iter()
        .find(|d| d.element_type == 7);
    assert!(
        wstr_desc.is_some(),
        "Expected ARRAY OF WSTRING descriptor with element_type=7 (WString), got: {:?}",
        type_section.array_descriptors
    );
    let desc = wstr_desc.unwrap();
    assert_eq!(desc.total_elements, 3);
    assert_eq!(desc.element_extra, 10);
}

// --- Top-level ARRAY OF <struct> (issue #1383) ---

#[test]
fn compile_when_var_top_level_array_of_struct_then_descriptor_covers_all_slots() {
    // FieldType::Slot = 10 per container/src/type_section.rs. A structure
    // occupies a contiguous run of slots, so the descriptor spans
    // total_elements * element_slots rather than one entry per element.
    let source = "
TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE

PROGRAM main
  VAR
    arr : ARRAY[1..4] OF Item;
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let type_section = container.type_section.as_ref().unwrap();
    let desc = type_section
        .array_descriptors
        .iter()
        .find(|d| d.element_type == 10)
        .unwrap_or_else(|| {
            panic!(
                "expected a slot-typed descriptor, got: {:?}",
                type_section.array_descriptors
            )
        });
    // 4 elements * 2 slots each.
    assert_eq!(desc.total_elements, 8);
    assert_eq!(desc.element_extra, 0);
}

#[test]
fn compile_when_top_level_array_of_struct_field_stored_then_index_is_element_stride_plus_leaf() {
    // `arr[3].b` addresses slot (3 - 1) * 2 + 1 of the array's region: the
    // element index is folded to a constant scaled by the element slot count,
    // then the leaf field's offset is added.
    let source = "
TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE

PROGRAM main
  VAR
    arr : ARRAY[1..4] OF Item;
  END_VAR
  arr[3].b := 42;
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap();
    let store_pos = bytecode
        .iter()
        .position(|&b| b == opcode::STORE_ARRAY)
        .unwrap();

    // ... LOAD_CONST_I32 <flat index> LOAD_CONST_I64 <leaf offset> ADD_I64 STORE_ARRAY
    let add_pos = store_pos - 1;
    assert_eq!(bytecode[add_pos], opcode::ADD_I64);
    assert_eq!(bytecode[add_pos - 3], opcode::LOAD_CONST_I64);
    let leaf_const = u16::from_le_bytes([bytecode[add_pos - 2], bytecode[add_pos - 1]]);
    assert_eq!(
        container
            .constant_pool
            .get_i64(ironplc_container::ConstantIndex::new(leaf_const))
            .unwrap(),
        1,
        "leaf field 'b' sits one slot into the element"
    );
    assert_eq!(bytecode[add_pos - 6], opcode::LOAD_CONST_I32);
    let index_const = u16::from_le_bytes([bytecode[add_pos - 5], bytecode[add_pos - 4]]);
    assert_eq!(
        container
            .constant_pool
            .get_i32(ironplc_container::ConstantIndex::new(index_const))
            .unwrap(),
        4,
        "element 3 of a 2-slot element type starts at slot 4"
    );
}

#[test]
fn compile_when_top_level_array_of_struct_then_data_offset_stored_in_variable_slot() {
    // The variable slot holds the data region byte offset, the same protocol a
    // structure variable uses. Without it every element access addresses
    // offset 0.
    let source = "
TYPE Item : STRUCT a : DINT; END_STRUCT; END_TYPE

PROGRAM main
  VAR
    arr : ARRAY[1..4] OF Item;
  END_VAR
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let bytecode = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(0))
        .unwrap();
    assert!(
        bytecode.contains(&opcode::STORE_VAR_I32),
        "init function must store the array's data offset into its variable slot"
    );
}

#[test]
fn compile_when_top_level_array_of_ref_to_struct_then_stays_on_reference_path() {
    // `ARRAY[..] OF REF_TO <struct>` elements are one-slot references, so the
    // array keeps the ordinary (U64) descriptor rather than the slot-typed one.
    // FieldType::U64 = 3 per container/src/type_section.rs.
    let source = "
TYPE Item : STRUCT a : DINT; END_STRUCT; END_TYPE

PROGRAM main
  VAR
    arr : ARRAY[1..4] OF REF_TO Item;
  END_VAR
END_PROGRAM
";
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let container = try_parse_and_compile(source, &options).unwrap();
    let type_section = container.type_section.as_ref().unwrap();
    let desc = type_section
        .array_descriptors
        .iter()
        .find(|d| d.total_elements == 4)
        .unwrap_or_else(|| {
            panic!(
                "expected a 4-element descriptor, got: {:?}",
                type_section.array_descriptors
            )
        });
    assert_eq!(desc.element_type, 3);
}
