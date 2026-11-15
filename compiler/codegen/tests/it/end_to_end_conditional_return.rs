//! End-to-end tests for the `RETURN(condition)` conditional return
//! (`allow_conditional_return`). The condition decides whether the POU
//! returns; it is never assigned to the return value.

use ironplc_parser::options::CompilerOptions;

fn opts() -> CompilerOptions {
    CompilerOptions {
        allow_conditional_return: true,
        ..CompilerOptions::default()
    }
}

fn opts_with_methods() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..opts()
    }
}

e2e_i32_with!(
    end_to_end_when_program_condition_true_then_skips_rest,
    opts(),
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  x := 1;
  RETURN(x > 0);
  y := 7;
END_PROGRAM
",
    &[("x", 1), ("y", 0)],
);

e2e_i32_with!(
    end_to_end_when_program_condition_false_then_continues,
    opts(),
    "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  RETURN(x > 0);
  y := 7;
END_PROGRAM
",
    &[("y", 7)],
);

// The function result is the last value assigned before the return.
e2e_i32_with!(
    end_to_end_when_function_condition_then_keeps_assigned_result,
    opts(),
    "
FUNCTION f : DINT
  VAR_INPUT
    a : DINT;
  END_VAR
  f := 1;
  RETURN(a > 5);
  f := 2;
END_FUNCTION

PROGRAM main
  VAR
    returned : DINT;
    continued : DINT;
  END_VAR
  returned := f(10);
  continued := f(1);
END_PROGRAM
",
    &[("returned", 1), ("continued", 2)],
);

// In a BOOL function the condition still is not the result: `RETURN(TRUE)`
// returns the FALSE assigned before it.
e2e_i32_with!(
    end_to_end_when_bool_function_condition_true_then_result_not_assigned,
    opts(),
    "
FUNCTION g : BOOL
  g := FALSE;
  RETURN(TRUE);
  g := TRUE;
END_FUNCTION

PROGRAM main
  VAR
    r : BOOL := TRUE;
  END_VAR
  r := g();
END_PROGRAM
",
    &[("r", 0)],
);

// Inside a loop the conditional return leaves the whole function block body,
// not only the loop.
e2e_i32_with!(
    end_to_end_when_condition_true_in_loop_then_leaves_function_block,
    opts(),
    "
FUNCTION_BLOCK FB_Count
  VAR_OUTPUT
    count : DINT;
    after : DINT;
  END_VAR
  VAR
    i : DINT;
  END_VAR
  count := 0;
  after := 0;
  FOR i := 1 TO 10 DO
    count := count + 1;
    RETURN(i = 3);
  END_FOR;
  after := 1;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    fb : FB_Count;
    count : DINT;
    after : DINT;
  END_VAR
  fb();
  count := fb.count;
  after := fb.after;
END_PROGRAM
",
    &[("count", 3), ("after", 0)],
);

e2e_i32_with!(
    end_to_end_when_method_condition_true_then_skips_rest,
    opts_with_methods(),
    "
FUNCTION_BLOCK FB_Motor
  VAR
    speed : DINT;
  END_VAR
  METHOD SetSpeed
    VAR_INPUT
      newSpeed : DINT;
    END_VAR
    RETURN(newSpeed < 0);
    speed := newSpeed;
  END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    m : FB_Motor;
    accepted : DINT;
    rejected : DINT;
  END_VAR
  m.SetSpeed(5);
  accepted := m.speed;
  m.SetSpeed(-1);
  rejected := m.speed;
END_PROGRAM
",
    &[("accepted", 5), ("rejected", 5)],
);
