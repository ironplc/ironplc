//! End-to-end tests for type alias resolution through codegen.
//!
//! These tests validate that the analyzer's `resolve_types` pass correctly
//! resolves type aliases to their elementary types, enabling codegen to select
//! the correct opcodes.

use crate::common::bit_string_options;

// BYTE is an unsigned 8-bit type; 42 fits within u8 range
e2e_i32_with!(
    end_to_end_when_type_alias_byte_assignment_then_correct,
    bit_string_options(),
    "
TYPE MyByte : BYTE := 0; END_TYPE
PROGRAM main
  VAR
    x : MyByte;
  END_VAR
  x := 42;
END_PROGRAM
",
    &[("x", 42)],
);

// 150 + 150 = 300, truncated to u8 = 300 - 256 = 44
e2e_i32_with!(
    end_to_end_when_type_alias_byte_truncation_then_correct,
    bit_string_options(),
    "
TYPE MyByte : BYTE := 0; END_TYPE
PROGRAM main
  VAR
    x : MyByte;
    a : MyByte;
  END_VAR
  a := 150;
  x := a + a;
END_PROGRAM
",
    &[("a", 150), ("x", 44)],
);

e2e_i32!(
    end_to_end_when_type_alias_int_arithmetic_then_correct,
    "
TYPE MyInt : INT := 0; END_TYPE
PROGRAM main
  VAR
    x : MyInt;
    y : MyInt;
  END_VAR
  x := 100;
  y := x + 200;
END_PROGRAM
",
    &[("x", 100), ("y", 300)],
);

// An alias with no initializer takes the base type's default, 0 (#1416).
e2e_i32!(
    end_to_end_when_type_alias_without_initializer_then_uses_base_type,
    "
TYPE MyInt : INT; END_TYPE
PROGRAM main
  VAR
    x : MyInt;
    y : MyInt;
  END_VAR
  y := x - 3;
END_PROGRAM
",
    &[("x", 0), ("y", -3)],
);
