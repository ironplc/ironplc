//! End-to-end tests for `AND`, `OR` and `XOR` on bit strings of different
//! widths: the operation computes at the type every operand widens to and its
//! result widens by its own type, so no bit of the widest operand is lost and
//! an unsigned result is not sign-extended.

use spec_test_macro::spec_test;

e2e_i64!(
    #[spec_test(REQ_IC_codegen_008)]
    end_to_end_when_bitwise_operand_wider_on_either_side_then_keeps_its_bits,
    "PROGRAM main
     VAR lw : LWORD := 16#100000000; w : WORD := 1; left_wide : LWORD; right_wide : LWORD; END_VAR
     left_wide := lw OR w; right_wide := w OR lw;
     END_PROGRAM",
    &[("left_wide", 0x1_0000_0001), ("right_wide", 0x1_0000_0001)],
);

e2e_i64!(
    #[spec_test(REQ_IC_codegen_008)]
    end_to_end_when_dword_bitwise_result_assigned_to_lword_then_zero_extended,
    "PROGRAM main
     VAR d1 : DWORD := 16#80000000; d2 : DWORD := 1; op : LWORD; form : LWORD; END_VAR
     op := d1 OR d2; form := OR(d1, d2);
     END_PROGRAM",
    &[("op", 0x8000_0001), ("form", 0x8000_0001)],
);

e2e_i64!(
    end_to_end_when_bitwise_function_form_of_several_widths_then_computes_at_widest,
    "PROGRAM main
     VAR lw : LWORD := 16#100000000; d : DWORD := 16#80000000; w : WORD := 1;
         and_r : LWORD; xor_r : LWORD; END_VAR
     and_r := AND(w, lw); xor_r := XOR(w, d, lw);
     END_PROGRAM",
    &[("and_r", 0), ("xor_r", 0x1_8000_0001)],
);
