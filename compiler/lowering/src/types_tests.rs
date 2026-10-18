//! Tests of the types lowering gives the program's variables.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use ironplc_ir::lowered::{IntWidth, TypeKind};

use crate::test_support::lower;

/// REQ-LOW-lowering-190: an enumeration's underlying type is the integer
/// or bit string type its declaration names, and `INT` when it names none.
#[test]
fn lower_statement_when_enumeration_then_underlying_type_declared_or_int() {
    let outcome = lower(
        "TYPE SMALL : (A, B) WORD; END_TYPE
         TYPE PLAIN : (C, D); END_TYPE
         TYPE RENAMED : SMALL; END_TYPE
         FUNCTION F : DINT
           VAR s : SMALL; p : PLAIN; r : RENAMED; e : (X, Y); END_VAR
           s := A;
           p := C;
           r := B;
           e := Y;
           F := 0;
         END_FUNCTION",
        "F",
    );
    let underlying = |name: &str| {
        let variable = outcome
            .variables
            .iter()
            .find(|variable| variable.name().to_string() == name)
            .unwrap();
        let Some(TypeKind::Enumeration { underlying, .. }) = outcome.types.kind(variable.ty())
        else {
            panic!("{name} is not an enumeration");
        };
        outcome.types.kind(*underlying).cloned()
    };
    let word = |kind: Option<TypeKind>| {
        matches!(
            kind,
            Some(TypeKind::BitString {
                width: IntWidth::B16
            })
        )
    };
    let int = |kind: Option<TypeKind>| {
        matches!(
            kind,
            Some(TypeKind::Integer {
                width: IntWidth::B16,
                signed: true
            })
        )
    };
    assert!(word(underlying("s")));
    assert!(int(underlying("p")));
    assert!(word(underlying("r")));
    assert!(int(underlying("e")));
}
