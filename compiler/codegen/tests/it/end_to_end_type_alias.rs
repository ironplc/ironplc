//! End-to-end tests for type alias resolution through codegen.
//!
//! These tests validate that the analyzer's `resolve_types` pass correctly
//! resolves type aliases to their elementary types, enabling codegen to select
//! the correct opcodes.

// BYTE is an unsigned 8-bit type; 42 fits within u8 range
e2e_i32!(
    end_to_end_when_type_alias_byte_assignment_then_correct,
    "TYPE MyByte : BYTE := 0; END_TYPE PROGRAM main VAR x : MyByte; END_VAR x := 42; END_PROGRAM",
    &[(0, 42)],
);

// 300 truncated to u8 = 300 - 256 = 44
e2e_i32!(
    end_to_end_when_type_alias_byte_truncation_then_correct,
    "TYPE MyByte : BYTE := 0; END_TYPE PROGRAM main VAR x : MyByte; END_VAR x := 300; END_PROGRAM",
    &[(0, 44)],
);

e2e_i32!(
    end_to_end_when_type_alias_int_arithmetic_then_correct,
    "TYPE MyInt : INT := 0; END_TYPE PROGRAM main VAR x : MyInt; y : MyInt; END_VAR x := 100; y := x + 200; END_PROGRAM",
    &[(0, 100), (1, 300)],
);

// INT is signed 16-bit; 40000 truncated to i16 = 40000 - 65536 = -25536
e2e_i32!(
    end_to_end_when_type_alias_int_overflow_then_truncated,
    "TYPE MyInt : INT := 0; END_TYPE PROGRAM main VAR x : MyInt; END_VAR x := 40000; END_PROGRAM",
    &[(0, -25536)],
);
