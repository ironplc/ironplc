//! Compiling the same source twice produces the same container bytes.
//!
//! Each compilation builds its hash maps with fresh random keys, so output
//! that follows hash iteration order differs between compilations of one
//! source within a few runs.

use crate::common::parse_and_compile;
use ironplc_parser::options::CompilerOptions;

/// Several enumerations and function block types, the declarations whose
/// descriptors and debug entries the container lists.
const PROGRAM: &str = "
TYPE
  COLOR : (RED, GREEN, BLUE);
  LEVEL : (LOW, HIGH);
  MODE : (IDLE, RUN, STOP, FAULT);
END_TYPE

FUNCTION_BLOCK FB_A
VAR_INPUT x : DINT; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B
VAR_INPUT y : DINT; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_C
VAR_INPUT z : DINT; END_VAR
END_FUNCTION_BLOCK

PROGRAM main
VAR
  c : COLOR := GREEN;
  l : LEVEL := HIGH;
  m : MODE := RUN;
  a : FB_A;
  b : FB_B;
  d : FB_C;
END_VAR
  a(x := 1);
  b(y := 2);
  d(z := 3);
END_PROGRAM
";

fn container_bytes() -> Vec<u8> {
    let container = parse_and_compile(PROGRAM, &CompilerOptions::default());
    let mut bytes = Vec::new();
    container.write_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn compile_when_same_source_compiled_repeatedly_then_same_bytes() {
    let first = container_bytes();

    for _ in 0..16 {
        assert!(container_bytes() == first, "container bytes differ");
    }
}

#[test]
fn compile_when_several_enumerations_then_debug_enum_defs_sorted_by_name() {
    let container = parse_and_compile(PROGRAM, &CompilerOptions::default());

    let names: Vec<&str> = container
        .debug_section
        .as_ref()
        .unwrap()
        .enum_defs
        .iter()
        .map(|def| def.type_name.as_str())
        .collect();
    assert_eq!(names, ["COLOR", "LEVEL", "MODE"]);
}

#[test]
fn compile_when_several_function_blocks_then_user_fb_types_sorted_by_type_id() {
    let container = parse_and_compile(PROGRAM, &CompilerOptions::default());

    let ids: Vec<_> = container
        .type_section
        .as_ref()
        .unwrap()
        .user_fb_types
        .iter()
        .map(|desc| desc.type_id.raw())
        .collect();
    assert_eq!(ids.len(), 3);
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]), "{ids:?}");
}
