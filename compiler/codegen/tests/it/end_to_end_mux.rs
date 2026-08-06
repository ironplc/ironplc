//! End-to-end integration tests for the MUX function.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;
use proptest::prelude::*;

// --- Deterministic anchors: distinct variadic arities + clamp branch ---

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

// K=5 is out of range (only 3 inputs), clamps to last = 30.
e2e_i32!(
    end_to_end_when_mux_k_out_of_range_then_clamps_to_last,
    "PROGRAM main VAR y : DINT; END_VAR y := MUX(5, 10, 20, 30); END_PROGRAM",
    &[(0, 30)],
);

// --- Property test: MUX(k, a, b, c, d) selects inputs[clamp(k, 0, 3)] ---
// Fixed arity 4. k ranges past both ends to exercise the clamp in both
// directions (negative -> first, >= n -> last), pinned deterministically by
// the anchors above. Oracle is pure Rust.
proptest! {
    #[test]
    fn end_to_end_when_mux_4_inputs_over_range_then_selects_clamped(
        a in any::<i32>(),
        b in any::<i32>(),
        c in any::<i32>(),
        d in any::<i32>(),
        k in -2i32..=6,
    ) {
        let inputs = [a, b, c, d];
        let expected = inputs[k.clamp(0, 3) as usize];
        // k is declared as a variable (matching the negative-selector lowering),
        // so y is the second declared variable at slot 1.
        let source = format!(
            "PROGRAM main VAR k : DINT; y : DINT; END_VAR k := {k}; y := MUX(k, {a}, {b}, {c}, {d}); END_PROGRAM"
        );
        let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
        prop_assert_eq!(bufs.vars[1].as_i32(), expected);
    }
}
