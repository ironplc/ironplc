//! End-to-end tests pairing the two paths a narrow store can take.
//!
//! When the value comes from a constant load the compiler settles the
//! truncation itself and emits no `TRUNC_*`; when it is computed during the
//! scan the VM executes one. Each test below drives the same out-of-range
//! value down both paths in one program and asserts the two slots agree, so
//! the compile-time fold cannot drift from the VM's wrapping semantics
//! without a test failing.
//!
//! `a + a` keeps the run-time path honest: the analyzer folds
//! literal-op-literal before codegen, so a computed value needs variable
//! operands to survive as far as the VM.

// SINT: 100 + 100 = 200, wrapped to i8 = -56.
e2e_i32!(
    end_to_end_when_sint_overflow_then_folded_matches_computed,
    "PROGRAM main
       VAR live : SINT; folded : SINT; a : SINT; END_VAR
       a := 100;
       live := a + a;
       folded := 200;
     END_PROGRAM",
    &[(0, -56), (1, -56)],
);

// USINT: 150 + 150 = 300, wrapped to u8 = 44.
e2e_i32!(
    end_to_end_when_usint_overflow_then_folded_matches_computed,
    "PROGRAM main
       VAR live : USINT; folded : USINT; a : USINT; END_VAR
       a := 150;
       live := a + a;
       folded := 300;
     END_PROGRAM",
    &[(0, 44), (1, 44)],
);

// INT: 20000 + 20000 = 40000, wrapped to i16 = -25536.
e2e_i32!(
    end_to_end_when_int_overflow_then_folded_matches_computed,
    "PROGRAM main
       VAR live : INT; folded : INT; a : INT; END_VAR
       a := 20000;
       live := a + a;
       folded := 40000;
     END_PROGRAM",
    &[(0, -25536), (1, -25536)],
);

// UINT: 40000 + 40000 = 80000, wrapped to u16 = 14464.
e2e_i32!(
    end_to_end_when_uint_overflow_then_folded_matches_computed,
    "PROGRAM main
       VAR live : UINT; folded : UINT; a : UINT; END_VAR
       a := 40000;
       live := a + a;
       folded := 80000;
     END_PROGRAM",
    &[(0, 14464), (1, 14464)],
);

// A constant already inside the narrow range keeps its value: the fold drops
// the TRUNC rather than changing what is stored.
e2e_i32!(
    end_to_end_when_constant_in_range_then_value_unchanged,
    "PROGRAM main
       VAR s : SINT; u : USINT; i : INT; w : WORD; END_VAR
       s := -128;
       u := 255;
       i := 32767;
       w := WORD#16#FFFF;
     END_PROGRAM",
    &[(0, -128), (1, 255), (2, 32767), (3, 65535)],
);

// Structure field initialization is constant loads too, and the narrow
// fields are the single largest source of folded truncations in ordinary
// programs. Both the explicit values and the implicit zero defaults go
// through the fold.
e2e_i32!(
    end_to_end_when_struct_narrow_fields_initialized_then_values_correct,
    "TYPE Motor : STRUCT speed : INT; fault : SINT; END_STRUCT; END_TYPE
     PROGRAM main
       VAR m : Motor := (speed := 100, fault := -3); a : INT; b : SINT; END_VAR
       a := m.speed;
       b := m.fault;
     END_PROGRAM",
    &[(1, 100), (2, -3)],
);

e2e_i32!(
    end_to_end_when_struct_narrow_fields_defaulted_then_zero,
    "TYPE Motor : STRUCT speed : INT; fault : SINT; END_STRUCT; END_TYPE
     PROGRAM main
       VAR m : Motor; a : INT; b : SINT; END_VAR
       a := m.speed;
       b := m.fault;
     END_PROGRAM",
    &[(1, 0), (2, 0)],
);
