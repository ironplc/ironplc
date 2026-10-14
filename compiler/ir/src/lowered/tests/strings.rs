//! Tests of string expressions and the `AssignStr` statement.

use super::*;

/// The fixture's program with two `STRING[10]` variables and a
/// `WSTRING[10]` one.
fn strings(f: &mut Fixture) -> (VarId, VarId, VarId) {
    let string = |b: &mut ProgramBuilder, encoding| {
        let shape = StringShape {
            encoding,
            capacity: 10,
        };
        b.push_type(entry(TypeKind::String(shape)), &span())
            .unwrap()
    };
    let narrow = string(&mut f.b, Encoding::Narrow);
    let wide = string(&mut f.b, Encoding::Wide);
    let Owner::Pou(main) = f.b.variables().get(f.dint).unwrap().owner() else {
        panic!("the fixture's variables belong to main");
    };
    let mut var = |text: &str, ty| f.b.push_variable(name(text), Owner::Pou(main), ty).unwrap();
    (var("s", narrow), var("t", narrow), var("w", wide))
}

fn literal(text: &str, encoding: Encoding) -> StrExpr {
    StrExpr::literal(text.chars().collect(), encoding, span()).unwrap()
}

/// A string literal or the string at a place is stored at a place of a
/// string of its encoding. A literal's capacity is its length.
#[test]
fn assign_str_when_value_of_place_encoding_then_assign_str() {
    let mut f = fixture();
    let (s, t, w) = strings(&mut f);
    let from_literal =
        f.b.assign_str(f.place(s), literal("abc", Encoding::Narrow), span())
            .unwrap();
    assert_eq!(
        from_literal.to_string(),
        "assign-str v8 (literal string[3] \"abc\")"
    );
    let read = f.b.read_str(f.place(t), span()).unwrap();
    let from_place = f.b.assign_str(f.place(s), read, span()).unwrap();
    assert_eq!(from_place.to_string(), "assign-str v8 (read string[10] v9)");
    let wide =
        f.b.assign_str(f.place(w), literal("€", Encoding::Wide), span())
            .unwrap();
    assert_eq!(
        wide.to_string(),
        "assign-str v10 (literal wstring[1] \"€\")"
    );
}

/// REQ-LOW-lowering-050: a string is stored at a string of its own
/// encoding, and only at a string.
#[test]
fn assign_str_when_other_encoding_or_not_string_then_internal_error() {
    let mut f = fixture();
    let (s, _, _) = strings(&mut f);
    assert!(is_internal_error_naming(
        f.b.assign_str(f.place(s), literal("abc", Encoding::Wide), span()),
        "Stmt::assign_str"
    ));
    assert!(is_internal_error_naming(
        f.b.assign_str(f.place(f.dint), literal("abc", Encoding::Narrow), span()),
        "Stmt::assign_str"
    ));
    assert!(is_internal_error_naming(
        f.b.read_str(f.place(f.dint), span()),
        "ProgramBuilder::read_str"
    ));
}

/// REQ-LOW-lowering-051: a literal's characters fit its encoding: a byte
/// for a `STRING`, a 16-bit code unit for a `WSTRING`.
#[test]
fn literal_when_character_does_not_fit_encoding_then_internal_error() {
    let narrow = |text: &str| StrExpr::literal(text.chars().collect(), Encoding::Narrow, span());
    let wide = |text: &str| StrExpr::literal(text.chars().collect(), Encoding::Wide, span());
    assert!(narrow("é").is_ok());
    assert!(is_internal_error_naming(narrow("€"), "StrExpr::literal"));
    assert!(wide("€").is_ok());
    assert!(is_internal_error_naming(wide("😀"), "StrExpr::literal"));
}
