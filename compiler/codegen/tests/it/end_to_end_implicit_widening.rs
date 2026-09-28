//! End-to-end tests for implicit widening when a variable is read as a
//! wider or real type (#1812): the value is converted, not its bits
//! reinterpreted.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_run;

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
    &[(0, -3.0)],
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
    &[(0, 7.0)],
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
    &[(0, 5.0)],
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
    &[(0, 1.5)],
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
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i64(), 4_000_000_000);
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
    &[(0, -3.0)],
);
