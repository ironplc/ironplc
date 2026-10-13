//! End-to-end tests for implicit widening when a variable is read as a
//! wider or real type (#1812): the value is converted, not its bits
//! reinterpreted.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::Snapshot;

e2e_f32!(
    end_to_end_when_int_assigned_to_real_then_converted,
    "
PROGRAM main
  VAR
    r : REAL;
    i : INT := -3;
  END_VAR
  r := i;
END_PROGRAM
",
    &[("r", -3.0)],
);

e2e_f64!(
    end_to_end_when_dint_assigned_to_lreal_then_converted,
    "
PROGRAM main
  VAR
    l : LREAL;
    d : DINT := 7;
  END_VAR
  l := d;
END_PROGRAM
",
    &[("l", 7.0)],
);

e2e_f64!(
    end_to_end_when_uint_assigned_to_lreal_then_converted,
    "
PROGRAM main
  VAR
    l : LREAL;
    u : UINT := 5;
  END_VAR
  l := u;
END_PROGRAM
",
    &[("l", 5.0)],
);

e2e_f64!(
    end_to_end_when_real_assigned_to_lreal_then_converted,
    "
PROGRAM main
  VAR
    l : LREAL;
    r : REAL := 1.5;
  END_VAR
  l := r;
END_PROGRAM
",
    &[("l", 1.5)],
);

#[test]
fn end_to_end_when_udint_assigned_to_lint_then_zero_extended() {
    let source = "
PROGRAM main
  VAR
    l : LINT;
    u : UDINT := 4000000000;
  END_VAR
  l := u;
END_PROGRAM
";
    let snapshot = Snapshot::run(source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<i64>("l"), 4_000_000_000);
}

e2e_f32!(
    end_to_end_when_int_assigned_to_real_array_element_then_converted,
    "
PROGRAM main
  VAR
    r : REAL;
    i : INT := -3;
    a : ARRAY[1..2] OF REAL;
  END_VAR
  a[1] := i;
  r := a[1];
END_PROGRAM
",
    &[("r", -3.0)],
);

/// An unsigned value stored in a signed target of the same operation width
/// keeps its value, by both routes (REQ-IC-analyzer-105).
#[rstest]
#[case::usint_to_dint("USINT", "DINT", "200", 200)]
#[case::uint_to_dint("UINT", "DINT", "65535", 65535)]
#[case::usint_to_int("USINT", "INT", "255", 255)]
fn end_to_end_when_unsigned_assigned_to_signed_of_same_width_then_keeps_value(
    #[case] src: &str,
    #[case] tgt: &str,
    #[case] value: &str,
    #[case] expected: i32,
) {
    let source = format!(
        "
PROGRAM main
  VAR
    x : {src} := {value};
    r : {tgt};
  END_VAR
  r := x;
END_PROGRAM
"
    );
    let snapshot = Snapshot::run(&source, &CompilerOptions::default());
    assert_eq!(snapshot.read_as::<i32>("r"), expected);
}
