//! Tests for a named string variable passed where a parameter of the other
//! encoding is required. Codegen has no bytecode for that copy and refuses it
//! as P4034 (`resolve_string_arg`); analysis reports it first, as an argument
//! whose type does not match its parameter.

use ironplc_problems::Problem;

rule_err!(
    apply_when_string_parameter_given_wstring_variable_then_error,
    "FUNCTION narrow_len : INT
VAR_INPUT
    s : STRING[10];
END_VAR
    narrow_len := LEN(s);
END_FUNCTION

PROGRAM main
VAR
    w : WSTRING[10];
    n : INT;
END_VAR
    n := narrow_len(w);
END_PROGRAM",
    [Problem::FunctionCallArgTypeMismatch]
);

rule_err!(
    apply_when_string_to_int_given_wstring_variable_then_error,
    "PROGRAM main
VAR
    w : WSTRING[10];
    n : INT;
END_VAR
    n := STRING_TO_INT(w);
END_PROGRAM",
    [Problem::FunctionCallArgTypeMismatch]
);
