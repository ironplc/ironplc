//! End-to-end tests pairing the two paths a narrow store can take.
//!
//! When the value comes from a constant load the compiler settles the
//! truncation itself and emits no `TRUNC_*`; when it is computed during the
//! scan the VM executes one. The out-of-range tests drive the same value down
//! both paths in one program and assert the two slots agree, so the
//! compile-time fold cannot drift from the VM's wrapping semantics without a
//! test failing.
//!
//! Analysis rejects an out-of-range constant of a numeric type (P2026) but
//! does not range-check a bit string, so a bit string is how an out-of-range
//! constant reaches the fold in a program analysis accepts. Those tests use
//! `BYTE` and `WORD`, with the flags that admit an integer literal and
//! arithmetic on them (ADR-0031, ADR-0053).
//!
//! `a + a` keeps the run-time path honest: the analyzer folds
//! literal-op-literal before codegen, so a computed value needs variable
//! operands to survive as far as the VM.

use crate::common::bit_string_options;

// BYTE: 150 + 150 = 300, wrapped to u8 = 44.
e2e_i32_with!(
    end_to_end_when_byte_overflow_then_folded_matches_computed,
    bit_string_options(),
    "PROGRAM main
       VAR live : BYTE; folded : BYTE; a : BYTE; END_VAR
       a := 150;
       live := a + a;
       folded := 300;
     END_PROGRAM",
    &[("live", 44), ("folded", 44)],
);

// WORD: 40000 + 40000 = 80000, wrapped to u16 = 14464.
e2e_i32_with!(
    end_to_end_when_word_overflow_then_folded_matches_computed,
    bit_string_options(),
    "PROGRAM main
       VAR live : WORD; folded : WORD; a : WORD; END_VAR
       a := 40000;
       live := a + a;
       folded := 80000;
     END_PROGRAM",
    &[("live", 14464), ("folded", 14464)],
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
