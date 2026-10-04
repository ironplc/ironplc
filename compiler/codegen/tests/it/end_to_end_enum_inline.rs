//! End-to-end tests for inline enumerations: a variable whose type is the
//! value list itself (`e : (A, B) := B;`) rather than a `TYPE` name.
//!
//! An inline enumeration compiles as a named one with the same value list
//! does: its members, their ordinals and its default are recorded with its
//! anonymous type, and codegen looks a value's ordinal up by that type. See
//! `specs/design/enumeration-codegen.md`, section 10.

use crate::common::parse_and_compile;
use ironplc_container::debug_section::iec_type_tag;
use ironplc_parser::options::{CompilerOptions, Dialect};

fn edition_3() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)
}

// --- Initialization ---

// The program from issue #1873: B is ordinal 1, so the IF takes the THEN arm.
e2e_i32!(
    end_to_end_when_inline_enum_compared_with_its_value_then_matches,
    "PROGRAM main VAR e : (A, B) := B; d : DINT; END_VAR IF e = B THEN d := 1; ELSE d := 2; END_IF; END_PROGRAM",
    &[("e", 1), ("d", 1)],
);

// No initial value: the first member, A = 0.
e2e_i32!(
    end_to_end_when_inline_enum_without_initial_value_then_starts_at_first_value,
    "PROGRAM main VAR e : (A, B, C); END_VAR END_PROGRAM",
    &[("e", 0)],
);

// The initial value resolves against the declaration's own list: GREEN is 0
// in `(GREEN, RED)` although it is 1 in COLOR.
e2e_i32!(
    end_to_end_when_inline_value_also_in_named_enum_then_initial_value_uses_own_list,
    "TYPE COLOR : (RED, GREEN); END_TYPE PROGRAM main VAR e : (GREEN, RED) := GREEN; f : (GREEN, RED) := RED; END_VAR END_PROGRAM",
    &[("e", 0), ("f", 1)],
);

// A named enumeration's initial value is looked up in that enumeration, so
// a value name an inline enumeration shares at another position does not
// disturb it: GREEN is 1 in COLOR.
e2e_i32!(
    end_to_end_when_named_enum_value_also_in_inline_enum_then_initial_value_uses_named_type,
    "TYPE COLOR : (RED, GREEN); END_TYPE PROGRAM main VAR e : (GREEN, RED); c : COLOR := GREEN; END_VAR END_PROGRAM",
    &[("e", 0), ("c", 1)],
);

// --- Body ---

e2e_i32!(
    end_to_end_when_inline_enum_assigned_in_body_then_stores_ordinal,
    "PROGRAM main VAR e : (A, B, C); END_VAR e := C; END_PROGRAM",
    &[("e", 2)],
);

e2e_i32!(
    end_to_end_when_inline_enum_case_then_matches_correct_arm,
    "PROGRAM main VAR e : (A, B, C) := B; d : DINT; END_VAR CASE e OF A: d := 10; B, C: d := 20; END_CASE; END_PROGRAM",
    &[("d", 20)],
);

e2e_i32!(
    end_to_end_when_inline_enum_not_equal_then_true,
    "PROGRAM main VAR e : (A, B) := A; d : BOOL; END_VAR d := e <> B; END_PROGRAM",
    &[("d", 1)],
);

// Y is 1 in `a`'s list and 0 in `b`'s. Each assignment takes Y from its
// target's type.
e2e_i32!(
    end_to_end_when_unqualified_value_in_two_inline_enums_at_different_ordinals_then_uses_target_type,
    "PROGRAM main VAR a : (X, Y); b : (Y, X); END_VAR a := Y; b := Y; END_PROGRAM",
    &[("a", 1), ("b", 0)],
);

// Compared with `b`, Y has `b`'s type, so it is 0 and the test is true.
e2e_i32!(
    end_to_end_when_unqualified_value_compared_then_uses_other_operand_type,
    "PROGRAM main VAR a : (X, Y); b : (Y, X) := Y; d : BOOL; END_VAR d := b = Y; END_PROGRAM",
    &[("d", 1)],
);

// The same value name at the same ordinal in two lists is not ambiguous.
e2e_i32!(
    end_to_end_when_unqualified_value_in_two_inline_enums_at_same_ordinal_then_stores_it,
    "PROGRAM main VAR a : (IDLE, BUSY); b : (IDLE, BUSY); END_VAR a := BUSY; b := BUSY; END_PROGRAM",
    &[("a", 1), ("b", 1)],
);

// --- Variable sections ---

e2e_i32!(
    end_to_end_when_inline_enum_in_program_input_and_output_then_initialized,
    "PROGRAM main VAR_INPUT i : (A, B) := B; END_VAR VAR_OUTPUT o : (X, Y, Z); END_VAR IF i = B THEN o := Z; END_IF; END_PROGRAM",
    &[("i", 1), ("o", 2)],
);

// A function block field declared inline: R is ordinal 2.
e2e_i32!(
    end_to_end_when_inline_enum_function_block_input_then_case_selects_value,
    "FUNCTION_BLOCK FB1
       VAR_INPUT i : (P, Q, R); END_VAR
       VAR_OUTPUT o : DINT; END_VAR
       CASE i OF P: o := 10; Q: o := 20; R: o := 30; END_CASE;
     END_FUNCTION_BLOCK
     PROGRAM main VAR fb : FB1; d : DINT; END_VAR fb(i := R); d := fb.o; END_PROGRAM",
    &[("d", 30)],
);

// A function local is re-initialized on every call: the first call leaves
// it at T0, and the second still starts at T1.
e2e_i32!(
    end_to_end_when_inline_enum_function_local_then_reinitialized_each_call,
    "FUNCTION F1 : DINT
       VAR t : (T0, T1) := T1; END_VAR
       IF t = T1 THEN F1 := 7; ELSE F1 := 3; END_IF;
       t := T0;
     END_FUNCTION
     PROGRAM main VAR a : DINT; b : DINT; END_VAR a := F1(); b := F1(); END_PROGRAM",
    &[("a", 7), ("b", 7)],
);

// --- Explicit member values ---

e2e_i32_with!(
    end_to_end_when_inline_enum_explicit_values_then_uses_them,
    edition_3(),
    "PROGRAM main VAR e : (X := 1, Y := 5) := Y; d : DINT; END_VAR IF e = Y THEN d := 1; END_IF; END_PROGRAM",
    &[("e", 5), ("d", 1)],
);

// The first member is 1, so an uninitialized variable starts at 1, not 0.
e2e_i32_with!(
    end_to_end_when_inline_enum_explicit_values_without_initial_value_then_first_value,
    edition_3(),
    "PROGRAM main VAR e : (X := 1, Y := 5); END_VAR END_PROGRAM",
    &[("e", 1)],
);

e2e_i32_with!(
    end_to_end_when_named_enum_explicit_values_without_default_then_first_value,
    edition_3(),
    "TYPE E : (X := 1, Y := 5); END_TYPE PROGRAM main VAR e : E; END_VAR END_PROGRAM",
    &[("e", 1)],
);

// --- Debug section ---

#[test]
fn end_to_end_when_inline_enum_then_debug_entry_named_by_its_type() {
    let source = "PROGRAM main VAR e : (Red, Green) := Green; END_VAR END_PROGRAM";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let debug = container.debug_section.as_ref().unwrap();

    let var = &debug.var_names[0];
    assert_eq!(var.name, "e");
    assert!(var.type_name.starts_with("(ANONYMOUS ENUMERATION "));
    assert_eq!(var.iec_type_tag, iec_type_tag::DINT);

    let def = debug
        .enum_defs
        .iter()
        .find(|e| e.type_name == var.type_name)
        .unwrap();
    assert_eq!(def.values, vec!["RED", "GREEN"]);
}

// Each declaration declares a type of its own (ADR-0055).
#[test]
fn end_to_end_when_two_inline_enums_spell_same_values_then_one_debug_entry_each() {
    let source = "PROGRAM main VAR a : (A, B); b : (A, B); END_VAR END_PROGRAM";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let debug = container.debug_section.as_ref().unwrap();

    assert_eq!(debug.enum_defs.len(), 2);
    assert_ne!(debug.var_names[0].type_name, debug.var_names[1].type_name);
}

// --- Value names shared by named enumerations (#1943, #1950) ---

// The sources below declare `A : (X, Y)` and `B : (W, X, Z)`.

// X is 0 in A and 1 in B: the initial value, the assignment and the
// comparison each use B's.
e2e_i32!(
    end_to_end_when_named_enums_share_value_then_each_use_resolves_by_type,
    "TYPE A : (X, Y); B : (W, X, Z); END_TYPE PROGRAM main VAR b : B := X; c : B; d : DINT; END_VAR c := X; IF b = X THEN d := 1; END_IF; END_PROGRAM",
    &[("b", 1), ("c", 1), ("d", 1)],
);

// An alias has the values of its base: X is 1 in B, so in BB too.
e2e_i32!(
    end_to_end_when_alias_typed_initial_value_shared_then_uses_alias_base_ordinal,
    "TYPE A : (X, Y); B : (W, X, Z); END_TYPE TYPE BB : B; END_TYPE PROGRAM main VAR b : BB := X; END_VAR END_PROGRAM",
    &[("b", 1)],
);

// A CASE label takes the selector's type: X is 1 in B.
e2e_i32!(
    end_to_end_when_case_label_shared_then_uses_selector_type,
    "TYPE A : (X, Y); B : (W, X, Z); END_TYPE PROGRAM main VAR b : B := X; d : DINT; END_VAR CASE b OF W: d := 10; X: d := 20; END_CASE; END_PROGRAM",
    &[("d", 20)],
);

// --- Structure fields ---

// A structure field of an enumeration type without an initializer starts at
// the enumeration's default.
e2e_i32!(
    end_to_end_when_struct_field_enum_without_initializer_then_enum_default,
    "TYPE L : (LOW, HIGH) := HIGH; S : STRUCT f : L; END_STRUCT; END_TYPE PROGRAM main VAR s : S; e : L; END_VAR e := s.f; END_PROGRAM",
    &[("e", 1)],
);
