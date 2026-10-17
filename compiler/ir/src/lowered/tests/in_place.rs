//! Tests of in-place calls: their constructors and text form.

use super::*;

/// A function `f` of the fixture's program, taking a `DINT` input and
/// giving a `DINT` output and result, and `main`'s temporary for the output.
fn with_output(f: &mut Fixture) -> (PouId, VarId) {
    let pou = f.b.push_pou(name("f"));
    let dint = f.ty(f.dint);
    let output = f.b.push_type(
        entry(TypeKind::Reference {
            target: dint,
            nullability: Nullability::NonNull,
        }),
        &span(),
    );
    let (output, main) = (output.unwrap(), main(f));
    let mut var = |text: &str, owner, ty| f.b.push_variable(name(text), owner, ty).unwrap();
    let parameters = vec![
        var("x", Owner::Pou(pou), dint),
        var("n", Owner::Pou(pou), output),
    ];
    let result = var("f", Owner::Pou(pou), dint);
    let temporary = var("f.n", Owner::Pou(main), dint);
    f.b.declare_function(pou, parameters, result, &span())
        .unwrap();
    (pou, temporary)
}

/// A function `g` of the fixture's program whose result is a `STRING[10]`,
/// returned through its first parameter, then a `DINT` input and a `DINT`
/// output, and `main`'s temporaries for its result and its output.
fn returning_string(f: &mut Fixture) -> (PouId, VarId, VarId) {
    let pou = f.b.push_pou(name("g"));
    let shape = StringShape {
        encoding: Encoding::Narrow,
        capacity: 10,
    };
    let dint = f.ty(f.dint);
    let string = f.b.push_type(entry(TypeKind::String(shape)), &span());
    let string = string.unwrap();
    let mut non_null = |target| {
        f.b.push_type(
            entry(TypeKind::Reference {
                target,
                nullability: Nullability::NonNull,
            }),
            &span(),
        )
        .unwrap()
    };
    let (to_string, to_dint) = (non_null(string), non_null(dint));
    let main = main(f);
    let mut var = |text: &str, owner, ty| f.b.push_variable(name(text), owner, ty).unwrap();
    let parameters = vec![
        var("g", Owner::Pou(pou), to_string),
        var("x", Owner::Pou(pou), dint),
        var("n", Owner::Pou(pou), to_dint),
    ];
    let result = var("g()", Owner::Pou(main), string);
    let temporary = var("g.n", Owner::Pou(main), dint);
    f.b.declare_function_returning_through(pou, parameters, &span())
        .unwrap();
    (pou, result, temporary)
}

/// The POU the fixture's variables belong to.
fn main(f: &Fixture) -> PouId {
    let Owner::Pou(main) = f.b.variables().get(f.dint).unwrap().owner() else {
        panic!("the fixture's variables belong to main");
    };
    main
}

/// The store of the output passed `temporary` in `target`, a `DINT`.
fn store(f: &Fixture, target: VarId, temporary: VarId) -> Stmt {
    f.b.assign(f.place(target), f.read(temporary), span())
        .unwrap()
}

/// REQ-LOW-ir-182: an in-place call has the value of its callee's result,
/// and holds the call, then the stores of its outputs, which may narrow
/// what they store.
#[test]
fn call_in_place_when_function_with_output_then_value_of_result() {
    let mut f = fixture();
    let (pou, temporary) = with_output(&mut f);
    let args = vec![Arg::Value(int(3)), Arg::Ref(f.place(temporary))];
    let narrowed = Expr::truncate(NarrowWidth::B16, f.read(temporary), span()).unwrap();
    let stores = vec![
        store(&f, f.dint, temporary),
        f.b.assign(f.place(f.int), narrowed, span()).unwrap(),
    ];
    let call = f.b.call_in_place(pou, args, stores, span()).unwrap();
    assert_eq!(call.ty(), DINT);
    assert_eq!(
        call.to_string(),
        "(call-in-place pou1 i32s (const i32s 3) (ref v11) \
         {assign v0 (read i32s v11)} {assign v1 (truncate 16 (read i32s v11))})"
    );
    assert!(matches!(
        call.kind(),
        ExprKind::InPlaceCall(call) if call.pou() == pou && call.stores().len() == 2
    ));
}

/// REQ-LOW-ir-183: a store of an in-place call is an assignment of a place
/// the call passes by reference, read whole: not a constant, another
/// variable or an operation on the output, nor a statement that is not an
/// assignment.
#[test]
fn call_in_place_when_store_not_of_output_then_internal_error() {
    let mut f = fixture();
    let (pou, temporary) = with_output(&mut f);
    let sum = Expr::binary(BinaryOp::Add, f.read(temporary), int(1), span()).unwrap();
    let refused = [
        f.b.assign(f.place(f.dint), int(1), span()).unwrap(),
        store(&f, f.dint, f.dint),
        f.b.assign(f.place(f.dint), sum, span()).unwrap(),
        Stmt::return_(span()),
    ];
    for refused in refused {
        let args = vec![Arg::Value(int(3)), Arg::Ref(f.place(temporary))];
        assert!(is_internal_error_naming(
            f.b.call_in_place(pou, args, vec![refused], span()),
            "ProgramBuilder::call_in_place"
        ));
    }
}

/// REQ-LOW-ir-182: an in-place call passes its callee's parameters as a
/// `Call` does, reads places of one program, and has a value only when its
/// callee has one: a function whose result is a string is not one.
#[test]
fn call_in_place_when_arguments_wrong_or_no_value_then_internal_error() {
    let mut f = fixture();
    let (pou, temporary) = with_output(&mut f);
    let (string, result, output) = returning_string(&mut f);
    let g = fixture();
    let calls = [
        (pou, vec![Arg::Value(int(3))], vec![]),
        (pou, vec![Arg::Value(int(3)), Arg::Value(int(4))], vec![]),
        (
            pou,
            vec![Arg::Value(int(3)), Arg::Ref(f.place(temporary))],
            vec![store(&g, g.dint, g.dint)],
        ),
        (
            string,
            vec![
                Arg::Ref(f.place(result)),
                Arg::Value(int(3)),
                Arg::Ref(f.place(output)),
            ],
            vec![],
        ),
    ];
    for (callee, args, stores) in calls {
        assert!(is_internal_error_naming(
            f.b.call_in_place(callee, args, stores, span()),
            "ProgramBuilder::call_in_place"
        ));
    }
}

/// REQ-LOW-ir-184: an in-place call of a function whose result is a string
/// is a string of the result's shape, and names the temporary it passes for
/// the result.
#[test]
fn str_call_in_place_when_function_returns_string_then_string_of_its_result() {
    let mut f = fixture();
    let (pou, result, temporary) = returning_string(&mut f);
    let args = vec![
        Arg::Ref(f.place(result)),
        Arg::Value(int(3)),
        Arg::Ref(f.place(temporary)),
    ];
    let stores = vec![store(&f, f.dint, temporary)];
    let call = f.b.str_call_in_place(pou, args, stores, span()).unwrap();
    let shape = StringShape {
        encoding: Encoding::Narrow,
        capacity: 10,
    };
    assert_eq!(call.shape(), shape);
    assert_eq!(
        call.to_string(),
        "(call-in-place pou1 string[10] (ref v11) (const i32s 3) (ref v12) \
         {assign v0 (read i32s v12)})"
    );
    assert!(matches!(
        call.kind(),
        StrExprKind::InPlaceCall { call, result: named } if call.pou() == pou && *named == result
    ));
}

/// REQ-LOW-ir-184: a string in-place call calls a function whose result is
/// a string, passing the whole of a temporary for it; not a function with a
/// value, nor a part of a place.
#[test]
fn str_call_in_place_when_no_string_result_or_not_whole_temporary_then_internal_error() {
    let mut f = fixture();
    let (scalar, temporary) = with_output(&mut f);
    let (pou, result, output) = returning_string(&mut f);
    let string = f.ty(result);
    let strings = f.b.push_type(
        entry(TypeKind::Array {
            element: string,
            dimensions: vec![(0, 1)],
        }),
        &span(),
    );
    let strings = strings.unwrap();
    let array = f.b.push_variable(name("a"), Owner::Pou(main(&f)), strings);
    let array = f.place(array.unwrap());
    let element = f.b.index(array, vec![int(0)], &span()).unwrap();
    let calls = [
        (
            scalar,
            vec![Arg::Value(int(3)), Arg::Ref(f.place(temporary))],
        ),
        (
            pou,
            vec![
                Arg::Ref(element),
                Arg::Value(int(3)),
                Arg::Ref(f.place(output)),
            ],
        ),
    ];
    for (callee, args) in calls {
        assert!(is_internal_error_naming(
            f.b.str_call_in_place(callee, args, vec![], span()),
            "ProgramBuilder::str_call_in_place"
        ));
    }
}
