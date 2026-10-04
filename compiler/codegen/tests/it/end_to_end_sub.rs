//! End-to-end integration tests for the SUB operator.

use ironplc_parser::options::CompilerOptions;

use crate::common::run_scans;

e2e_i32!(
    end_to_end_when_sub_expression_then_variable_has_difference,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x - 3;
END_PROGRAM
",
    &[("x", 10), ("y", 7)],
);

e2e_i32!(
    end_to_end_when_sub_result_negative_then_correct,
    "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := 3 - 10;
END_PROGRAM
",
    &[("result", -7)],
);

e2e_i32!(
    end_to_end_when_chain_of_subtractions_then_correct,
    "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := 100 - 30 - 20 - 10;
END_PROGRAM
",
    &[("result", 40)],
);

e2e_i32!(
    end_to_end_when_mixed_add_sub_then_correct,
    "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := 10 + 5 - 3;
END_PROGRAM
",
    &[("result", 12)],
);

e2e_i32!(
    end_to_end_when_sub_with_variables_then_correct,
    "
PROGRAM main
  VAR
    a : DINT;
    b : DINT;
    c : DINT;
  END_VAR
  a := 100;
  b := 30;
  c := a - b;
END_PROGRAM
",
    &[("a", 100), ("b", 30), ("c", 70)],
);

e2e_i32!(
    end_to_end_when_sub_zero_then_identity,
    "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42 - 0;
END_PROGRAM
",
    &[("x", 42)],
);

e2e_i32!(
    end_to_end_when_sub_from_zero_then_negation,
    "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 0 - 7;
END_PROGRAM
",
    &[("x", -7)],
);

#[test]
fn end_to_end_when_countdown_program_then_decrements_across_scans() {
    let source = "
PROGRAM main
  VAR
    count : DINT;
  END_VAR
  count := count - 1;
END_PROGRAM
";
    run_scans(source, &CompilerOptions::default(), |session| {
        for _ in 0..5 {
            session.scan(0).unwrap();
        }
        assert_eq!(session.read("count"), -5);
    });
}

// 10 - (-5) = 15
e2e_i32!(
    end_to_end_when_sub_negative_constant_then_effective_addition,
    "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 10 - -5;
END_PROGRAM
",
    &[("x", 15)],
);
