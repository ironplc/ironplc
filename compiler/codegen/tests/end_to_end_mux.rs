//! End-to-end integration tests for the MUX function.

#[macro_use]
mod common;

e2e_i32!(
    end_to_end_when_mux_k0_then_returns_in0,
    "PROGRAM main VAR y : DINT; END_VAR y := MUX(0, 10, 20, 30); END_PROGRAM",
    &[(0, 10)],
);

e2e_i32!(
    end_to_end_when_mux_k1_then_returns_in1,
    "PROGRAM main VAR y : DINT; END_VAR y := MUX(1, 10, 20, 30); END_PROGRAM",
    &[(0, 20)],
);

e2e_i32!(
    end_to_end_when_mux_k2_then_returns_in2,
    "PROGRAM main VAR y : DINT; END_VAR y := MUX(2, 10, 20, 30); END_PROGRAM",
    &[(0, 30)],
);

// K=5 is out of range (only 3 inputs), clamps to last = 30.
e2e_i32!(
    end_to_end_when_mux_k_out_of_range_then_clamps_to_last,
    "PROGRAM main VAR y : DINT; END_VAR y := MUX(5, 10, 20, 30); END_PROGRAM",
    &[(0, 30)],
);

// K=-1 clamps to 0 = first input = 10.
e2e_i32!(
    end_to_end_when_mux_k_negative_then_clamps_to_first,
    "PROGRAM main VAR k : DINT; y : DINT; END_VAR k := -1; y := MUX(k, 10, 20, 30); END_PROGRAM",
    &[(1, 10)],
);

e2e_i32!(
    end_to_end_when_mux_with_variable_selector_then_selects,
    "PROGRAM main VAR k : DINT; y : DINT; END_VAR k := 1; y := MUX(k, 100, 200, 300); END_PROGRAM",
    &[(0, 1), (1, 200)],
);

e2e_i32!(
    end_to_end_when_mux_2_inputs_then_works,
    "PROGRAM main VAR y : DINT; END_VAR y := MUX(1, 42, 99); END_PROGRAM",
    &[(0, 99)],
);

e2e_i32!(
    end_to_end_when_mux_4_inputs_then_works,
    "PROGRAM main VAR y : DINT; END_VAR y := MUX(3, 10, 20, 30, 40); END_PROGRAM",
    &[(0, 40)],
);

e2e_i32!(
    end_to_end_when_mux_16_inputs_then_selects_last,
    "PROGRAM main VAR y : DINT; END_VAR y := MUX(15, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16); END_PROGRAM",
    &[(0, 16)],
);

e2e_i32!(
    end_to_end_when_mux_16_inputs_k0_then_selects_first,
    "PROGRAM main VAR y : DINT; END_VAR y := MUX(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16); END_PROGRAM",
    &[(0, 1)],
);

e2e_i32!(
    end_to_end_when_mux_16_inputs_k7_then_selects_middle,
    "PROGRAM main VAR y : DINT; END_VAR y := MUX(7, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16); END_PROGRAM",
    &[(0, 8)],
);

// K=3 with 3 inputs (indices 0..2), should clamp to IN2.
e2e_i32!(
    end_to_end_when_mux_k_equals_input_count_then_clamps_to_last,
    "PROGRAM main VAR y : DINT; END_VAR y := MUX(3, 10, 20, 30); END_PROGRAM",
    &[(0, 30)],
);
