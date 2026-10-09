//! End-to-end tests for a value stored into a variable of another type: an
//! input of a function block call, an assignment to a function block field or
//! through a reference, an argument of a method call, and the bounds of a
//! `FOR` loop. The value is converted to the variable's declared type by its
//! own signedness, as an assigned value is.

use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

e2e_i64!(
    end_to_end_when_function_block_input_narrower_than_field_then_widened_by_its_signedness,
    "FUNCTION_BLOCK Keep VAR_INPUT x : LINT; END_VAR VAR_OUTPUT y : LINT; END_VAR
       y := x;
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR k : Keep; u : UDINT := 4000000000; d : DINT := -5; e : DINT := 2;
         a : LINT; b : LINT; c : LINT; END_VAR
       k(x := u); a := k.y;
       k(x := d); b := k.y;
       k(x := d + e); c := k.y;
     END_PROGRAM",
    &[("a", 4_000_000_000), ("b", -5), ("c", -3)],
);

e2e_f32!(
    end_to_end_when_integer_function_block_input_to_real_field_then_converted,
    "FUNCTION_BLOCK Keep VAR_INPUT x : REAL; END_VAR VAR_OUTPUT y : REAL; END_VAR
       y := x;
     END_FUNCTION_BLOCK
     PROGRAM main VAR k : Keep; i : INT := -3; r : REAL; END_VAR
       k(x := i); r := k.y;
     END_PROGRAM",
    &[("r", -3.0)],
);

e2e_i64!(
    end_to_end_when_function_block_field_assigned_narrower_value_then_widened,
    "FUNCTION_BLOCK Keep VAR_INPUT x : LINT; END_VAR VAR_OUTPUT y : LINT; END_VAR
       y := x;
     END_FUNCTION_BLOCK
     PROGRAM main VAR k : Keep; u : UDINT := 4000000000; a : LINT; END_VAR
       k.x := u; k(); a := k.y;
     END_PROGRAM",
    &[("a", 4_000_000_000)],
);

e2e_i64_with!(
    end_to_end_when_method_argument_narrower_than_parameter_then_widened,
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    },
    "FUNCTION_BLOCK Keep
     VAR v : LINT; END_VAR
     METHOD Set
     VAR_INPUT x : LINT; END_VAR
       v := x;
     END_METHOD
     METHOD Get : LINT
       Get := v;
     END_METHOD
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR k : Keep; u : UDINT := 4000000000; d : DINT := -5; e : DINT := 2;
         a : LINT; b : LINT; END_VAR
       k.Set(u); a := k.Get();
       k.Set(x := d + e); b := k.Get();
     END_PROGRAM",
    &[("a", 4_000_000_000), ("b", -3)],
);

e2e_i64!(
    end_to_end_when_for_bounds_narrower_than_control_then_widened,
    "PROGRAM main VAR l : LINT; d : DINT := -2; e : DINT := 3; sum : LINT; END_VAR
       FOR l := d TO e DO sum := sum + l; END_FOR;
     END_PROGRAM",
    &[("sum", 3)],
);

e2e_i64_with!(
    end_to_end_when_assigned_through_reference_then_converted_to_referenced_type,
    CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    },
    "PROGRAM main
     VAR x : LINT; y : LINT; z : LINT; l : LINT := 5000000000; u : UDINT := 4000000000;
         p : REF_TO LINT; END_VAR
       p := REF(x); p^ := l;
       p := REF(y); p^ := u;
       p := REF(z); p^ := 5000000001;
     END_PROGRAM",
    &[
        ("x", 5_000_000_000),
        ("y", 4_000_000_000),
        ("z", 5_000_000_001)
    ],
);

e2e_f32_with!(
    #[spec_test(REQ_IC_codegen_005)]
    end_to_end_when_real_assigned_through_reference_then_stored_as_real,
    CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    },
    "PROGRAM main VAR f : REAL; q : REF_TO REAL; END_VAR
       q := REF(f); q^ := 1.5;
     END_PROGRAM",
    &[("f", 1.5)],
);

// A field or parameter of a subrange type is stored at the subrange's base
// type (#2131).
e2e_i64_with!(
    end_to_end_when_subrange_field_or_parameter_then_stored_at_base_type,
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    },
    "TYPE R : LINT(0..9000000000); END_TYPE
     FUNCTION_BLOCK Keep
     VAR_INPUT x : R; END_VAR VAR_OUTPUT y : LINT; END_VAR VAR v : LINT; END_VAR
       y := x;
     METHOD Set
     VAR_INPUT m : R; END_VAR
       v := m;
     END_METHOD
     METHOD Get : LINT
       Get := v;
     END_METHOD
     END_FUNCTION_BLOCK
     PROGRAM main VAR k : Keep; u : UDINT := 4000000000; a : LINT; b : LINT; c : LINT; END_VAR
       k(x := 4000000001); a := k.y;
       k(x := u); b := k.y;
       k.Set(4000000002); c := k.Get();
     END_PROGRAM",
    &[
        ("a", 4_000_000_001),
        ("b", 4_000_000_000),
        ("c", 4_000_000_002)
    ],
);
