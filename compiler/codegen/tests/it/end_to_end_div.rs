//! End-to-end integration tests for the DIV operator.

use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_try_run;
use ironplc_vm::error::Trap;

e2e_i32!(
    end_to_end_when_div_expression_then_variable_has_quotient,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 12;
  y := x / 4;
END_PROGRAM
",
    &[(0, 12), (1, 3)],
);

// (100 / 5) / 2 = 10
e2e_i32!(
    end_to_end_when_chain_of_divisions_then_correct,
    "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 100 / 5 / 2;
END_PROGRAM
",
    &[(0, 10)],
);

#[test]
fn end_to_end_when_integer_divide_by_zero_then_traps() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x / 0;
END_PROGRAM
";
    let result = parse_and_try_run(source, &CompilerOptions::default());

    assert!(result.is_err(), "expected DivideByZero trap");
    assert_eq!(result.unwrap_err().trap, Trap::DivideByZero);
}
