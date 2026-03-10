//! Shared helpers for codegen-based benchmarks.
//!
//! Compiles IEC 61131-3 Structured Text source into containers via the
//! full pipeline (parser → analyzer → codegen), providing realistic
//! benchmarks that exercise the same code paths as real programs.

#![allow(dead_code)]

use ironplc_codegen::compile;
use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_parser::parse_program;

/// Compiles an IEC 61131-3 source string through the full pipeline.
fn compile_st(source: &str) -> Container {
    let library = parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap();
    let (analyzed, _ctx) = ironplc_analyzer::stages::resolve_types(&[&library]).unwrap();
    compile(&analyzed).unwrap()
}

/// Counter loop: WHILE loop decrementing a DINT variable.
///
/// The initial value of `counter` is set by the caller via `VmBuffers`.
pub fn counter_loop() -> Container {
    compile_st(
        "PROGRAM main
  VAR counter : DINT; END_VAR
  WHILE counter > 0 DO
    counter := counter - 1;
  END_WHILE;
END_PROGRAM",
    )
}

/// Straight-line i32 arithmetic with no branches.
///
/// Generates `repetitions` chained arithmetic operations on a DINT variable.
pub fn arithmetic_i32(repetitions: usize) -> (Container, String) {
    let mut body = String::from(
        "PROGRAM main
  VAR x : DINT; END_VAR
",
    );
    for _ in 0..repetitions {
        body.push_str("  x := (x + 7 - 3) * 2;\n");
    }
    body.push_str("END_PROGRAM\n");
    let container = compile_st(&body);
    (container, body)
}

/// Straight-line f64 arithmetic with no branches.
///
/// Generates `repetitions` chained arithmetic operations on an LREAL variable.
pub fn arithmetic_f64(repetitions: usize) -> (Container, String) {
    let mut body = String::from(
        "PROGRAM main
  VAR x : LREAL; END_VAR
",
    );
    for _ in 0..repetitions {
        body.push_str("  x := (x + 7.0 - 3.0) * 2.0;\n");
    }
    body.push_str("END_PROGRAM\n");
    let container = compile_st(&body);
    (container, body)
}

/// IF-ELSIF branching chain with `branches` comparisons.
///
/// The variable `sel` is compared against each branch value; `result` is
/// assigned in the matching branch. Worst case: `sel` matches the last branch.
pub fn branching(branches: usize) -> (Container, String) {
    let mut body = String::from(
        "PROGRAM main
  VAR sel : DINT; result : DINT; END_VAR
",
    );
    for i in 0..branches {
        if i == 0 {
            body.push_str(&format!("  IF sel = {} THEN\n    result := {};\n", i, i));
        } else {
            body.push_str(&format!("  ELSIF sel = {} THEN\n    result := {};\n", i, i));
        }
    }
    body.push_str("  END_IF;\nEND_PROGRAM\n");
    let container = compile_st(&body);
    (container, body)
}

/// FOR loop summing integers from 1 to `limit`.
pub fn for_loop_sum() -> Container {
    compile_st(
        "PROGRAM main
  VAR i : DINT; sum : DINT; limit : DINT; END_VAR
  sum := 0;
  FOR i := 1 TO limit DO
    sum := sum + i;
  END_FOR;
END_PROGRAM",
    )
}

/// Nested loops: outer × inner iterations of arithmetic.
pub fn nested_loops() -> Container {
    compile_st(
        "PROGRAM main
  VAR i : DINT; j : DINT; acc : DINT;
      outer : DINT; inner : DINT; END_VAR
  acc := 0;
  FOR i := 1 TO outer DO
    FOR j := 1 TO inner DO
      acc := acc + i * j;
    END_FOR;
  END_FOR;
END_PROGRAM",
    )
}
