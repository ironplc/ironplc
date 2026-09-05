//! End-to-end integration tests for the `AND_THEN` / `OR_ELSE` short-circuit
//! boolean operators.
//!
//! These prove the two things that distinguish the operators from `AND` and
//! `OR`: the answer is the same, and the right operand is *not* evaluated when
//! the left one already decides it. The second is what the operators exist for
//! — the guarded dereference below traps under eager evaluation.
//!
//! See specs/design/beckhoff-twincat-dialect.md §3.4.

use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_vm::error::Trap;

use crate::common::{parse_and_run, parse_and_try_run};

/// Options enabling the short-circuit operators and nothing else.
fn short_circuit_options() -> CompilerOptions {
    CompilerOptions {
        allow_short_circuit_operators: true,
        ..CompilerOptions::default()
    }
}

/// Options enabling the short-circuit operators on top of Edition 3, which is
/// what supplies `REF_TO` / `NULL` / `^` for the guarded-dereference tests.
fn short_circuit_ref_options() -> CompilerOptions {
    CompilerOptions {
        allow_short_circuit_operators: true,
        ..CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)
    }
}

// var layout: a=0, b=1, tt=2, tf=3, ft=4, ff=5
e2e_i32_with!(
    end_to_end_when_and_then_then_matches_and_truth_table,
    short_circuit_options(),
    "
PROGRAM main
  VAR
    a : BOOL := TRUE;
    b : BOOL := FALSE;
    tt : BOOL;
    tf : BOOL;
    ft : BOOL;
    ff : BOOL;
  END_VAR
  tt := a AND_THEN a;
  tf := a AND_THEN b;
  ft := b AND_THEN a;
  ff := b AND_THEN b;
END_PROGRAM
",
    &[(2, 1), (3, 0), (4, 0), (5, 0)],
);

// var layout: a=0, b=1, tt=2, tf=3, ft=4, ff=5
e2e_i32_with!(
    end_to_end_when_or_else_then_matches_or_truth_table,
    short_circuit_options(),
    "
PROGRAM main
  VAR
    a : BOOL := TRUE;
    b : BOOL := FALSE;
    tt : BOOL;
    tf : BOOL;
    ft : BOOL;
    ff : BOOL;
  END_VAR
  tt := a OR_ELSE a;
  tf := a OR_ELSE b;
  ft := b OR_ELSE a;
  ff := b OR_ELSE b;
END_PROGRAM
",
    &[(2, 1), (3, 1), (4, 1), (5, 0)],
);

// The motivating case from the design document: the right operand dereferences
// a null reference, so eager evaluation traps. Reaching the end of the scan
// with guarded = FALSE is the proof that the right operand never ran.
// var layout: r=0, guarded=1
e2e_i32_with!(
    end_to_end_when_and_then_guards_null_deref_then_right_operand_not_evaluated,
    short_circuit_ref_options(),
    "
PROGRAM main
  VAR
    r : REF_TO INT := NULL;
    guarded : BOOL;
  END_VAR
  guarded := r <> NULL AND_THEN r^ = 99;
END_PROGRAM
",
    &[(1, 0)],
);

#[test]
fn end_to_end_when_eager_and_guards_null_deref_then_traps() {
    // The control for the test above: with plain AND the right operand is
    // evaluated regardless, so the same guard crashes. This is the behaviour
    // AND_THEN exists to avoid, and what codegen would have produced had it
    // lowered AND_THEN eagerly.
    let source = "
PROGRAM main
  VAR
    r : REF_TO INT := NULL;
    guarded : BOOL;
  END_VAR
  guarded := r <> NULL AND r^ = 99;
END_PROGRAM
";
    let err = parse_and_try_run(source, &short_circuit_ref_options()).unwrap_err();
    assert_eq!(err.trap, Trap::NullDereference);
}

// The dual: OR_ELSE stops at a TRUE left operand, so the dereference is
// likewise never reached.
// var layout: r=0, guarded=1
e2e_i32_with!(
    end_to_end_when_or_else_guards_null_deref_then_right_operand_not_evaluated,
    short_circuit_ref_options(),
    "
PROGRAM main
  VAR
    r : REF_TO INT := NULL;
    guarded : BOOL;
  END_VAR
  guarded := r = NULL OR_ELSE r^ = 99;
END_PROGRAM
",
    &[(1, 1)],
);

// When the left operand does not decide the answer, the right operand runs --
// including the dereference the guard was protecting.
// var layout: target=0, r=1, guarded=2
e2e_i32_with!(
    end_to_end_when_and_then_guard_passes_then_right_operand_evaluated,
    short_circuit_ref_options(),
    "
PROGRAM main
  VAR
    target : INT := 99;
    r : REF_TO INT := REF(target);
    guarded : BOOL;
  END_VAR
  guarded := r <> NULL AND_THEN r^ = 99;
END_PROGRAM
",
    &[(2, 1)],
);

// var layout: x=0, taken=1
e2e_i32_with!(
    end_to_end_when_and_then_is_if_condition_then_branches_on_short_circuit_result,
    short_circuit_options(),
    "
PROGRAM main
  VAR
    x : DINT := 10;
    taken : DINT := 0;
  END_VAR
  IF x > 0 AND_THEN x < 100 THEN
    taken := 1;
  END_IF;
  IF x < 0 AND_THEN x < 100 THEN
    taken := taken + 10;
  END_IF;
END_PROGRAM
",
    &[(1, 1)],
);

// OR_ELSE binds at OR precedence and AND_THEN at AND precedence, so this is
// `a OR_ELSE (b AND_THEN c)`: TRUE regardless of b and c.
// var layout: a=0, b=1, c=2, result=3
e2e_i32_with!(
    end_to_end_when_short_circuit_operators_nested_then_precedence_holds,
    short_circuit_options(),
    "
PROGRAM main
  VAR
    a : BOOL := TRUE;
    b : BOOL := FALSE;
    c : BOOL := FALSE;
    result : BOOL;
  END_VAR
  result := a OR_ELSE b AND_THEN c;
END_PROGRAM
",
    &[(3, 1)],
);

// Nesting a short-circuit expression inside another one exercises the branch
// merge the emitter has to keep the operand stack balanced across.
// var layout: a=0, b=1, c=2, d=3, result=4
e2e_i32_with!(
    end_to_end_when_short_circuit_operands_are_short_circuits_then_stack_balances,
    short_circuit_options(),
    "
PROGRAM main
  VAR
    a : BOOL := TRUE;
    b : BOOL := TRUE;
    c : BOOL := FALSE;
    d : BOOL := TRUE;
    result : BOOL;
  END_VAR
  result := (a AND_THEN b) OR_ELSE (c AND_THEN d);
END_PROGRAM
",
    &[(4, 1)],
);

#[test]
fn end_to_end_when_short_circuit_repeats_across_rounds_then_stack_does_not_grow() {
    // The operand stack is never truncated between scan rounds, so a slot
    // leaked by an unbalanced branch would accumulate until it overflows.
    // Running the same short-circuit expression many times catches that.
    let source = "
PROGRAM main
  VAR
    a : BOOL := FALSE;
    b : BOOL := TRUE;
    result : BOOL;
  END_VAR
  result := a AND_THEN b;
  result := b OR_ELSE a;
END_PROGRAM
";
    crate::common::parse_and_run_rounds(source, &short_circuit_options(), |vm| {
        for round in 0..64 {
            vm.run_round(round).unwrap();
        }
    });
}

#[test]
fn end_to_end_when_function_returns_short_circuit_then_returns_bool() {
    // A function body's return value goes through the same branch merge, and
    // its calling convention requires exactly one slot on RET.
    let source = "
FUNCTION IN_RANGE : BOOL
  VAR_INPUT
    value : DINT;
  END_VAR
  IN_RANGE := value > 0 AND_THEN value < 100;
END_FUNCTION

PROGRAM main
  VAR
    inside : BOOL;
    outside : BOOL;
  END_VAR
  inside := IN_RANGE(10);
  outside := IN_RANGE(-1);
END_PROGRAM
";
    let (_container, bufs) = parse_and_run(source, &short_circuit_options());

    assert_eq!(bufs.vars[0].as_i32(), 1, "IN_RANGE(10)");
    assert_eq!(bufs.vars[1].as_i32(), 0, "IN_RANGE(-1)");
}
