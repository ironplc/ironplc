//! End-to-end tests of the narrow stores the compiler settles itself.
//!
//! When the value comes from a constant load the compiler settles the
//! truncation itself and emits no `TRUNC_*`; when it is computed during the
//! scan the VM executes one. Analysis rejects a constant its type cannot hold
//! (P2026), bit strings included, so every constant that reaches the fold is
//! in range, and these tests pin that the fold keeps its value. Wrapping a
//! value computed at run time is covered by `end_to_end_bitstring.rs`.

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
    &[("s", -128), ("u", 255), ("i", 32767), ("w", 65535)],
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
    &[("a", 100), ("b", -3)],
);

e2e_i32!(
    end_to_end_when_struct_narrow_fields_defaulted_then_zero,
    "TYPE Motor : STRUCT speed : INT; fault : SINT; END_STRUCT; END_TYPE
     PROGRAM main
       VAR m : Motor; a : INT; b : SINT; END_VAR
       a := m.speed;
       b := m.fault;
     END_PROGRAM",
    &[("a", 0), ("b", 0)],
);
