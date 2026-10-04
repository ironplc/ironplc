//! End-to-end integration tests for the ADD operator.

use ironplc_parser::options::CompilerOptions;

use crate::common::run_scans;

e2e_i32!(
    end_to_end_when_add_expression_then_variable_has_sum,
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 10;
  y := x + 32;
END_PROGRAM
",
    &[("x", 10), ("y", 42)],
);

e2e_i32!(
    end_to_end_when_chain_of_additions_then_variable_has_total,
    "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := 1 + 2 + 3;
END_PROGRAM
",
    &[("result", 6)],
);

e2e_i32!(
    end_to_end_when_multiple_assignments_then_all_variables_correct,
    "
PROGRAM main
  VAR
    a : DINT;
    b : DINT;
    c : DINT;
  END_VAR
  a := 100;
  b := 200;
  c := a + b;
END_PROGRAM
",
    &[("a", 100), ("b", 200), ("c", 300)],
);

e2e_i32!(
    end_to_end_when_deeply_nested_expression_then_correct_result,
    "
PROGRAM main
  VAR
    result : DINT;
  END_VAR
  result := 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10;
END_PROGRAM
",
    &[("result", 55)],
);

// Multi-scan test: the counter accumulates state across scans, so it uses a
// session rather than the single-scan `e2e_i32!` helper.
#[test]
fn end_to_end_when_counter_program_then_increments_across_scans() {
    let source = "
PROGRAM main
  VAR
    count : DINT;
  END_VAR
  count := count + 1;
END_PROGRAM
";
    run_scans(source, &CompilerOptions::default(), |session| {
        for _ in 0..5 {
            session.scan(0).unwrap();
        }
        assert_eq!(session.read("count"), 5);
    });
}
