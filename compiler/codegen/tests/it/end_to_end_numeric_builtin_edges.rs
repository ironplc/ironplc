//! End-to-end tests for the edges of `ABS`, `LIMIT` and `EXPT` (#1504): an
//! unsigned `ABS`, a `LIMIT` whose `MN` is above its `MX`, and a `LINT`
//! exponent beyond 32 bits.

use ironplc_parser::options::CompilerOptions;

use crate::common::Snapshot;

#[test]
fn end_to_end_when_abs_of_udint_then_identity() {
    let source = "
PROGRAM main
  VAR
    x : UDINT := 3000000000;
    y : UDINT;
  END_VAR
  y := ABS(x);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<u32>("y"), 3_000_000_000);
}

#[test]
fn end_to_end_when_abs_of_ulint_then_identity() {
    let source = "
PROGRAM main
  VAR
    x : ULINT := 10000000000000000000;
    y : ULINT;
  END_VAR
  y := ABS(x);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<u64>("y"), 10_000_000_000_000_000_000);
}

// LIMIT is MIN(MAX(IN, MN), MX), so with MN above MX the result is MX.
e2e_i32!(
    end_to_end_when_limit_dint_min_above_max_then_max,
    "
PROGRAM main
  VAR
    mn : DINT := 10;
    mx : DINT := 5;
    y : DINT;
  END_VAR
  y := LIMIT(mn, 7, mx);
END_PROGRAM
",
    &[("y", 5)],
);

e2e_i64!(
    end_to_end_when_limit_lint_min_above_max_then_max,
    "
PROGRAM main
  VAR
    mn : LINT := 10;
    mx : LINT := 5;
    y : LINT;
  END_VAR
  y := LIMIT(mn, 7, mx);
END_PROGRAM
",
    &[("y", 5)],
);

#[test]
fn end_to_end_when_limit_udint_min_above_max_then_max() {
    let source = "
PROGRAM main
  VAR
    mn : UDINT := 10;
    mx : UDINT := 5;
    y : UDINT;
  END_VAR
  y := LIMIT(mn, 7, mx);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<u32>("y"), 5);
}

#[test]
fn end_to_end_when_limit_ulint_min_above_max_then_max() {
    let source = "
PROGRAM main
  VAR
    mn : ULINT := 10;
    mx : ULINT := 5;
    y : ULINT;
  END_VAR
  y := LIMIT(mn, 7, mx);
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<u64>("y"), 5);
}

e2e_f32!(
    end_to_end_when_limit_real_min_above_max_then_max,
    "
PROGRAM main
  VAR
    mn : REAL := 10.0;
    mx : REAL := 5.0;
    y : REAL;
  END_VAR
  y := LIMIT(mn, 7.0, mx);
END_PROGRAM
",
    &[("y", 5.0)],
);

// 2 ** 2^32 overflows like any large power and wraps to 0; truncating the
// exponent to 32 bits gave 2 ** 0 = 1.
e2e_i64!(
    end_to_end_when_expt_lint_exponent_beyond_32_bits_then_full_exponent,
    "
PROGRAM main
  VAR
    b : LINT := 2;
    e : LINT := 4294967296;
    y : LINT;
  END_VAR
  y := EXPT(b, e);
END_PROGRAM
",
    &[("y", 0)],
);

e2e_i64!(
    end_to_end_when_expt_lint_minus_one_odd_exponent_beyond_32_bits_then_minus_one,
    "
PROGRAM main
  VAR
    b : LINT := -1;
    e : LINT := 4294967297;
    y : LINT;
  END_VAR
  y := EXPT(b, e);
END_PROGRAM
",
    &[("y", -1)],
);
