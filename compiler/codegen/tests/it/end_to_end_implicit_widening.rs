//! End-to-end tests for implicit widening when a variable is read as a
//! wider or real type (#1812): the value is converted, not its bits
//! reinterpreted.

use ironplc_parser::options::CompilerOptions;

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

e2e_i32!(
    end_to_end_when_unsigned_assigned_to_signed_of_same_operation_width_then_keeps_value,
    "
PROGRAM main
  VAR
    us : USINT := 255;
    ui : UINT := 65535;
    usd : DINT;
    uid : DINT;
    usi : INT;
  END_VAR
  usd := us;
  uid := ui;
  usi := us;
END_PROGRAM
",
    &[("usd", 255), ("uid", 65535), ("usi", 255)],
);

e2e_i32!(
    end_to_end_when_unsigned_operand_of_signed_arithmetic_of_same_operation_width_then_keeps_value,
    "PROGRAM main VAR u : USINT := 200; d : DINT := 5; r : DINT; n : DINT; END_VAR
       r := u + d; n := u - d * 50;
     END_PROGRAM",
    &[("r", 205), ("n", -50)],
);

e2e_i32!(
    end_to_end_when_unsigned_input_of_signed_function_of_same_operation_width_then_keeps_value,
    "FUNCTION less_one : DINT VAR_INPUT x : DINT; END_VAR less_one := x - 1; END_FUNCTION
     PROGRAM main
       VAR u : USINT := 200; ui : UINT := 65535; k : USINT := 1; d : DINT := 5;
           biggest : DINT; selected : DINT; passed : DINT; END_VAR
       biggest := MAX(u, d); selected := MUX(k, d, ui); passed := less_one(u);
     END_PROGRAM",
    &[("biggest", 200), ("selected", 65535), ("passed", 199)],
);
