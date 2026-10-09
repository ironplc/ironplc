//! End-to-end tests for operations on a subrange and on a field reached
//! through an element of an array of structures, which compute at their own
//! type, as on a variable of the base or field type, rather than at the type
//! of their context.

use spec_test_macro::spec_test;

e2e_i32!(
    #[spec_test(REQ_IC_codegen_012)]
    end_to_end_when_subrange_compared_with_wider_integer_then_wider_kept,
    "TYPE Small : INT (-100..100); END_TYPE
     PROGRAM main
     VAR s : Small := 10; l : LINT := 4294967297; gt : BOOL; lt : BOOL; END_VAR
       gt := s > l;
       lt := s < l;
     END_PROGRAM",
    &[("gt", 0), ("lt", 1)],
);

e2e_i64!(
    #[spec_test(REQ_IC_codegen_012)]
    end_to_end_when_subrange_product_assigned_to_lint_then_computes_at_base_type,
    "TYPE Wide : DINT (-100000..100000); END_TYPE
     PROGRAM main
     VAR s : Wide := 100000; d : DINT := 100000; product : LINT; expected : LINT; END_VAR
       product := s * s;
       expected := d * d;
     END_PROGRAM",
    &[("product", 1_410_065_408), ("expected", 1_410_065_408)],
);

e2e_i64!(
    #[spec_test(REQ_IC_codegen_012)]
    end_to_end_when_max_of_unsigned_subrange_assigned_to_lint_then_keeps_value,
    "TYPE Big : UDINT (0..4000000000); END_TYPE
     PROGRAM main
     VAR u : Big := 4000000000; v : Big := 1; l : LINT; END_VAR
       l := MAX(u, v);
     END_PROGRAM",
    &[("l", 4_000_000_000)],
);

e2e_i64!(
    #[spec_test(REQ_IC_codegen_012)]
    end_to_end_when_fields_of_array_of_structures_combined_then_compute_at_field_types,
    "TYPE Item : STRUCT a : DINT; b : LINT; END_STRUCT; END_TYPE
     TYPE Holder : STRUCT items : ARRAY[1..2] OF Item; END_STRUCT; END_TYPE
     PROGRAM main
     VAR h : Holder; product : LINT; sum : LINT; END_VAR
       h.items[1].a := 100000;
       h.items[2].a := 100000;
       h.items[1].b := 5000000000;
       product := h.items[1].a * h.items[2].a;
       sum := h.items[1].b + h.items[2].a;
     END_PROGRAM",
    &[("product", 1_410_065_408), ("sum", 5_000_100_000)],
);
