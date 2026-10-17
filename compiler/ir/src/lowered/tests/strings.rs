//! Tests of string expressions, the `AssignStr` statement and calls of the
//! string functions.

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

/// The string at `var`, as an argument.
fn str_arg(f: &Fixture, var: VarId) -> Arg {
    Arg::Str(f.b.read_str(f.place(var), span()).unwrap())
}

/// REQ-LOW-ir-145: `LEN` and `FIND` take strings of the encoding their
/// intrinsic names, of any capacity, and yield an `INT`.
#[test]
fn call_when_strings_of_intrinsic_encoding_then_int_result() {
    let mut f = fixture();
    let (s, _, w) = strings(&mut f);
    let len = Expr::call(Intrinsic::LenString, vec![str_arg(&f, s)], span()).unwrap();
    assert_eq!(len.ty(), ScalarType::I32(Signedness::Signed));
    assert_eq!(
        len.to_string(),
        "(call len_string i32s (read string[10] v8))"
    );
    let find = Expr::call(
        Intrinsic::FindString,
        vec![str_arg(&f, s), Arg::Str(literal("ab", Encoding::Narrow))],
        span(),
    )
    .unwrap();
    assert_eq!(
        find.to_string(),
        "(call find_string i32s (read string[10] v8) (literal string[2] \"ab\"))"
    );
    let wide = Expr::call(
        Intrinsic::FindWString,
        vec![str_arg(&f, w), str_arg(&f, w)],
        span(),
    );
    assert!(wide.is_ok());
}

/// REQ-LOW-lowering-050: each string argument is of the encoding its
/// intrinsic names, and a string parameter takes no value.
#[test]
fn call_when_string_of_other_encoding_or_value_then_internal_error() {
    let mut f = fixture();
    let (s, _, w) = strings(&mut f);
    assert!(is_internal_error_naming(
        Expr::call(Intrinsic::LenString, vec![str_arg(&f, w)], span()),
        "Expr::call"
    ));
    assert!(is_internal_error_naming(
        Expr::call(
            Intrinsic::FindString,
            vec![str_arg(&f, s), str_arg(&f, w)],
            span()
        ),
        "Expr::call"
    ));
    assert!(is_internal_error_naming(
        Expr::call(Intrinsic::LenString, vec![Arg::Value(int(3))], span()),
        "Expr::call"
    ));
}

#[test]
fn call_when_string_arguments_read_two_programs_then_internal_error() {
    let mut f = fixture();
    let mut g = fixture();
    let (s, _, _) = strings(&mut f);
    let (other, _, _) = strings(&mut g);
    assert!(is_internal_error_naming(
        Expr::call(
            Intrinsic::FindString,
            vec![str_arg(&f, s), str_arg(&g, other)],
            span()
        ),
        "Expr::call"
    ));
}

/// REQ-LOW-lowering-052: a standard string function that yields a string is
/// a call of its intrinsic, of the encoding the intrinsic names and the
/// capacity lowering gives it, with each length and position a value.
#[test]
fn str_call_when_arguments_of_intrinsic_then_string_of_given_capacity() {
    let mut f = fixture();
    let (s, _, w) = strings(&mut f);
    let concat = StrExpr::call(
        Intrinsic::ConcatString,
        vec![str_arg(&f, s), Arg::Str(literal("ab", Encoding::Narrow))],
        12,
        span(),
    )
    .unwrap();
    assert_eq!(
        concat.shape(),
        StringShape {
            encoding: Encoding::Narrow,
            capacity: 12
        }
    );
    assert_eq!(
        concat.to_string(),
        "(call concat_string string[12] (read string[10] v8) (literal string[2] \"ab\"))"
    );
    let mid = StrExpr::call(
        Intrinsic::MidWString,
        vec![str_arg(&f, w), Arg::Value(int(2)), Arg::Value(int(1))],
        10,
        span(),
    )
    .unwrap();
    assert_eq!(
        mid.to_string(),
        "(call mid_wstring wstring[10] (read wstring[10] v10) (const i32s 2) (const i32s 1))"
    );
    let nested = StrExpr::call(
        Intrinsic::LeftString,
        vec![Arg::Str(concat), Arg::Value(int(3))],
        12,
        span(),
    );
    assert!(nested.is_ok());
}

/// A call yields what its intrinsic yields: a string function that yields a
/// string is never a value, and one that yields a number never a string.
#[test]
fn call_when_intrinsic_yields_other_kind_then_internal_error() {
    let mut f = fixture();
    let (s, _, _) = strings(&mut f);
    assert!(is_internal_error_naming(
        StrExpr::call(Intrinsic::LenString, vec![str_arg(&f, s)], 10, span()),
        "StrExpr::call"
    ));
    assert!(is_internal_error_naming(
        Expr::call(
            Intrinsic::ConcatString,
            vec![str_arg(&f, s), str_arg(&f, s)],
            span()
        ),
        "Expr::call"
    ));
}

/// REQ-LOW-ir-145: each argument of a string call passes what its parameter
/// takes, and the arguments read places of one program.
#[test]
fn str_call_when_argument_not_taken_or_two_programs_then_internal_error() {
    let mut f = fixture();
    let mut g = fixture();
    let (s, _, w) = strings(&mut f);
    let (other, _, _) = strings(&mut g);
    assert!(is_internal_error_naming(
        StrExpr::call(
            Intrinsic::LeftString,
            vec![Arg::Value(int(3)), Arg::Value(int(3))],
            10,
            span()
        ),
        "StrExpr::call"
    ));
    assert!(is_internal_error_naming(
        StrExpr::call(
            Intrinsic::ConcatString,
            vec![str_arg(&f, s), str_arg(&f, w)],
            20,
            span()
        ),
        "StrExpr::call"
    ));
    assert!(is_internal_error_naming(
        StrExpr::call(
            Intrinsic::ConcatString,
            vec![str_arg(&f, s), str_arg(&g, other)],
            20,
            span()
        ),
        "StrExpr::call"
    ));
}

/// REQ-LOW-lowering-073: a conversion from a `STRING` reads a number of its
/// target type as the policy it names says, and one to a `STRING` writes a
/// string of the capacity lowering gives it. Neither converts a `WSTRING`,
/// and each takes the type its intrinsic names.
#[test]
fn call_when_string_conversion_then_number_or_string_of_its_intrinsic() {
    let mut f = fixture();
    let (s, _, w) = strings(&mut f);
    let lenient = ParsePolicy {
        non_numeric: NonNumeric::IgnoreTrailing,
        failure: ParseFailure::Zero,
    };
    let parse = Expr::call(
        Intrinsic::StringToI32(lenient),
        vec![str_arg(&f, s)],
        span(),
    )
    .unwrap();
    assert_eq!(parse.ty(), ScalarType::I32(Signedness::Signed));
    assert_eq!(
        parse.to_string(),
        "(call string_to_i32[ignore-trailing,zero] i32s (read string[10] v8))"
    );
    let unsigned = ScalarType::I32(Signedness::Unsigned);
    let seven = || Arg::Value(Expr::integer(7, unsigned, span()).unwrap());
    let write = StrExpr::call(Intrinsic::U32ToString, vec![seven()], 254, span()).unwrap();
    assert_eq!(
        write.to_string(),
        "(call u32_to_string string[254] (const i32u 7))"
    );
    let strict = ParsePolicy {
        non_numeric: NonNumeric::Reject,
        failure: ParseFailure::Trap,
    };
    assert!(is_internal_error_naming(
        Expr::call(Intrinsic::StringToF64(strict), vec![str_arg(&f, w)], span()),
        "Expr::call"
    ));
    assert!(is_internal_error_naming(
        StrExpr::call(Intrinsic::I32ToString, vec![seven()], 254, span()),
        "StrExpr::call"
    ));
}

/// REQ-LOW-lowering-050: a comparison of strings orders two strings of the
/// encoding its intrinsic names, of any capacity, and yields a `DINT`.
#[test]
fn call_when_compare_strings_of_intrinsic_encoding_then_int_result() {
    let mut f = fixture();
    let (s, t, w) = strings(&mut f);
    let order = Expr::call(
        Intrinsic::CompareString,
        vec![str_arg(&f, s), str_arg(&f, t)],
        span(),
    )
    .unwrap();
    assert_eq!(order.ty(), ScalarType::I32(Signedness::Signed));
    assert_eq!(
        order.to_string(),
        "(call compare_string i32s (read string[10] v8) (read string[10] v9))"
    );
    let wide = Expr::call(
        Intrinsic::CompareWString,
        vec![str_arg(&f, w), Arg::Str(literal("ab", Encoding::Wide))],
        span(),
    );
    assert!(wide.is_ok());
    assert!(is_internal_error_naming(
        Expr::call(
            Intrinsic::CompareString,
            vec![str_arg(&f, s), str_arg(&f, w)],
            span()
        ),
        "Expr::call"
    ));
}
