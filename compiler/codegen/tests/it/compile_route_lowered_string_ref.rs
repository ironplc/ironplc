//! Tests of strings read and written through a reference, `rs^`, `rsa^[i]`
//! and a `REFERENCE TO STRING`, compiled from the lowered program, with
//! every statement required to lower (REQ-LOW-lowering-188,
//! REQ-LOW-codegen-189).
//!
//! The route that reads the AST reads such a string through `LOAD_INDIRECT`,
//! which does not follow the reference to a string, so these run with
//! `RequireLowered` only.

use ironplc_codegen::Route;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::error::Trap;
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::VmBuffers;

use crate::common::{compile_by_route, parse, Snapshot};
use crate::compile_route_lowered::{options, run_lowered};

/// A string a reference refers to, read whole, by `LEN`, as an argument of
/// a standard function and in a comparison, and an element of an array of
/// strings a reference refers to, read the string the reference reaches
/// when they are read.
#[test]
fn run_when_string_read_through_reference_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR s : STRING := 'abcd'; w : WSTRING := \"xyz\";
               sa : ARRAY[1..2] OF STRING := ['xy', 'pqr']; i : DINT := 1;
               rs : REF_TO STRING; rw : REF_TO WSTRING;
               rsa : REF_TO ARRAY[1..2] OF STRING;
               t : STRING; u : STRING; v : STRING; e : STRING; f : STRING;
               later : STRING; x : WSTRING;
               n : DINT; m : DINT; same : BOOL; END_VAR
           rs := REF(s);
           rw := REF(w);
           rsa := REF(sa);
           t := rs^;
           n := LEN(rs^);
           u := CONCAT(rs^, 'ef');
           same := rs^ = 'abcd';
           v := REF(s)^;
           x := rw^;
           e := rsa^[2];
           f := rsa^[i];
           m := LEN(rsa^[2]);
           s := 'zz';
           later := rs^;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read("t"), "abcd");
    assert_eq!(snapshot.read_as::<i32>("n"), 4);
    assert_eq!(snapshot.read("u"), "abcdef");
    assert_eq!(snapshot.read_as::<i32>("same"), 1);
    assert_eq!(snapshot.read("v"), "abcd");
    assert_eq!(snapshot.read("x"), "xyz");
    assert_eq!(snapshot.read("e"), "pqr");
    assert_eq!(snapshot.read("f"), "xy");
    assert_eq!(snapshot.read_as::<i32>("m"), 3);
    assert_eq!(snapshot.read("later"), "zz");
}

/// A store through a reference writes the string the reference refers to,
/// or the element of the array of strings it refers to, keeping the code
/// units the string's capacity holds (REQ-LOW-codegen-056), and leaves the
/// other strings as they were.
#[test]
fn run_when_string_stored_through_reference_then_referenced_string_holds_it() {
    let snapshot = run_lowered(
        "TYPE Short : STRING[3]; END_TYPE
         PROGRAM main
           VAR s : STRING; k : Short; w : WSTRING; sa : ARRAY[1..3] OF STRING[4];
               i : DINT := 3; other : STRING := 'same';
               rs : REF_TO STRING; rk : REF_TO Short; rw : REF_TO WSTRING;
               rsa : REF_TO ARRAY[1..3] OF STRING[4]; a1 : STRING; a2 : STRING;
               a3 : STRING; END_VAR
           rs := REF(s);
           rk := REF(k);
           rw := REF(w);
           rsa := REF(sa);
           rs^ := 'xyz';
           rs^ := CONCAT(rs^, '!');
           rk^ := 'abcdef';
           rw^ := \"π\";
           rsa^[1] := 'one';
           rsa^[i] := 'three';
           a1 := sa[1];
           a2 := sa[2];
           a3 := sa[3];
         END_PROGRAM",
    );
    assert_eq!(snapshot.read("s"), "xyz!");
    assert_eq!(snapshot.read("k"), "abc");
    assert_eq!(snapshot.read("w"), "π");
    assert_eq!(snapshot.read("other"), "same");
    assert_eq!(snapshot.read("a1"), "one");
    assert_eq!(snapshot.read("a2"), "");
    assert_eq!(snapshot.read("a3"), "thre");
}

/// A `REFERENCE TO STRING` names the string it is bound to, read and
/// written as that string.
#[test]
fn run_when_reference_to_string_read_and_written_then_bound_string() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR s : STRING := 'abc'; r : REFERENCE TO STRING; t : STRING; n : DINT; END_VAR
           r REF= s;
           t := r;
           n := LEN(r);
           r := 'xy';
         END_PROGRAM",
    );
    assert_eq!(snapshot.read("t"), "abc");
    assert_eq!(snapshot.read_as::<i32>("n"), 3);
    assert_eq!(snapshot.read("s"), "xy");
}

/// A reference a declaration starts with refers to its string as one a
/// statement makes does, and two references to the same string are equal.
#[test]
fn run_when_reference_starts_referring_to_string_then_reads_it() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR first : STRING := 'first'; s : STRING := 'abc';
               rs : REF_TO STRING := REF(s); t : STRING; same : BOOL; END_VAR
           t := rs^;
           same := rs = REF(s);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read("t"), "abc");
    assert_eq!(snapshot.read_as::<i32>("same"), 1);
}

/// A function's and a function block's reference to a string reach the
/// string they are passed, to read it and to write it, and a function's
/// reference to its own string reaches that.
#[test]
fn run_when_body_reads_and_writes_string_through_reference_then_values() {
    let snapshot = run_lowered(
        "FUNCTION F : DINT VAR_INPUT p : REF_TO STRING; END_VAR
           F := LEN(p^);
           p^ := 'set by F';
         END_FUNCTION
         FUNCTION G : DINT VAR l : STRING := 'local'; r : REF_TO STRING; END_VAR
           r := REF(l);
           G := LEN(r^);
         END_FUNCTION
         FUNCTION_BLOCK B VAR_INPUT p : REF_TO STRING; END_VAR VAR_OUTPUT n : DINT; END_VAR
           n := LEN(p^);
         END_FUNCTION_BLOCK
         PROGRAM main
           VAR s : STRING := 'abc'; b : STRING := 'block'; k : DINT; g : DINT; f : B;
               n : DINT; END_VAR
           k := F(p := REF(s));
           g := G();
           f(p := REF(b));
           n := f.n;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("k"), 3);
    assert_eq!(snapshot.read("s"), "set by F");
    assert_eq!(snapshot.read_as::<i32>("g"), 5);
    assert_eq!(snapshot.read_as::<i32>("n"), 5);
}

/// `ADR(s)^`, which the analyzer makes `REF(s)^`, is `s`, and a pointer
/// `ADR` makes reaches `s` as a reference does.
#[test]
fn run_when_adr_of_string_dereferenced_then_value_of_string() {
    let options = CompilerOptions {
        allow_pointer_to: true,
        allow_adr: true,
        ..options()
    };
    let (library, context) = parse(
        "PROGRAM main
           VAR s : STRING := 'abc'; p : POINTER TO STRING; t : STRING; u : STRING; END_VAR
           t := ADR(s)^;
           p := ADR(s);
           u := p^;
         END_PROGRAM",
        &options,
    );
    let container = compile_by_route(&library, &context, &options, Route::RequireLowered)
        .expect("every statement lowers");
    let snapshot = Snapshot::run_compiled(container);
    assert_eq!(snapshot.read("t"), "abc");
    assert_eq!(snapshot.read("u"), "abc");
}

/// A string read through a null reference traps, as any value read through
/// one does.
#[test]
fn run_when_string_read_through_null_reference_then_null_dereference() {
    let options = options();
    let (library, context) = parse(
        "PROGRAM main VAR rs : REF_TO STRING; t : STRING; END_VAR t := rs^; END_PROGRAM",
        &options,
    );
    let container = compile_by_route(&library, &context, &options, Route::RequireLowered)
        .expect("every statement lowers");
    let mut bufs = VmBuffers::from_container(&container);
    let fault = load_and_start(&container, &mut bufs)
        .and_then(|mut vm| vm.run_round(0))
        .unwrap_err();
    assert_eq!(fault.trap, Trap::NullDereference);
}
