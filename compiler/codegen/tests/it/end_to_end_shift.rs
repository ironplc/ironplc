//! End-to-end integration tests for bit shift/rotate functions (SHL, SHR, ROL, ROR).

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;

// --- SHL ---

e2e_i32!(
    end_to_end_when_shl_byte_then_shifts_left,
    "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#0F;
  y := SHL(x, 4);
END_PROGRAM
",
    &[("x", 0x0F), ("y", 0xF0)],
);

e2e_i32!(
    end_to_end_when_shl_dword_then_shifts_left,
    "
PROGRAM main
  VAR
    x : DWORD;
    y : DWORD;
  END_VAR
  x := DWORD#16#0000_000F;
  y := SHL(x, 16);
END_PROGRAM
",
    &[("x", 0x0F), ("y", 0x000F_0000_u32 as i32)],
);

e2e_i64!(
    end_to_end_when_shl_lword_then_shifts_left_64bit,
    "
PROGRAM main
  VAR
    x : LWORD;
    y : LWORD;
  END_VAR
  x := LWORD#16#01;
  y := SHL(x, 32);
END_PROGRAM
",
    &[("x", 0x01), ("y", 0x1_0000_0000)],
);

// --- SHR ---

e2e_i32!(
    end_to_end_when_shr_word_then_shifts_right,
    "
PROGRAM main
  VAR
    x : WORD;
    y : WORD;
  END_VAR
  x := WORD#16#FF00;
  y := SHR(x, 8);
END_PROGRAM
",
    &[("x", 0xFF00), ("y", 0x00FF)],
);

e2e_i32!(
    end_to_end_when_shr_byte_then_shifts_right,
    "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#F0;
  y := SHR(x, 4);
END_PROGRAM
",
    &[("x", 0xF0), ("y", 0x0F)],
);

// --- ROL ---

// ROL(BYTE#16#81, 1) should give 0x03 (bit 7 wraps to bit 0 within 8 bits)
e2e_i32!(
    end_to_end_when_rol_byte_then_rotates_within_8_bits,
    "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#81;
  y := ROL(x, 1);
END_PROGRAM
",
    &[("x", 0x81), ("y", 0x03)],
);

// ROL(WORD#16#8001, 1) = 0x0003
e2e_i32!(
    end_to_end_when_rol_word_then_rotates_within_16_bits,
    "
PROGRAM main
  VAR
    x : WORD;
    y : WORD;
  END_VAR
  x := WORD#16#8001;
  y := ROL(x, 1);
END_PROGRAM
",
    &[("x", 0x8001), ("y", 0x0003)],
);

// ROL(DWORD#16#80000001, 1) = 0x00000003
e2e!(
    end_to_end_when_rol_dword_then_rotates_left,
    "
PROGRAM main
  VAR
    x : DWORD;
    y : DWORD;
  END_VAR
  x := DWORD#16#80000001;
  y := ROL(x, 1);
END_PROGRAM
",
    &[("x", 0x8000_0001_u32), ("y", 0x0000_0003)],
);

// --- ROR ---

// ROR(DWORD#16#00000001, 1) = 0x80000000 (bit 0 wraps to bit 31)
e2e!(
    end_to_end_when_ror_dword_then_rotates_right,
    "
PROGRAM main
  VAR
    x : DWORD;
    y : DWORD;
  END_VAR
  x := DWORD#16#00000001;
  y := ROR(x, 1);
END_PROGRAM
",
    &[("x", 0x0000_0001_u32), ("y", 0x8000_0000)],
);

// ROR(BYTE#16#01, 1) = 0x80 (bit 0 wraps to bit 7 within 8 bits)
e2e_i32!(
    end_to_end_when_ror_byte_then_rotates_within_8_bits,
    "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#01;
  y := ROR(x, 1);
END_PROGRAM
",
    &[("x", 0x01), ("y", 0x80)],
);

// SHL(BYTE#16#FF, 4) = 0xF0 (shifted to 0xFF0, truncated to u8 = 0xF0)
e2e_i32!(
    end_to_end_when_shl_byte_overflow_then_truncates,
    "
PROGRAM main
  VAR
    x : BYTE;
    y : BYTE;
  END_VAR
  x := BYTE#16#FF;
  y := SHL(x, 4);
END_PROGRAM
",
    &[("x", 0xFF), ("y", 0xF0)],
);

#[test]
fn end_to_end_when_shl_with_zero_shift_then_unchanged() {
    let source = "
PROGRAM main
  VAR
    x : DWORD;
    y : DWORD;
  END_VAR
  x := DWORD#16#DEADBEEF;
  y := SHL(x, 0);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<u32>("y"), 0xDEAD_BEEF);
}

// --- Nested function calls ---

e2e_i32!(
    end_to_end_when_shr_with_abs_then_computes_correctly,
    "
PROGRAM main
  VAR
    a : DINT;
    result : DINT;
  END_VAR
  a := -8;
  result := SHR(ABS(a), 1);
END_PROGRAM
",
    &[("a", -8), ("result", 4)],
);
