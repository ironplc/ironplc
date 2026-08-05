//! End-to-end integration tests for the MOD operator.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_try_run;
use ironplc_vm::error::Trap;

e2e_i32!(
    end_to_end_when_mod_expression_then_variable_has_remainder,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 12;
  y := x MOD 5;
END_PROGRAM
",
    &[(0, 12), (1, 2)],
);

// (100 MOD 7) MOD 3 = 2 MOD 3 = 2
e2e_i32!(
    end_to_end_when_chain_of_modulos_then_correct,
    "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 100 MOD 7 MOD 3;
END_PROGRAM
",
    &[(0, 2)],
);

#[test]
fn end_to_end_when_integer_mod_by_zero_then_traps() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x MOD 0;
END_PROGRAM
";
    let result = parse_and_try_run(source, &CompilerOptions::default());

    assert!(result.is_err(), "expected DivideByZero trap");
    assert_eq!(result.unwrap_err().trap, Trap::DivideByZero);
}
