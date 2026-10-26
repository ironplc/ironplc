//! Tests of calls of function blocks, user-defined and standard, compiled
//! from the lowered program (REQ-LOW-lowering-069), and one scan of each
//! program checked to leave every variable with the same values on every
//! backend.

use ironplc_parser::options::CompilerOptions;

use rstest::rstest;

use crate::common::{
    compile_analyzed, parse, parse_and_compile_to_run, run_compiled_scans, Duration, Snapshot,
};
use crate::compile_body::{assert_compiles, compile_and_run, compile_and_run_agreeing, options};

/// A block whose outputs are its inputs' sum, whether the sum is positive,
/// a `REAL`, a `UDINT` and a count of its calls.
const BLOCK: &str = "
    FUNCTION_BLOCK FB
      VAR_INPUT x : DINT; s : SINT; u : UDINT; END_VAR
      VAR_OUTPUT q : DINT; done : BOOL; r : REAL; qu : UDINT; calls : DINT; END_VAR
      q := x + s;
      done := q > 0;
      r := 1.5;
      qu := u;
      calls := calls + 1;
    END_FUNCTION_BLOCK";

/// REQ-LOW-lowering-069: a call stores its inputs, in the order the block
/// declares them, runs the block, and stores each output in its target: a
/// negated one inverted, one of a narrower type converted to its target's,
/// and one in an element of an array at a literal subscript.
#[test]
fn run_when_block_called_then_inputs_stored_and_outputs_read() {
    let snapshot = compile_and_run_agreeing(&format!(
        "{BLOCK}
         PROGRAM main
           VAR f : FB; a : DINT := 40; b : BOOL := TRUE; lr : LREAL; lu : LINT;
               arr : ARRAY[1..2] OF DINT; element : DINT; END_VAR
           f(u := 4000000000, s := 2, x := a, q => a, NOT done => b, r => lr, qu => lu,
             calls => arr[2]);
           element := arr[2];
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("a"), 42);
    assert_eq!(snapshot.read_as::<i32>("b"), 0);
    assert_eq!(snapshot.read_as::<f64>("lr"), 1.5);
    assert_eq!(snapshot.read_as::<i64>("lu"), 4_000_000_000);
    assert_eq!(snapshot.read_as::<i32>("element"), 1);
}

/// REQ-LOW-lowering-069: an input a call omits keeps the value the previous
/// call stored, and the instance keeps its fields from one call to the next.
#[test]
fn run_when_block_called_twice_then_omitted_input_and_fields_kept() {
    let snapshot = compile_and_run_agreeing(&format!(
        "{BLOCK}
         PROGRAM main VAR f : FB; first : DINT; second : DINT; n : DINT; END_VAR
           f(x := 5, s := 1, q => first);
           f(s := -3, q => second, calls => n);
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("first"), 6);
    assert_eq!(snapshot.read_as::<i32>("second"), 2);
    assert_eq!(snapshot.read_as::<i32>("n"), 2);
}

/// A call inside an `IF` and a `FOR` runs each time its statement does, and
/// two instances of one block keep separate fields.
#[test]
fn run_when_block_called_in_loop_and_if_then_runs_each_time() {
    let snapshot = compile_and_run_agreeing(&format!(
        "{BLOCK}
         PROGRAM main
           VAR f : FB; g : FB; i : DINT; total : DINT; skipped : DINT; fc : DINT; gc : DINT;
           END_VAR
           FOR i := 1 TO 4 DO
             f(x := i, q => total);
           END_FOR;
           IF total > 100 THEN
             g(x := 1, calls => skipped);
           END_IF;
           fc := f.calls;
           gc := g.calls;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("total"), 4);
    assert_eq!(snapshot.read_as::<i32>("fc"), 4);
    assert_eq!(snapshot.read_as::<i32>("gc"), 0);
    assert_eq!(snapshot.read_as::<i32>("skipped"), 0);
}

/// REQ-LOW-lowering-046: an input is stored in its field as `f.s := v + 100`
/// stores it, narrowed to the field's storage (#2292).
#[test]
fn run_when_input_outside_narrow_field_then_narrowed() {
    let snapshot = compile_and_run_agreeing(
        "FUNCTION_BLOCK Narrow
           VAR_INPUT s : SINT; u : USINT; END_VAR
           VAR_OUTPUT q : DINT; qu : DINT; END_VAR
           q := s;
           qu := u;
         END_FUNCTION_BLOCK
         PROGRAM main
           VAR f : Narrow; v : SINT := 100; w : USINT := 200; r : DINT; ru : DINT; END_VAR
           f(s := v + 100, u := w + 100, q => r, qu => ru);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("r"), -56);
    assert_eq!(snapshot.read_as::<i32>("ru"), 44);
}

/// A call lowers and compiles: the instance pushed once, each input narrowed
/// and stored in its field, the call, then each output stored. A call inside
/// an `IF` too.
#[rstest]
#[case::inputs_and_outputs("f(s := 2, x := d, q => d, NOT done => b, r => lr);")]
#[case::narrowed_input("f(s := n + 1);")]
#[case::no_arguments("f();")]
#[case::outputs_only("f(q => d, qu => lu);")]
#[case::inside_if("IF b THEN f(x := d, q => d); END_IF;")]
#[case::two_calls_in_a_loop("FOR d := 1 TO 3 DO f(x := d); g(x := d, q => n2); END_FOR;")]
fn compile_when_block_called_then_compiles(#[case] statement: &str) {
    assert_compiles(&format!(
        "{BLOCK}
         PROGRAM main
           VAR f : FB; g : FB; d : DINT; n : SINT; n2 : DINT; b : BOOL; lr : LREAL; lu : LINT;
           END_VAR
           {statement}
         END_PROGRAM"
    ));
}

/// A call of an instance declared inside a function block is not supported
/// yet (P9999, #1553).
#[test]
fn compile_when_instance_declared_in_block_then_not_implemented() {
    assert_not_implemented(&format!(
        "{BLOCK}
         FUNCTION_BLOCK Outer
           VAR inner : FB; n : DINT; END_VAR
           inner(x := n);
         END_FUNCTION_BLOCK
         PROGRAM main VAR o : Outer; END_VAR
           o();
         END_PROGRAM"
    ));
}

/// Fails unless compiling `source` fails with P9999.
fn assert_not_implemented(source: &str) {
    let options = CompilerOptions::default();
    let (library, context) = parse(source, &options);
    let code = compile_analyzed(&library, &context, &options)
        .err()
        .map(|diagnostic| diagnostic.code);
    assert_eq!(code, Some("P9999".to_string()));
}

/// The variables of a program that calls standard function blocks: one of
/// each type a field of one takes.
const STANDARD_VARS: &str = "b : BOOL; nb : BOOL; i : INT; n : INT; d : DINT; l : LINT;
    u : UDINT; ul : ULINT; et : TIME;";

/// A call of a standard function block lowers and compiles: a timer, a
/// counter at two widths, an edge detector with a negated output, a
/// bistable, an input narrowed to its field, a call written with positional
/// inputs, which the analyzer writes in normal form, and calls inside an `IF`
/// and a `FOR`. So does a read of a
/// counter's field, stored narrowed to its target as an output is.
#[rstest]
#[case::timer("t : TON;", "t(IN := b, PT := T#5S, Q => b, ET => et);")]
#[case::counter_int("c : CTU;", "c(CU := b, PV := i, Q => b, CV => i);")]
#[case::counter_lint("c : CTU_LINT;", "c(CU := b, R := FALSE, PV := l, CV => l);")]
#[case::edge_detector("e : R_TRIG;", "e(CLK := b, NOT Q => nb);")]
#[case::bistable("s : SR;", "s(S1 := b, R := NOT b, Q1 => b);")]
#[case::narrowed_input("c : CTU;", "c(CU := b, PV := i + n);")]
#[case::positional("c : CTU;", "c(TRUE, FALSE, 1);")]
#[case::inside_if("t : TOF;", "IF b THEN t(IN := nb, PT := et, Q => b); END_IF;")]
#[case::field_read("c : CTU;", "i := c.CV;")]
#[case::inside_for(
    "c : CTD_DINT;",
    "FOR d := 1 TO 3 DO c(CD := b, PV := d, CV => d); END_FOR;"
)]
fn compile_when_standard_block_called_then_compiles(
    #[case] instance: &str,
    #[case] statement: &str,
) {
    assert_compiles(&format!(
        "PROGRAM main
           VAR {instance} {STANDARD_VARS} END_VAR
           {statement}
         END_PROGRAM"
    ));
}

/// Every input and output of every timer, edge detector and bistable is a
/// field of the VM's block, which this backend stores and reads.
#[rstest]
#[case::ton("TON", "f(IN := b, PT := et, Q => b, ET => et);")]
#[case::tof("TOF", "f(IN := b, PT := et, Q => b, ET => et);")]
#[case::tp("TP", "f(IN := b, PT := et, Q => b, ET => et);")]
#[case::r_trig("R_TRIG", "f(CLK := b, Q => b);")]
#[case::f_trig("F_TRIG", "f(CLK := b, Q => b);")]
#[case::sr("SR", "f(S1 := b, R := b, Q1 => b);")]
#[case::rs("RS", "f(S := b, R1 := b, Q1 => b);")]
fn compile_when_every_input_and_output_bound_then_compiles(
    #[case] block: &str,
    #[case] statement: &str,
) {
    assert_compiles(&format!(
        "PROGRAM main
           VAR f : {block}; {STANDARD_VARS} END_VAR
           {statement}
         END_PROGRAM"
    ));
}

/// Every input and output of every counter, at every width, is a field of
/// the VM's block for that width, which this backend stores and reads. `{v}`
/// stands for a variable of the width.
#[rstest]
fn compile_when_every_counter_input_and_output_bound_then_compiles(
    #[values(
        ("CTU", "f(CU := b, R := b, PV := {v}, Q => b, CV => {v});"),
        ("CTD", "f(CD := b, LD := b, PV := {v}, Q => b, CV => {v});"),
        ("CTUD", "f(CU := b, CD := b, R := b, LD := b, PV := {v}, QU => b, QD => b, CV => {v});")
    )]
    counter: (&str, &str),
    #[values(("", "i"), ("_DINT", "d"), ("_LINT", "l"), ("_UDINT", "u"), ("_ULINT", "ul"))] width: (
        &str,
        &str,
    ),
) {
    let (block, statement) = counter;
    let (suffix, variable) = width;
    assert_compiles(&format!(
        "PROGRAM main
           VAR f : {block}{suffix}; {STANDARD_VARS} END_VAR
           {}
         END_PROGRAM",
        statement.replace("{v}", variable)
    ));
}

/// One scan of calls of standard function blocks leaves the outputs the VM's
/// blocks give: a counter counts its first rising edge, a down counter
/// loads its preset, an up-down counter counts up at its own width, an edge
/// detector sees its first rising edge, and each bistable keeps its dominant
/// input.
#[test]
fn run_when_standard_blocks_called_then_outputs_of_vm_blocks() {
    let snapshot = compile_and_run_agreeing(
        "PROGRAM main
           VAR c : CTU_DINT; down : CTD; ud : CTUD_ULINT; e : R_TRIG; s : SR; r : RS;
               cv : DINT; cq : BOOL; dv : INT; dq : BOOL; udv : ULINT; uq : BOOL;
               eq : BOOL; sq : BOOL; rq : BOOL; END_VAR
           c(CU := TRUE, PV := 1, Q => cq, CV => cv);
           down(CD := FALSE, LD := TRUE, PV := 3, Q => dq, CV => dv);
           ud(CU := TRUE, PV := 2, QU => uq, CV => udv);
           e(CLK := TRUE, Q => eq);
           s(S1 := TRUE, R := TRUE, Q1 => sq);
           r(S := TRUE, R1 := TRUE, Q1 => rq);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("cv"), 1);
    assert_eq!(snapshot.read_as::<i32>("cq"), 1);
    assert_eq!(snapshot.read_as::<i32>("dv"), 3);
    assert_eq!(snapshot.read_as::<i32>("dq"), 0);
    assert_eq!(snapshot.read_as::<u64>("udv"), 1);
    assert_eq!(snapshot.read_as::<i32>("uq"), 0);
    assert_eq!(snapshot.read_as::<i32>("eq"), 1);
    assert_eq!(snapshot.read_as::<i32>("sq"), 1);
    assert_eq!(snapshot.read_as::<i32>("rq"), 0);
}

/// REQ-LOW-lowering-046: a standard block's input is narrowed to its field
/// once: `PV` of a `CTU` is an `INT`, so `a + b`, 40000, is stored as
/// -25536.
#[test]
fn run_when_standard_block_input_outside_field_then_narrowed_once() {
    let snapshot = compile_and_run_agreeing(
        "PROGRAM main
           VAR c : CTU; a : INT := 20000; b : INT := 20000; preset : DINT; END_VAR
           c(CU := FALSE, PV := a + b);
           preset := c.PV;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("preset"), -25536);
}

/// The VM's block counts the `CV` of an `_INT` counter in 32 bits, past the
/// range of `INT`, and a store of it in an `INT`, by an output or by a read
/// of the field, narrows it to the target.
#[test]
fn run_when_int_counter_counts_past_int_then_stored_narrowed() {
    let snapshot = compile_and_run_agreeing(
        "PROGRAM main VAR d : CTD; out : INT; read : INT; END_VAR
           d(CD := FALSE, LD := TRUE, PV := -32768);
           d(CD := TRUE, LD := FALSE, CV => out);
           read := d.CV;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("out"), 32767);
    assert_eq!(snapshot.read_as::<i32>("read"), 32767);
}

/// A call with positional inputs stores each in the field the analyzer
/// names for it.
#[test]
fn run_when_standard_block_called_positionally_then_inputs_stored() {
    let snapshot = compile_and_run_agreeing(
        "PROGRAM main
           VAR c : CTU; cv : INT; pv : INT; q : BOOL; END_VAR
           c(TRUE, FALSE, 1);
           cv := c.CV;
           pv := c.PV;
           q := c.Q;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("cv"), 1);
    assert_eq!(snapshot.read_as::<i32>("pv"), 1);
    assert_eq!(snapshot.read_as::<i32>("q"), 1);
}

/// A timer keeps its state from one round to the next, and reads each
/// round's time: its output and elapsed time after each of several rounds.
#[test]
fn run_when_timer_called_over_rounds_then_outputs_follow_time() {
    let source = "PROGRAM main
           VAR t : TON; done : BOOL; elapsed : TIME; END_VAR
           t(IN := TRUE, PT := T#5S, Q => done, ET => elapsed);
         END_PROGRAM";
    let options = options();
    let (library, context) = parse(source, &options);
    let container = compile_analyzed(&library, &context, &options).unwrap();
    let observed = run_compiled_scans(&container, |session| {
        [0, 2, 4, 6].map(|seconds| {
            session.scan(seconds * 1_000_000).unwrap();
            (
                session.read_as::<i32>("done"),
                session.read_as::<Duration>("elapsed"),
            )
        })
    });
    assert_eq!(
        observed,
        [
            (0, Duration::ZERO),
            (0, Duration::seconds(2)),
            (0, Duration::seconds(4)),
            (1, Duration::seconds(5)),
        ]
    );
}

/// A call of a standard block's instance declared inside a function block
/// is not supported yet (P9999, #1553).
#[test]
fn compile_when_standard_instance_declared_in_block_then_not_implemented() {
    assert_not_implemented(
        "FUNCTION_BLOCK Outer VAR t : TON; END_VAR t(IN := TRUE); END_FUNCTION_BLOCK
         PROGRAM main VAR o : Outer; END_VAR o(); END_PROGRAM",
    );
}

/// A call of a standard block's instance a program reaches through
/// `VAR_EXTERNAL` lowers and compiles.
#[test]
fn compile_when_standard_instance_external_then_compiles() {
    assert_compiles(
        "PROGRAM main
           VAR_EXTERNAL t : TON; END_VAR
           VAR q : BOOL; END_VAR
           t(IN := TRUE, PT := T#1S, Q => q);
         END_PROGRAM
         CONFIGURATION config
           VAR_GLOBAL t : TON; END_VAR
           RESOURCE res ON PLC
             TASK plc_task(INTERVAL := T#100MS, PRIORITY := 1);
             PROGRAM plc_task_instance WITH plc_task : main;
           END_RESOURCE
         END_CONFIGURATION",
    );
}

/// REQ-LOW-codegen-165: an instance assigned from another of its block gets
/// a copy of every field, in storage of its own, so a later call of either
/// leaves the other as it was, and an instance assigned to itself is
/// unchanged: `f(v := 9)` calls `f`, not `g`.
#[test]
fn run_when_instance_assigned_then_copy_of_its_fields() {
    let snapshot = compile_and_run(
        "FUNCTION_BLOCK FBV
           VAR_INPUT v : DINT; END_VAR
           VAR_OUTPUT q : DINT; calls : DINT; END_VAR
           q := v;
           calls := calls + 1;
         END_FUNCTION_BLOCK
         PROGRAM main
           VAR f : FBV; g : FBV; r : DINT; rf : DINT; gc : DINT; fc : DINT; END_VAR
           g(v := 3);
           f := g;
           f := f;
           f(v := 9);
           r := g.q;
           rf := f.q;
           gc := g.calls;
           fc := f.calls;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("r"), 3);
    assert_eq!(snapshot.read_as::<i32>("rf"), 9);
    assert_eq!(snapshot.read_as::<i32>("gc"), 1);
    assert_eq!(snapshot.read_as::<i32>("fc"), 2);
}

/// REQ-LOW-codegen-165: a standard block's instance assigned from another
/// gets the fields the VM's block keeps for itself as well as its inputs and
/// outputs. `t := u` while `u` times copies the time `u` started at, so `t`
/// has run as long as `u`; resetting `t` afterwards leaves `u` timing.
#[test]
fn run_when_timer_assigned_while_timing_then_copy_keeps_its_start() {
    let source = "PROGRAM main
           VAR u : TON; t : TON; scan : DINT; ue : TIME; te : TIME; END_VAR
           scan := scan + 1;
           u(IN := TRUE, PT := T#5S, ET => ue);
           IF scan = 2 THEN t := u; END_IF;
           IF scan >= 2 THEN t(IN := scan = 2, PT := T#5S, ET => te); END_IF;
         END_PROGRAM";
    let options = options();
    let (library, context) = parse(source, &options);
    let container = compile_analyzed(&library, &context, &options).expect("every statement lowers");
    let observed = run_compiled_scans(&container, |session| {
        [0, 2, 4, 6].map(|seconds| {
            session.scan(seconds * 1_000_000).unwrap();
            (
                session.read_as::<Duration>("ue"),
                session.read_as::<Duration>("te"),
            )
        })
    });
    assert_eq!(
        observed,
        [
            (Duration::ZERO, Duration::ZERO),
            (Duration::seconds(2), Duration::seconds(2)),
            (Duration::seconds(4), Duration::ZERO),
            (Duration::seconds(5), Duration::ZERO),
        ]
    );
}

/// A function's body sees the program's instances by name. Its own
/// structures named as two instances of one block are copied as structures,
/// not taken for the instances, which keep their fields: `r` is left as `a`
/// was called.
#[test]
fn run_when_function_copies_structures_named_as_instances_then_instances_kept() {
    let container = parse_and_compile_to_run(
        "TYPE S : STRUCT n : DINT; END_STRUCT; END_TYPE
         FUNCTION_BLOCK FBV
           VAR_INPUT v : DINT; END_VAR
         END_FUNCTION_BLOCK
         FUNCTION F : DINT
           VAR a : S; b : S; END_VAR
           a := b;
           F := 1;
         END_FUNCTION
         PROGRAM main
           VAR a : FBV; b : FBV; k : DINT; r : DINT; END_VAR
           a(v := 1);
           b(v := 2);
           k := F();
           r := a.v;
         END_PROGRAM",
        &options(),
    );
    assert_eq!(Snapshot::run_compiled(container).read_as::<i32>("r"), 1);
}

/// A block that refers to itself through a reference field, the usual
/// linked list, and two blocks that refer to each other.
const LINKED: &str = "
    FUNCTION_BLOCK Node
      VAR_INPUT next : REF_TO Node; v : DINT; END_VAR
      VAR_OUTPUT o : DINT; linked : BOOL; END_VAR
      o := v * 2;
      linked := next <> NULL;
    END_FUNCTION_BLOCK
    FUNCTION_BLOCK Ping VAR_INPUT peer : REF_TO Pong; END_VAR END_FUNCTION_BLOCK
    FUNCTION_BLOCK Pong VAR_INPUT peer : REF_TO Ping; END_VAR END_FUNCTION_BLOCK";

/// REQ-LOW-lowering-195: a block that refers to itself through a reference
/// field runs, as do calls passing it a reference to another instance, and
/// every backend leaves every variable the same.
#[test]
fn run_when_block_refers_to_itself_through_reference_then_values() {
    let snapshot = compile_and_run_agreeing(&format!(
        "{LINKED}
         PROGRAM main
           VAR a : Node; b : Node; pi : Ping; po : Pong;
               k : DINT; j : DINT; la : BOOL; lb : BOOL; END_VAR
           b(v := 3);
           a(next := REF(b), v := 1);
           pi(peer := REF(po));
           k := b.o;
           j := a.o;
           la := a.linked;
           lb := b.linked;
         END_PROGRAM"
    ));
    assert_eq!(snapshot.read_as::<i32>("k"), 6);
    assert_eq!(snapshot.read_as::<i32>("j"), 2);
    assert_eq!(snapshot.read_as::<i32>("la"), 1);
    assert_eq!(snapshot.read_as::<i32>("lb"), 0);
}

/// A call of a block that refers to itself lowers and compiles.
#[rstest]
#[case::call("b(v := 3);")]
#[case::call_with_reference("a(next := REF(b), v := 1);")]
#[case::output("k := b.o;")]
#[case::mutual("pi(peer := REF(po));")]
fn compile_when_block_refers_to_itself_through_reference_then_compiles(#[case] statement: &str) {
    assert_compiles(&format!(
        "{LINKED}
         PROGRAM main VAR a : Node; b : Node; pi : Ping; po : Pong; k : DINT; END_VAR
           {statement}
         END_PROGRAM"
    ));
}
