//! End-to-end tests for the counters of the wider and unsigned integer
//! types (`CTU_UDINT`, `CTD_LINT`, `CTUD_ULINT`, ...), which count in their
//! own type rather than in a signed 32-bit integer (#2054).

use ironplc_parser::options::CompilerOptions;
use proptest::prelude::*;
use proptest::sample::select;
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::common::scalar::{Mismatch, Num, Scalar, DINT, LINT, UDINT, ULINT};
use crate::common::Snapshot;

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

/// The widths a counter is checked at from any count. The `INT` forms are
/// left out: they count in 32 bits today, past the range of an `INT`.
const WIDE: [Scalar; 4] = [DINT, LINT, UDINT, ULINT];

/// Sets the count of an up counter, a down counter and an up-down counter of
/// `ty` to `start`, shows each one rising edge, and returns what differs from
/// counting one step in `ty`: up to the greatest value and no further, down
/// to the least and no further, with the outputs comparing that count.
fn counted_from(ty: Scalar, start: i128, preset: i128) -> Vec<Mismatch> {
    let source = format!(
        "
PROGRAM main
  VAR
    up : CTU_{ty}; down : CTD_{ty}; both : CTUD_{ty};
    start : {ty} := {start}; preset : {ty} := {preset};
    up_count : {ty}; down_count : {ty}; both_count : {ty};
    up_reached : BOOL; down_at_zero : BOOL; both_reached : BOOL; both_at_zero : BOOL;
  END_VAR
  up.CV := start;
  down.CV := start;
  both.CV := start;
  up(CU := TRUE, R := FALSE, PV := preset);
  down(CD := TRUE, LD := FALSE, PV := preset);
  both(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := preset);
  up_count := up.CV;
  down_count := down.CV;
  both_count := both.CV;
  up_reached := up.Q;
  down_at_zero := down.Q;
  both_reached := both.QU;
  both_at_zero := both.QD;
END_PROGRAM
",
        ty = ty.name
    );
    let (least, greatest) = ty.range();
    let (stepped_up, stepped_down) = ((start + 1).min(greatest), (start - 1).max(least));
    let snapshot = Snapshot::run(&source, &CompilerOptions::default());
    [
        ("up_count", ty.value(Num::Int(stepped_up))),
        ("down_count", ty.value(Num::Int(stepped_down))),
        ("both_count", ty.value(Num::Int(stepped_up))),
        ("up_reached", (stepped_up >= preset).into()),
        ("down_at_zero", (stepped_down <= 0).into()),
        ("both_reached", (stepped_up >= preset).into()),
        ("both_at_zero", (stepped_up <= 0).into()),
    ]
    .into_iter()
    .filter_map(|(name, expected)| {
        Mismatch::of(
            || format!("{name} of {} from {start} with PV {preset}", ty.name),
            snapshot.read(name),
            expected,
        )
    })
    .collect()
}

/// [`counted_from`] every edge value of `ty` as the count, with the preset
/// at the least value of the type, at one and at the greatest.
fn counted_from_edges(ty: Scalar) -> Vec<Mismatch> {
    let (least, greatest) = ty.range();
    ty.edges()
        .iter()
        .flat_map(|start| [least, 1, greatest].map(|preset| (start.trunc(), preset)))
        .flat_map(|(start, preset)| counted_from(ty, start, preset))
        .collect()
}

/// Any wide counter type, with any count and any preset of it.
fn any_count_and_preset() -> impl Strategy<Value = (Scalar, Num, Num)> {
    select(WIDE.to_vec()).prop_flat_map(|ty| (Just(ty), ty.any(), ty.any()))
}

#[rstest]
fn end_to_end_when_counter_at_ends_of_range_then_counts_one_step_and_stops_at_limit(
    #[values(DINT, LINT, UDINT, ULINT)] ty: Scalar,
) {
    assert_eq!(counted_from_edges(ty), []);
}

proptest! {
    // Each case compiles a program, and the anchors above already run every
    // type at the ends of its range, so a quarter of the default cases is
    // enough.
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn end_to_end_when_counter_at_arbitrary_count_then_counts_one_step_and_stops_at_limit(
        (ty, start, preset) in any_count_and_preset(),
    ) {
        prop_assert_eq!(counted_from(ty, start.trunc(), preset.trunc()), []);
    }
}
