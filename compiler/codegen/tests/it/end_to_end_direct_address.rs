//! End-to-end integration tests for located variables whose direct address
//! has multi-digit fields (`%MW10`, `%QD100`).

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
    &[(0, 0), (1, 0), (2, 0)],
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
    total := speed_setpoint * 1000;
    limit := total > 1000000;
END_PROGRAM
",
    &[(0, 1500), (1, 1_500_000), (2, 1)],
);
