//! End-to-end tests that a value stored in a field of a function block
//! instance, as an input of a call or by an assignment to `inst.x`, is
//! narrowed to the field's declared type, as a store to a variable of that
//! type is. A field used to be stored at its operation type, so an `INT`
//! input passed 40000 held 40000.

e2e_i32!(
    end_to_end_when_call_input_does_not_fit_field_then_narrowed,
    "FUNCTION_BLOCK FB
     VAR_INPUT x : INT; END_VAR
     VAR_OUTPUT y : DINT; END_VAR
       y := x;
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR f : FB; g : FB; a : INT := 20000; b : INT := 20000; wrapped : DINT; fits : DINT; END_VAR
       f(x := a + b, y => wrapped);
       g(x := a, y => fits);
     END_PROGRAM",
    &[("wrapped", -25536), ("fits", 20000)],
);

e2e_i32!(
    end_to_end_when_field_assigned_value_that_does_not_fit_then_narrowed,
    "FUNCTION_BLOCK FB
     VAR_INPUT x : INT; END_VAR
     VAR_OUTPUT y : DINT; END_VAR
       y := x;
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR f : FB; a : INT := 20000; b : INT := 20000; field : DINT; output : DINT; END_VAR
       f.x := a + b;
       field := f.x;
       f();
       output := f.y;
     END_PROGRAM",
    &[("field", -25536), ("output", -25536)],
);

e2e_i32!(
    end_to_end_when_unsigned_call_input_does_not_fit_field_then_wrapped,
    "FUNCTION_BLOCK FB
     VAR_INPUT x : USINT; END_VAR
     VAR_OUTPUT y : DINT; END_VAR
       y := x;
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR f : FB; u : USINT := 200; wrapped : DINT; END_VAR
       f(x := u + u, y => wrapped);
     END_PROGRAM",
    &[("wrapped", 144)],
);

e2e_i32!(
    end_to_end_when_standard_block_input_does_not_fit_field_then_narrowed,
    "PROGRAM main
     VAR c : CTU; a : INT := 20000; b : INT := 20000; preset : DINT; END_VAR
       c(CU := FALSE, PV := a + b);
       preset := c.PV;
     END_PROGRAM",
    &[("preset", -25536)],
);
