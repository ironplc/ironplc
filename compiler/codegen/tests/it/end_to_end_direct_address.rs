//! End-to-end integration tests for located variables whose direct address
//! has fields of more than one digit (`%MW10`, `%QD100`).

// The example on docs/reference/language/variables/io-qualifiers.rst; keep
// the two in step.
e2e_i32!(
    end_to_end_when_io_qualifiers_docs_example_then_runs,
    "
PROGRAM main
    VAR
        start_button AT %IX0.0 : BOOL;
        motor_output AT %QX0.0 : BOOL;
        speed_setpoint AT %MW10 : INT;
    END_VAR

    motor_output := start_button;
END_PROGRAM
",
    &[
        ("start_button", 0),
        ("motor_output", 0),
        ("speed_setpoint", 0)
    ],
);

e2e_i32!(
    end_to_end_when_located_vars_have_multi_digit_fields_then_assignments_stored,
    "
PROGRAM main
    VAR
        speed_setpoint AT %MW10 : INT;
        total AT %QD100 : DINT;
        limit AT %IX12.7 : BOOL;
    END_VAR

    speed_setpoint := 1500;
    total := 100000;
    limit := total > 99999;
END_PROGRAM
",
    &[("speed_setpoint", 1500), ("total", 100_000), ("limit", 1)],
);
