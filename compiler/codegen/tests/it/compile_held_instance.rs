//! Tests of the instance that a field of a function block type holds,
//! compiled from the lowered program (REQ-LOW-codegen-196): a copy from such
//! a field, `f := g.ob` or `g(ob => f)`, to one, `g.ob := f` or
//! `g(inner := f)`, a copy of an instance whose fields hold instances, and a
//! copy between such fields in a block's body or method, `ob := inp`.
//!
//! Each copy gives the destination its own copy of the instance's fields, so
//! the destination is no alias of the instance the field holds.

use crate::common::Duration;
use crate::compile_body::compile_and_run;

/// Blocks whose fields hold instances, of a user-defined block and of a
/// standard one, and a block whose field holds an instance that holds them
/// in turn. No body reads the instances its fields hold, which this backend
/// does not compile yet (issue 1553).
const BLOCKS: &str = "
    FUNCTION_BLOCK INNER
      VAR_INPUT v : DINT; END_VAR
      VAR_OUTPUT q : DINT; END_VAR
      q := v;
    END_FUNCTION_BLOCK
    FUNCTION_BLOCK OUTER
      VAR_INPUT inner : INNER; timer : TON; d : DINT; END_VAR
      VAR_OUTPUT ob : INNER; ot : TON; r : DINT; END_VAR
      r := d + 1;
    END_FUNCTION_BLOCK
    FUNCTION_BLOCK HOLDER
      VAR_OUTPUT og : OUTER; END_VAR
    END_FUNCTION_BLOCK";

/// An output that holds an instance, stored by the call or read after it,
/// gives the target a copy of the instance the output holds: a later call
/// of the target changes neither that instance nor any other, as `first`,
/// laid out first, shows.
#[test]
fn run_when_instance_stored_from_output_then_copy_of_held_instance() {
    let snapshot = compile_and_run(&format!(
        "{BLOCKS}
         PROGRAM main
           VAR first : INNER; g : OUTER; f : INNER; f2 : INNER; f3 : INNER;
               k : DINT; fq : DINT; q2 : DINT; q3 : DINT; END_VAR
           first(v := 100);
           g(d := 1, ob => f);
           f(v := 5);
           f2 := g.ob;
           f3(v := 8);
           f3 := g.ob;
           k := first.q;
           fq := f.q;
           q2 := f2.q;
           q3 := f3.q;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("k"), 100);
    assert_eq!(snapshot.read_as::<i32>("fq"), 5);
    assert_eq!(snapshot.read_as::<i32>("q2"), 0);
    assert_eq!(snapshot.read_as::<i32>("q3"), 0);
}

/// A standard block's instance that an output holds is copied with every
/// field, so a later call of the target leaves the block's own fields as
/// they were.
#[test]
fn run_when_standard_instance_stored_from_output_then_copy_of_held_instance() {
    let snapshot = compile_and_run(&format!(
        "{BLOCKS}
         PROGRAM main
           VAR g : OUTER; t : TON; t2 : TON; r : DINT; pt : TIME; pt2 : TIME; END_VAR
           g(d := 1, ot => t, r => r);
           t(IN := TRUE, PT := T#1s);
           t2 := g.ot;
           pt := t.PT;
           pt2 := t2.PT;
           r := g.r;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("r"), 2);
    assert_eq!(snapshot.read("pt"), Duration::milliseconds(1000));
    assert_eq!(snapshot.read("pt2"), Duration::milliseconds(0));
}

/// An instance stored in a field that holds one, by a call's input or
/// outside a call, is copied into the instance the field holds: a later
/// call of the source leaves that copy as it was.
#[test]
fn run_when_instance_stored_in_field_then_field_holds_copy() {
    let snapshot = compile_and_run(&format!(
        "{BLOCKS}
         PROGRAM main
           VAR f : INNER; g : OUTER; t : TON; f2 : INNER; f3 : INNER;
               r : DINT; q : DINT; q2 : DINT; q3 : DINT; END_VAR
           f(v := 7);
           g(inner := f, timer := t, d := 4, r => r);
           g.ob := f;
           f(v := 9);
           f2 := g.ob;
           f3 := g.inner;
           q := f.q;
           q2 := f2.q;
           q3 := f3.q;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("r"), 5);
    assert_eq!(snapshot.read_as::<i32>("q"), 9);
    assert_eq!(snapshot.read_as::<i32>("q2"), 7);
    assert_eq!(snapshot.read_as::<i32>("q3"), 7);
}

/// A copy of an instance whose field holds an instance copies that instance
/// too, to any depth, into storage of the destination's own: a later store
/// to the source's leaves the destination's as it was.
#[test]
fn run_when_instance_holding_instances_copied_then_held_instances_copied() {
    let snapshot = compile_and_run(&format!(
        "{BLOCKS}
         PROGRAM main
           VAR f : INNER; h : HOLDER; h2 : HOLDER; f2 : INNER; f3 : INNER;
               q2 : DINT; q3 : DINT; END_VAR
           f(v := 7);
           h.og.ob := f;
           h2 := h;
           f(v := 9);
           h.og.ob := f;
           f2 := h2.og.ob;
           f3 := h.og.ob;
           q2 := f2.q;
           q3 := f3.q;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("q2"), 7);
    assert_eq!(snapshot.read_as::<i32>("q3"), 9);
}

/// A copy in a block's body or method between fields that hold instances,
/// `ob := inp`, copies the instance `inp` holds into the one `ob` holds: a
/// later store to `inp` from outside the block leaves what `ob` holds as it
/// was, and no other instance changes.
#[test]
fn run_when_body_copies_between_held_fields_then_copy_of_held_instance() {
    let snapshot = compile_and_run(&format!(
        "{BLOCKS}
         FUNCTION_BLOCK RELAY
           VAR_INPUT inp : INNER; END_VAR
           VAR_OUTPUT ob : INNER; END_VAR
           ob := inp;
           METHOD Keep : DINT
             ob := inp;
             Keep := 1;
           END_METHOD
         END_FUNCTION_BLOCK
         PROGRAM main
           VAR first : INNER; f : INNER; k : INNER; g : RELAY; g2 : RELAY;
               h : INNER; h2 : INNER; kept : DINT;
               firstq : DINT; q : DINT; q2 : DINT; END_VAR
           first(v := 100);
           f(v := 7);
           k(v := 1);
           g(inp := f);
           g.inp := k;
           h := g.ob;
           g2.inp := f;
           kept := g2.Keep();
           g2.inp := k;
           h2 := g2.ob;
           firstq := first.q;
           q := h.q;
           q2 := h2.q;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("q"), 7);
    assert_eq!(snapshot.read_as::<i32>("q2"), 7);
    assert_eq!(snapshot.read_as::<i32>("firstq"), 100);
}
