//! Tests of the `Call` expression's constructor and text form.

use super::*;

const U32: ScalarType = ScalarType::I32(Signedness::Unsigned);

fn shl(args: Vec<Expr>) -> Result<Expr, ironplc_dsl::diagnostic::Diagnostic> {
    Expr::call(
        Intrinsic::ShlU32,
        args.into_iter().map(Arg::Value).collect(),
        span(),
    )
}

/// REQ-LOW-ir-145: a call whose arguments have the types its intrinsic names
/// has the intrinsic's result type.
#[test]
fn call_when_arguments_of_intrinsic_types_then_result_type() {
    let f = fixture();
    let value = Expr::integer(5, U32, span()).unwrap();
    let call = shl(vec![value, f.read(f.dint)]).unwrap();
    assert_eq!(call.ty(), U32);
    assert_eq!(
        call.to_string(),
        "(call shl_u32 i32u (const i32u 5) (read i32s v0))"
    );
}

/// REQ-LOW-ir-145: a call has one argument for each parameter.
#[test]
fn call_when_argument_missing_then_internal_error() {
    let value = Expr::integer(5, U32, span()).unwrap();
    assert!(is_internal_error_naming(shl(vec![value]), "Expr::call"));
}

/// REQ-LOW-ir-145: each argument has its parameter's type.
#[test]
fn call_when_argument_of_another_type_then_internal_error() {
    assert!(is_internal_error_naming(
        shl(vec![int(5), int(3)]),
        "Expr::call"
    ));
}

#[test]
fn call_when_arguments_read_two_programs_then_internal_error() {
    let f = fixture();
    let g = fixture();
    let value = Expr::convert(f.read(f.dint), U32, span()).unwrap();
    assert!(is_internal_error_naming(
        shl(vec![value, g.read(g.dint)]),
        "Expr::call"
    ));
}

/// REQ-LOW-ir-145: a real function takes and returns values of its real type.
#[test]
fn call_when_real_function_of_its_type_then_real_result() {
    let f = fixture();
    let call = Expr::call(
        Intrinsic::Atan2F64,
        vec![Arg::Value(f.read(f.lreal)), Arg::Value(f.read(f.lreal))],
        span(),
    )
    .unwrap();
    assert_eq!(call.ty(), ScalarType::F64);
    assert_eq!(
        call.to_string(),
        "(call atan2_f64 f64 (read f64 v4) (read f64 v4))"
    );
}

/// REQ-LOW-ir-145: `MUX` takes its selector, then two or more inputs of its
/// type, and has that type.
#[test]
fn call_when_mux_of_two_or_more_inputs_then_input_type() {
    let f = fixture();
    let mux = |inputs: usize| {
        let args = std::iter::once(Arg::Value(int(1)))
            .chain((0..inputs).map(|_| Arg::Value(f.read(f.lreal))))
            .collect();
        Expr::call(Intrinsic::MuxF64, args, span())
    };
    assert_eq!(mux(2).unwrap().ty(), ScalarType::F64);
    assert_eq!(mux(5).unwrap().ty(), ScalarType::F64);
    assert!(is_internal_error_naming(mux(1), "Expr::call"));
}

/// REQ-LOW-ir-145: each input of a `MUX` has the type its variant names.
#[test]
fn call_when_mux_input_of_another_type_then_internal_error() {
    let f = fixture();
    let args = vec![
        Arg::Value(int(1)),
        Arg::Value(f.read(f.lreal)),
        Arg::Value(f.read(f.lreal)),
        Arg::Value(int(2)),
    ];
    assert!(is_internal_error_naming(
        Expr::call(Intrinsic::MuxF64, args, span()),
        "Expr::call"
    ));
}

/// REQ-LOW-ir-145: `INT_TO_BCD` of a signed integer takes that integer and
/// returns the bit string as wide as it; an unsigned argument is refused.
#[test]
fn call_when_int_to_bcd_of_signed_then_bit_string_result() {
    let f = fixture();
    let call = Expr::call(
        Intrinsic::IntToBcdI32,
        vec![Arg::Value(f.read(f.dint))],
        span(),
    )
    .unwrap();
    assert_eq!(call.ty(), U32);
    assert_eq!(
        call.to_string(),
        "(call int_to_bcd_i32 i32u (read i32s v0))"
    );
    let unsigned = Expr::integer(5, U32, span()).unwrap();
    assert!(is_internal_error_naming(
        Expr::call(Intrinsic::IntToBcdI32, vec![Arg::Value(unsigned)], span(),),
        "Expr::call"
    ));
}

/// A function `f` of the fixture's program, taking a `DINT` and an `LREAL`
/// and returning an `INT`, with its parameters and result.
fn function(f: &mut Fixture) -> (PouId, Vec<VarId>, VarId) {
    let pou = f.b.push_pou(name("f"));
    let (dint, lreal, int) = (f.ty(f.dint), f.ty(f.lreal), f.ty(f.int));
    let mut var = |text: &str, ty| f.b.push_variable(name(text), Owner::Pou(pou), ty).unwrap();
    let parameters = vec![var("a", dint), var("x", lreal)];
    let result = var("f", int);
    (pou, parameters, result)
}

/// REQ-LOW-lowering-071, REQ-LOW-lowering-072: a call of a function passes
/// one argument for each parameter, in order, at the parameter's operation
/// type, and has the operation type of the function's result.
#[test]
fn call_when_arguments_of_function_parameters_then_result_type() {
    let mut f = fixture();
    let (pou, parameters, result) = function(&mut f);
    f.b.declare_function(pou, parameters.clone(), result, &span())
        .unwrap();
    let signature = f.b.pous().function(pou).unwrap();
    assert_eq!(signature.parameters(), parameters);
    assert_eq!(signature.result(), Some(result));
    let args = vec![Arg::Value(f.read(f.dint)), Arg::Value(f.read(f.lreal))];
    let call = f.b.call(pou, args, span()).unwrap();
    assert_eq!(call.ty(), DINT);
    assert_eq!(
        call.to_string(),
        "(call pou1 i32s (read i32s v0) (read f64 v4))"
    );
}

/// REQ-LOW-lowering-071, REQ-LOW-lowering-072: a call of a function with an
/// argument missing, out of order or of another type, of a POU that is not a
/// declared function, or reading another program's places is refused.
#[test]
fn call_when_arguments_do_not_match_function_then_internal_error() {
    let mut f = fixture();
    let (pou, parameters, result) = function(&mut f);
    f.b.declare_function(pou, parameters, result, &span())
        .unwrap();
    let main = f.b.variables().get(f.dint).unwrap().owner();
    let Owner::Pou(main) = main else {
        panic!("the fixture's variables belong to main");
    };
    let g = fixture();
    let calls = [
        (pou, vec![f.read(f.dint)]),
        (pou, vec![f.read(f.lreal), f.read(f.dint)]),
        (pou, vec![f.read(f.dint), f.read(f.dint)]),
        (main, vec![]),
        (pou, vec![g.read(g.dint), g.read(g.lreal)]),
    ];
    for (callee, args) in calls {
        let args = args.into_iter().map(Arg::Value).collect();
        assert!(is_internal_error_naming(
            f.b.call(callee, args, span()),
            "ProgramBuilder::call"
        ));
    }
}

/// A function's parameters and result are variables it owns, none of them
/// an aggregate, and it is declared once.
#[test]
fn declare_function_when_variable_not_its_own_scalar_or_declared_twice_then_internal_error() {
    let mut f = fixture();
    let (pou, parameters, result) = function(&mut f);
    let program = f.ty(f.program);
    let aggregate =
        f.b.push_variable(name("p"), Owner::Pou(pou), program)
            .unwrap();
    // `f.dint` belongs to `main`, and `aggregate` is no scalar.
    let refused = [(vec![f.dint], result), (parameters.clone(), aggregate)];
    for (parameters, result) in refused {
        assert!(is_internal_error_naming(
            f.b.declare_function(pou, parameters, result, &span()),
            "ProgramBuilder::declare_function"
        ));
    }
    f.b.declare_function(pou, parameters.clone(), result, &span())
        .unwrap();
    assert!(is_internal_error_naming(
        f.b.declare_function(pou, parameters, result, &span()),
        "ProgramBuilder::declare_function"
    ));
}

/// REQ-LOW-lowering-033, REQ-LOW-lowering-072: a function's `VAR_IN_OUT`
/// parameter, a variable of a non-null reference type, takes a `Ref` to a
/// place of the type it refers to: not a value, nor a place of another type.
/// A nullable reference parameter, a `REF_TO`, takes its value.
#[test]
fn call_when_in_out_parameter_then_takes_ref_to_place_of_its_type() {
    let mut f = fixture();
    let pou = f.b.push_pou(name("inc"));
    let (dint, ref_to) = (f.ty(f.dint), f.ty(f.reference));
    let in_out = f.b.push_type(
        entry(TypeKind::Reference {
            target: dint,
            nullability: Nullability::NonNull,
        }),
        &span(),
    );
    let in_out = in_out.unwrap();
    let mut var = |text: &str, ty| f.b.push_variable(name(text), Owner::Pou(pou), ty).unwrap();
    let parameters = vec![var("v", in_out), var("r", ref_to)];
    let result = var("inc", dint);
    f.b.declare_function(pou, parameters, result, &span())
        .unwrap();
    assert_eq!(
        f.b.params(pou),
        Some(vec![Param::Ref(dint), Param::Value(ScalarType::Ref)])
    );
    let reference = || Arg::Value(f.read(f.reference));
    let args = vec![Arg::Ref(f.place(f.dint)), reference()];
    let call = f.b.call(pou, args, span()).unwrap();
    assert_eq!(call.to_string(), "(call pou1 i32s (ref v0) (read ref v7))");
    let refused = [
        vec![Arg::Value(f.read(f.dint)), reference()],
        vec![Arg::Ref(f.place(f.int)), reference()],
    ];
    for args in refused {
        assert!(is_internal_error_naming(
            f.b.call(pou, args, span()),
            "ProgramBuilder::call"
        ));
    }
}

/// A function `g` of the fixture's program, taking a `STRING[10]` and a
/// `WSTRING[10]` and returning a `DINT`, with its parameters and result.
fn string_function(f: &mut Fixture) -> (PouId, Vec<VarId>, VarId) {
    let pou = f.b.push_pou(name("g"));
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
    let dint = f.ty(f.dint);
    let mut var = |text: &str, ty| f.b.push_variable(name(text), Owner::Pou(pou), ty).unwrap();
    let parameters = vec![var("s", narrow), var("w", wide)];
    let result = var("g", dint);
    (pou, parameters, result)
}

fn literal(text: &str, encoding: Encoding) -> Arg {
    Arg::Str(StrExpr::literal(text.chars().collect(), encoding, span()).unwrap())
}

/// REQ-LOW-lowering-072: a function's string parameter, held by value,
/// takes a string of its encoding, of any capacity: not a value, nor a
/// string of the other encoding.
#[test]
fn call_when_string_parameters_then_takes_strings_of_their_encodings() {
    let mut f = fixture();
    let (pou, parameters, result) = string_function(&mut f);
    f.b.declare_function(pou, parameters, result, &span())
        .unwrap();
    assert_eq!(
        f.b.params(pou),
        Some(vec![
            Param::Str(Encoding::Narrow),
            Param::Str(Encoding::Wide)
        ])
    );
    let args = vec![
        literal("abcdefghijkl", Encoding::Narrow),
        literal("x", Encoding::Wide),
    ];
    let call = f.b.call(pou, args, span()).unwrap();
    assert_eq!(
        call.to_string(),
        "(call pou1 i32s (literal string[12] \"abcdefghijkl\") (literal wstring[1] \"x\"))"
    );
    let refused = [
        vec![Arg::Value(f.read(f.dint)), literal("x", Encoding::Wide)],
        vec![literal("a", Encoding::Wide), literal("x", Encoding::Wide)],
    ];
    for args in refused {
        assert!(is_internal_error_naming(
            f.b.call(pou, args, span()),
            "ProgramBuilder::call"
        ));
    }
}

/// A function's result is a scalar variable: a string result is not one.
#[test]
fn declare_function_when_result_is_string_then_internal_error() {
    let mut f = fixture();
    let (pou, parameters, _) = string_function(&mut f);
    let string_result = parameters[0];
    assert!(is_internal_error_naming(
        f.b.declare_function(pou, parameters, string_result, &span()),
        "ProgramBuilder::declare_function"
    ));
}

/// A function `mk` of the fixture's program whose result is a `STRING[10]`,
/// returned through its first parameter, then a `DINT` input; the string
/// type, and `main`'s temporary of that type for a call to pass.
fn returning_string(f: &mut Fixture) -> (PouId, Vec<VarId>, TypeId, VarId) {
    let pou = f.b.push_pou(name("mk"));
    let shape = StringShape {
        encoding: Encoding::Narrow,
        capacity: 10,
    };
    let string =
        f.b.push_type(entry(TypeKind::String(shape)), &span())
            .unwrap();
    let result = f.b.push_type(
        entry(TypeKind::Reference {
            target: string,
            nullability: Nullability::NonNull,
        }),
        &span(),
    );
    let (result, dint) = (result.unwrap(), f.ty(f.dint));
    let Owner::Pou(main) = f.b.variables().get(f.dint).unwrap().owner() else {
        panic!("the fixture's variables belong to main");
    };
    let mut var = |text: &str, owner, ty| f.b.push_variable(name(text), owner, ty).unwrap();
    let parameters = vec![
        var("mk", Owner::Pou(pou), result),
        var("n", Owner::Pou(pou), dint),
    ];
    let temporary = var("mk()", Owner::Pou(main), string);
    (pou, parameters, string, temporary)
}

/// REQ-LOW-lowering-071, REQ-LOW-ir-181: a function whose result is a
/// string returns it through its first parameter, a non-null reference to
/// the string, which a call passes a `Ref` to a place of the string's type.
/// The call is a statement: the function has no value.
#[test]
fn declare_function_returning_through_when_parameter_refers_to_string_then_call_is_statement() {
    let mut f = fixture();
    let (pou, parameters, string, temporary) = returning_string(&mut f);
    f.b.declare_function_returning_through(pou, parameters.clone(), &span())
        .unwrap();
    let signature = f.b.pous().function(pou).unwrap();
    assert_eq!(signature.result_parameter(), Some(parameters[0]));
    assert_eq!(signature.result(), None);
    assert_eq!(
        f.b.params(pou),
        Some(vec![Param::Ref(string), Param::Value(DINT)])
    );
    let args = || vec![Arg::Ref(f.place(temporary)), Arg::Value(f.read(f.dint))];
    let call = f.b.call_stmt(pou, args(), span()).unwrap();
    assert_eq!(call.to_string(), "call pou1 (ref v10) (read i32s v0)");
    assert!(is_internal_error_naming(
        f.b.call(pou, args(), span()),
        "ProgramBuilder::call"
    ));
}

/// REQ-LOW-ir-181: a result parameter is a non-null reference to a string or
/// a structure, first among the parameters of a function.
#[test]
fn declare_function_returning_through_when_first_not_reference_to_string_then_internal_error() {
    let mut f = fixture();
    let (pou, parameters, _, _) = returning_string(&mut f);
    let (dint, reference) = (parameters[1], f.reference);
    let refused = [
        vec![],
        vec![dint],
        vec![dint, parameters[0]],
        vec![reference],
    ];
    for parameters in refused {
        assert!(is_internal_error_naming(
            f.b.declare_function_returning_through(pou, parameters, &span()),
            "ProgramBuilder::declare_function_returning_through"
        ));
    }
}

/// REQ-LOW-lowering-079, REQ-LOW-ir-181: a function whose result is a
/// structure returns it through its first parameter, a non-null reference to
/// the structure, as one whose result is a string does; a call of it is a
/// statement. A reference to an array is refused: no function returns one.
#[test]
fn declare_function_returning_through_when_parameter_refers_to_structure_then_call_is_statement() {
    let mut f = fixture();
    let pou = f.b.push_pou(name("mk"));
    let dint = f.ty(f.dint);
    let structure = TypeKind::Structure {
        fields: vec![Field::new("x", span(), None, dint)],
    };
    let structure = f.b.push_type(entry(structure), &span()).unwrap();
    let array = TypeKind::Array {
        dimensions: vec![(1, 2)],
        element: dint,
    };
    let array = f.b.push_type(entry(array), &span()).unwrap();
    let mut reference_to = |target| {
        let kind = TypeKind::Reference {
            target,
            nullability: Nullability::NonNull,
        };
        f.b.push_type(entry(kind), &span()).unwrap()
    };
    let (to_structure, to_array) = (reference_to(structure), reference_to(array));
    let Owner::Pou(main) = f.b.variables().get(f.dint).unwrap().owner() else {
        panic!("the fixture's variables belong to main");
    };
    let mut var = |text: &str, owner, ty| f.b.push_variable(name(text), owner, ty).unwrap();
    let result = var("mk", Owner::Pou(pou), to_structure);
    let array_result = var("mk", Owner::Pou(pou), to_array);
    let temporary = var("mk()", Owner::Pou(main), structure);
    assert!(is_internal_error_naming(
        f.b.declare_function_returning_through(pou, vec![array_result], &span()),
        "ProgramBuilder::declare_function_returning_through"
    ));
    f.b.declare_function_returning_through(pou, vec![result], &span())
        .unwrap();
    assert_eq!(f.b.params(pou), Some(vec![Param::Ref(structure)]));
    let call =
        f.b.call_stmt(pou, vec![Arg::Ref(f.place(temporary))], span());
    assert_eq!(call.unwrap().to_string(), "call pou1 (ref v10)");
}

/// REQ-LOW-lowering-072, REQ-LOW-lowering-075: a function's input that is a
/// structure takes a copy of a place of the structure's shape, a `Copy`
/// argument. A `Ref` to the place, and a copy of a structure of another
/// shape, are refused.
#[test]
fn call_when_aggregate_input_then_takes_copy_of_place_of_its_shape() {
    let mut f = fixture();
    let pou = f.b.push_pou(name("sum"));
    let dint = f.ty(f.dint);
    let mut structure = |field: &str| {
        let kind = TypeKind::Structure {
            fields: vec![Field::new(field, span(), None, dint)],
        };
        f.b.push_type(entry(kind), &span()).unwrap()
    };
    let (shape, other) = (structure("x"), structure("y"));
    let Owner::Pou(main) = f.b.variables().get(f.dint).unwrap().owner() else {
        panic!("the fixture's variables belong to main");
    };
    let mut var = |text: &str, owner, ty| f.b.push_variable(name(text), owner, ty).unwrap();
    let q = var("q", Owner::Pou(pou), shape);
    let result = var("sum", Owner::Pou(pou), dint);
    let p = var("p", Owner::Pou(main), shape);
    let o = var("o", Owner::Pou(main), other);
    f.b.declare_function(pou, vec![q], result, &span()).unwrap();
    assert_eq!(f.b.params(pou), Some(vec![Param::Copy(shape)]));
    let call = f.b.call(pou, vec![Arg::Copy(f.place(p))], span()).unwrap();
    assert_eq!(call.to_string(), "(call pou1 i32s (copy v10))");
    for refused in [Arg::Ref(f.place(p)), Arg::Copy(f.place(o))] {
        assert!(is_internal_error_naming(
            f.b.call(pou, vec![refused], span()),
            "ProgramBuilder::call"
        ));
    }
}

/// REQ-LOW-lowering-072: a `VAR_IN_OUT` parameter takes a `Ref` to a place
/// of a type of the shape it refers to, as an alias of that type is. A place
/// of another shape is refused.
#[test]
fn call_when_in_out_argument_has_type_of_same_shape_then_takes_ref() {
    let mut f = fixture();
    let pou = f.b.push_pou(name("inc"));
    let dint = f.ty(f.dint);
    let alias =
        f.b.push_type(
            entry(TypeKind::Integer {
                width: IntWidth::B32,
                signed: true,
            }),
            &span(),
        )
        .unwrap();
    let in_out =
        f.b.push_type(
            entry(TypeKind::Reference {
                target: dint,
                nullability: Nullability::NonNull,
            }),
            &span(),
        )
        .unwrap();
    let Owner::Pou(main) = f.b.variables().get(f.dint).unwrap().owner() else {
        panic!("the fixture's variables belong to main");
    };
    let mut var = |text: &str, owner, ty| f.b.push_variable(name(text), owner, ty).unwrap();
    let v = var("v", Owner::Pou(pou), in_out);
    let result = var("inc", Owner::Pou(pou), dint);
    let a = var("a", Owner::Pou(main), alias);
    f.b.declare_function(pou, vec![v], result, &span()).unwrap();
    assert_ne!(alias, dint);
    let call = f.b.call(pou, vec![Arg::Ref(f.place(a))], span()).unwrap();
    assert_eq!(call.to_string(), format!("(call pou1 i32s (ref {a}))"));
    assert!(is_internal_error_naming(
        f.b.call(pou, vec![Arg::Ref(f.place(f.int))], span()),
        "ProgramBuilder::call"
    ));
}
