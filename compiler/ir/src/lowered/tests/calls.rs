//! Tests of the `Call` expression's constructor and text form.

use super::*;

const U32: ScalarType = ScalarType::I32(Signedness::Unsigned);

fn shl(args: Vec<Expr>) -> Result<Expr, ironplc_dsl::diagnostic::Diagnostic> {
    Expr::call(
        Callee::Intrinsic(Intrinsic::ShlU32),
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
        Callee::Intrinsic(Intrinsic::Atan2F64),
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
        Expr::call(Callee::Intrinsic(Intrinsic::MuxF64), args, span())
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
        Expr::call(Callee::Intrinsic(Intrinsic::MuxF64), args, span()),
        "Expr::call"
    ));
}
