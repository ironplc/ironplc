//! End-to-end tests for the counters of the wider and unsigned integer
//! types (`CTU_UDINT`, `CTD_LINT`, `CTUD_ULINT`, ...), which count in their
//! own type rather than in a signed 32-bit integer (#2054).

use spec_test_macro::spec_test;

// Loading a CTD_LINT sets CV to a preset beyond 32 bits, read back as a
// field and as an output.
e2e_i64!(
    #[spec_test(REQ_CW_codegen_001)]
    end_to_end_when_ctd_lint_loads_preset_beyond_32_bits_then_cv_keeps_it,
    "PROGRAM main
     VAR down : CTD_LINT; big : LINT := 5000000000; field : LINT; output : LINT; END_VAR
       down(CD := FALSE, LD := TRUE, PV := big, CV => output);
       field := down.CV;
     END_PROGRAM",
    &[("field", 5_000_000_000), ("output", 5_000_000_000)],
);

// Loading a CTUD_LINT sets CV to a preset beyond 32 bits.
e2e_i64!(
    #[spec_test(REQ_CW_codegen_001)]
    end_to_end_when_ctud_lint_loads_preset_beyond_32_bits_then_cv_keeps_it,
    "PROGRAM main
     VAR c : CTUD_LINT; cv : LINT; END_VAR
       c(CU := FALSE, CD := FALSE, R := FALSE, LD := TRUE, PV := LINT#5000000000);
       cv := c.CV;
     END_PROGRAM",
    &[("cv", 5_000_000_000)],
);

// A CTU_UDINT preset above the largest DINT is not reached by a CV of 0.
e2e_i32!(
    #[spec_test(REQ_CW_codegen_001)]
    end_to_end_when_ctu_udint_preset_above_dint_range_then_q_false,
    "PROGRAM main
     VAR up : CTU_UDINT; big : UDINT := 3000000000; q : BOOL; END_VAR
       up(CU := FALSE, R := FALSE, PV := big, Q => q);
     END_PROGRAM",
    &[("q", 0)],
);

// A CTD_UDINT counting down from 0 stays at 0 rather than wrapping.
e2e_i32!(
    #[spec_test(REQ_CW_codegen_001)]
    end_to_end_when_ctd_udint_counts_down_from_zero_then_cv_stays_zero,
    "PROGRAM main
     VAR down : CTD_UDINT; q : BOOL; at_zero : BOOL; END_VAR
       down(CD := TRUE, LD := FALSE, PV := UDINT#5, Q => q);
       at_zero := down.CV = UDINT#0;
     END_PROGRAM",
    &[("q", 1), ("at_zero", 1)],
);

// A CTD_ULINT loads a preset above the largest LINT.
e2e_i32!(
    #[spec_test(REQ_CW_codegen_001)]
    end_to_end_when_ctd_ulint_loads_preset_above_lint_range_then_cv_keeps_it,
    "PROGRAM main
     VAR down : CTD_ULINT; q : BOOL; kept : BOOL; END_VAR
       down(CD := FALSE, LD := TRUE, PV := ULINT#10000000000000000000, Q => q);
       kept := down.CV = ULINT#10000000000000000000;
     END_PROGRAM",
    &[("q", 0), ("kept", 1)],
);
