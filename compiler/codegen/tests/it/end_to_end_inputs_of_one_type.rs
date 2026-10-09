//! End-to-end tests for a function of several inputs of one type -- `MIN`,
//! `MAX`, `LIMIT`, `SEL`, `MUX`, `EXPT`, `ATAN2` -- which computes at the type
//! the analyzer recorded for the call, the type every input widens to,
//! whatever the type of its context (#2127).

use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

fn fb_inheritance_options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

e2e_i64!(
    #[spec_test(REQ_IC_codegen_004)]
    end_to_end_when_max_of_int_and_wider_lint_then_selects_at_lint,
    "PROGRAM main VAR i : INT := 2; l2 : LINT := 5000000000; l : LINT; l_first : LINT; END_VAR
     l := MAX(i, l2); l_first := MAX(l2, i); END_PROGRAM",
    &[("l", 5_000_000_000), ("l_first", 5_000_000_000)],
);

// 3000000000 does not fit a DINT: passed to a DINT input it keeps its bits,
// -1294967296. Compared at the DINT of its context it was the smaller of the
// two, and 1 was selected.
e2e_i64_with!(
    end_to_end_when_max_of_udints_in_dint_context_then_selects_the_larger,
    fb_inheritance_options(),
    "FUNCTION_BLOCK Keep VAR_INPUT x : DINT; END_VAR VAR_OUTPUT y : DINT; END_VAR
       y := x;
     METHOD keep : DINT VAR_INPUT x : DINT; END_VAR keep := x; END_METHOD
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR u1 : UDINT := 3000000000; u2 : UDINT := 1; k : Keep;
         by_block : DINT; by_method : DINT; by_udint : UDINT; by_lint : LINT; END_VAR
       k(x := MAX(u1, u2)); by_block := k.y;
       by_method := k.keep(MAX(u1, u2));
       by_udint := MAX(u1, u2);
       by_lint := MAX(u1, u2);
     END_PROGRAM",
    &[
        ("by_block", -1_294_967_296),
        ("by_method", -1_294_967_296),
        ("by_udint", 3_000_000_000),
        ("by_lint", 3_000_000_000)
    ],
);

e2e_i64!(
    end_to_end_when_limit_of_udints_in_dint_context_then_clamps_unsigned,
    "FUNCTION_BLOCK Keep VAR_INPUT x : DINT; END_VAR VAR_OUTPUT y : DINT; END_VAR
       y := x;
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR lo : UDINT := 1; x : UDINT := 3000000000; hi : UDINT := 4000000000;
         k : Keep; clamped : DINT; END_VAR
       k(x := LIMIT(lo, x, hi)); clamped := k.y;
     END_PROGRAM",
    &[("clamped", -1_294_967_296)],
);

// MAX on a time is not accepted (its inputs are ANY_NUM), but a subrange is:
// it selects at its base type, unsigned for a UDINT.
e2e_i64!(
    end_to_end_when_max_of_unsigned_subrange_then_selects_at_base_type,
    "TYPE Big : UDINT (0..4000000000); END_TYPE
     FUNCTION_BLOCK Keep VAR_INPUT x : DINT; END_VAR VAR_OUTPUT y : DINT; END_VAR
       y := x;
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR s1 : Big := 3000000000; s2 : Big := 1; k : Keep; by_block : DINT; by_lint : LINT; END_VAR
       k(x := MAX(s1, s2)); by_block := k.y;
       by_lint := MAX(s1, s2);
     END_PROGRAM",
    &[("by_block", -1_294_967_296), ("by_lint", 3_000_000_000)],
);

e2e_i64!(
    end_to_end_when_mux_selector_is_lint_then_selects_by_it,
    "PROGRAM main
     VAR k : LINT := 2; a : LINT := 10; b : LINT := 20; c : LINT := 5000000000; m : LINT; END_VAR
       m := MUX(k, a, b, c);
     END_PROGRAM",
    &[("m", 5_000_000_000)],
);

e2e_i64!(
    end_to_end_when_sel_of_int_and_lint_then_selects_at_lint,
    "PROGRAM main
     VAR g : BOOL := TRUE; i : INT := 1; l : LINT := 5000000000; r : LINT; END_VAR
       r := SEL(g, i, l);
     END_PROGRAM",
    &[("r", 5_000_000_000)],
);

e2e_f64!(
    end_to_end_when_expt_and_atan2_of_reals_assigned_to_lreal_then_computed_at_real,
    "PROGRAM main
     VAR a : REAL := 1.1; b : REAL := 2.5; ex : LREAL; angle : LREAL; END_VAR
       ex := EXPT(a, b);
       angle := ATAN2(a, b);
     END_PROGRAM",
    &[
        ("ex", f64::from(1.1_f32.powf(2.5_f32))),
        ("angle", f64::from(1.1_f32.atan2(2.5_f32)))
    ],
);
