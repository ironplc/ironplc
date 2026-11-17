//! End-to-end test for names written with a library or namespace prefix
//! (`--allow-qualified-type-names`) and enumerated values written
//! `Type.value` (`--allow-dotted-enum-value`): a prefixed type is the type
//! its last segment names, and `Type.value` is the value `Type#value` is, in
//! an initial value and in a CASE label.

use ironplc_parser::options::CompilerOptions;

e2e_i32_with!(
    end_to_end_when_prefixed_type_and_dotted_enumerated_values_then_runs_as_unprefixed,
    CompilerOptions {
        allow_qualified_type_names: true,
        allow_dotted_enum_value: true,
        ..CompilerOptions::default()
    },
    "TYPE E_Mode : (idle, run, stop); END_TYPE

FUNCTION_BLOCK FB_Step
VAR_INPUT
    mode : E_Mode;
END_VAR
VAR_OUTPUT
    code : DINT;
END_VAR
CASE mode OF
    E_Mode.idle: code := 10;
    Lib.E_Mode.run: code := 20;
ELSE
    code := 30;
END_CASE;
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : E_Mode := E_Mode.run;
    n : Lib.E_Mode := Lib.E_Mode.stop;
    fb : Lib.FB_Step;
    a : DINT;
    b : DINT;
END_VAR
fb(mode := m);
a := fb.code;
fb(mode := n);
b := fb.code;
END_PROGRAM",
    &[("m", 1), ("n", 2), ("a", 20), ("b", 30)],
);
