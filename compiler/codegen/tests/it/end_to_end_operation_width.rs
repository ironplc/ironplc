//! End-to-end tests for an operation on one value whose context is wider
//! than its operand: a negation, `NOT`, `ABS`, a shift or rotate computes at
//! its operand's type and widens the result, rather than computing at the
//! width of its context (#2124).

use spec_test_macro::spec_test;

e2e_i64!(
    #[spec_test(REQ_IC_codegen_003)]
    end_to_end_when_shift_of_dword_assigned_to_lword_then_shifts_32_bits,
    "PROGRAM main
     VAR dw : DWORD := 16#80000000; one : DWORD := 1;
         shl_r : LWORD; shr_r : LWORD; rol_r : LWORD; ror_r : LWORD; END_VAR
     shl_r := SHL(dw, 1); shr_r := SHR(dw, 31); rol_r := ROL(dw, 1); ror_r := ROR(one, 1);
     END_PROGRAM",
    &[
        ("shl_r", 0),
        ("shr_r", 1),
        ("rol_r", 1),
        ("ror_r", 0x8000_0000)
    ],
);

e2e_i64!(
    end_to_end_when_not_of_bit_string_assigned_to_lword_then_inverts_its_own_bits,
    "PROGRAM main
     VAR dz : DWORD := 0; b : BYTE := 16#0F; lw : LWORD; lw_form : LWORD; lb : LWORD; END_VAR
     lw := NOT dz; lw_form := NOT(dz); lb := NOT b;
     END_PROGRAM",
    &[("lw", 0xFFFF_FFFF), ("lw_form", 0xFFFF_FFFF), ("lb", 0xF0)],
);

// Negating the least DINT overflows the DINT and wraps (ADR-0049), as it
// does when assigned to a DINT.
e2e_i64!(
    end_to_end_when_negation_or_abs_of_dint_assigned_to_lint_then_computed_as_dint,
    "PROGRAM main
     VAR d : DINT := -2147483648; neg : LINT; abs_r : LINT; END_VAR
     neg := -d; abs_r := ABS(d);
     END_PROGRAM",
    &[("neg", -2_147_483_648), ("abs_r", -2_147_483_648)],
);

e2e_f64!(
    end_to_end_when_real_function_assigned_to_lreal_then_computed_as_real,
    "PROGRAM main VAR r : REAL := 2.0; lr : LREAL; END_VAR lr := SQRT(r); END_PROGRAM",
    &[("lr", f64::from(2.0_f32.sqrt()))],
);

// A function block input records no conversion yet, so codegen converts the
// result itself.
e2e_i64!(
    end_to_end_when_shift_passed_to_wider_function_block_input_then_shifts_32_bits,
    "FUNCTION_BLOCK Keep VAR_INPUT x : LWORD; END_VAR VAR_OUTPUT y : LWORD; END_VAR
       y := x;
     END_FUNCTION_BLOCK
     PROGRAM main VAR k : Keep; dw : DWORD := 16#80000000; r : LWORD; END_VAR
       k(x := SHL(dw, 1)); r := k.y;
     END_PROGRAM",
    &[("r", 0)],
);
