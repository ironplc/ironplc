//! End-to-end integration tests for variables declared with a subrange
//! spelled in place (`x : INT(5..15)`).

// Without an initial value, an inline subrange starts at its lower bound.
e2e_i32!(
    end_to_end_when_inline_subrange_var_no_init_then_default_is_lower_bound,
    "
PROGRAM main
  VAR
    x : INT(5..15);
    y : INT(-10..-2);
  END_VAR
END_PROGRAM
",
    &[("x", 5), ("y", -10)],
);

e2e_i32!(
    end_to_end_when_inline_subrange_var_with_init_then_uses_init_value,
    "
PROGRAM main
  VAR
    x : INT(5..15) := 9;
    y : INT(-10..15) := -4;
  END_VAR
END_PROGRAM
",
    &[("x", 9), ("y", -4)],
);

e2e_i32!(
    end_to_end_when_inline_subrange_several_names_then_each_uses_init_value,
    "
PROGRAM main
  VAR
    x, y : UINT(2..15) := 4;
  END_VAR
END_PROGRAM
",
    &[("x", 4), ("y", 4)],
);

e2e_i32!(
    end_to_end_when_inline_subrange_unsigned_base_then_default_is_lower_bound,
    "
PROGRAM main
  VAR
    x : USINT(10..200);
  END_VAR
END_PROGRAM
",
    &[("x", 10)],
);

e2e_i64!(
    end_to_end_when_inline_subrange_64_bit_base_then_default_is_lower_bound,
    "
PROGRAM main
  VAR
    x : LINT(-5000000000..5000000000);
    y : ULINT(0..10000000000) := 6000000000;
  END_VAR
END_PROGRAM
",
    &[("x", -5_000_000_000), ("y", 6_000_000_000)],
);

e2e_i32!(
    end_to_end_when_inline_subrange_constant_then_reads_value,
    "
PROGRAM main
  VAR CONSTANT
    c : INT(5..15) := 7;
  END_VAR
  VAR
    y : INT;
  END_VAR
  y := c;
END_PROGRAM
",
    &[("y", 7)],
);

e2e_i32!(
    end_to_end_when_inline_subrange_located_then_uses_init_value,
    "
PROGRAM main
  VAR
    x AT %MW0 : INT(5..15) := 9;
  END_VAR
END_PROGRAM
",
    &[("x", 9)],
);

e2e_i32!(
    end_to_end_when_inline_subrange_var_temp_then_default_is_lower_bound,
    "
PROGRAM main
  VAR_TEMP
    t : INT(5..15);
  END_VAR
  VAR
    y : INT;
  END_VAR
  y := t;
END_PROGRAM
",
    &[("y", 5)],
);

e2e_i32!(
    end_to_end_when_inline_subrange_function_block_input_output_then_starting_values,
    "
FUNCTION_BLOCK FB
  VAR_INPUT
    i : INT(5..15);
  END_VAR
  VAR_OUTPUT
    o : INT(6..15) := 8;
  END_VAR
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    f : FB;
    a : INT;
    b : INT;
  END_VAR
  a := f.i;
  b := f.o;
END_PROGRAM
",
    &[("a", 5), ("b", 8)],
);

// A function's inline subrange local starts at its value on every call.
e2e_i32!(
    end_to_end_when_inline_subrange_function_local_then_starts_at_value_each_call,
    "
FUNCTION F : INT
  VAR
    t : INT(3..9) := 4;
  END_VAR
  F := t;
  t := 9;
END_FUNCTION

PROGRAM main
  VAR
    a : INT;
    b : INT;
  END_VAR
  a := F();
  b := F();
END_PROGRAM
",
    &[("a", 4), ("b", 4)],
);

e2e_i32!(
    end_to_end_when_inline_subrange_global_then_external_reads_value,
    "
CONFIGURATION config
  VAR_GLOBAL
    g : INT(5..15) := 9;
    h : INT(5..15);
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    g : INT(5..15);
    h : INT(5..15);
  END_VAR
  VAR
    a : INT;
    b : INT;
  END_VAR
  a := g;
  b := h;
END_PROGRAM
",
    &[("a", 9), ("b", 5)],
);

// A subrange is not enforced at run time: a value computed outside the
// range is stored as the base type stores it.
e2e_i32!(
    end_to_end_when_inline_subrange_assigned_value_outside_range_then_stores_value,
    "
PROGRAM main
  VAR
    x : INT(0..100);
    n : INT := 90;
  END_VAR
  x := n + 20;
END_PROGRAM
",
    &[("x", 110)],
);
