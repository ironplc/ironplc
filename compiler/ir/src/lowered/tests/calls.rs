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
    assert_eq!(signature.result(), result);
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

/// A function's parameters and result are scalar variables it owns, and it
/// is declared once.
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
