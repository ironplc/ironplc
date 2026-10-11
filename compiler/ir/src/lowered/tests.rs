//! Tests of the lowered program's constructors and text form.
//!
//! Each test names the requirement of `specs/design/lowered-program.md` it
//! checks. They become `#[spec_test]`s when this crate lists the design in
//! its build script, once every requirement the crate owns is tested.

use ironplc_dsl::core::SourceSpan;

use super::*;

fn span() -> SourceSpan {
    SourceSpan::default()
}

fn entry(kind: TypeKind) -> TypeEntry {
    TypeEntry { name: None, kind }
}

/// A table with a `DINT`, an `INT`, a `SINT`, a `BOOL`, an `LREAL`, a program
/// type with one `INT` field, and a non-null reference to that program type.
struct Fixture {
    types: TypeTable,
    dint: TypeId,
    int: TypeId,
    sint: TypeId,
    bool: TypeId,
    lreal: TypeId,
    program: TypeId,
    this: TypeId,
}

fn fixture() -> Fixture {
    let mut types = TypeTable::new();
    let dint = types.push(entry(TypeKind::Integer {
        bits: 32,
        signed: true,
    }));
    let int = types.push(entry(TypeKind::Integer {
        bits: 16,
        signed: true,
    }));
    let sint = types.push(entry(TypeKind::Integer {
        bits: 8,
        signed: true,
    }));
    let bool = types.push(entry(TypeKind::Bool));
    let lreal = types.push(entry(TypeKind::Real { bits: 64 }));
    let program = types.push(entry(TypeKind::Program {
        fields: vec![Field::new("x", span(), Some(Section::Local), int)],
        body: PouId::new(0),
    }));
    let this = types.push(entry(TypeKind::Reference {
        target: program,
        nullability: Nullability::NonNull,
    }));
    Fixture {
        types,
        dint,
        int,
        sint,
        bool,
        lreal,
        program,
        this,
    }
}

fn int(value: i128) -> Expr {
    Expr::integer(value, ScalarType::BOOL, span()).unwrap()
}

fn is_internal_error_naming(
    result: Result<impl std::fmt::Debug, ironplc_dsl::diagnostic::Diagnostic>,
    constructor: &str,
) -> bool {
    match result {
        Ok(_) => false,
        Err(diagnostic) => {
            diagnostic.code == "P9998" && diagnostic.primary.message.starts_with(constructor)
        }
    }
}

/// REQ-LOW-ir-019: the ids are distinct types.
#[test]
fn ids_when_compared_then_each_is_its_own_type() {
    use std::any::TypeId as RustType;
    let ids = [
        RustType::of::<VarId>(),
        RustType::of::<PouId>(),
        RustType::of::<FieldIdx>(),
        RustType::of::<LoopId>(),
        RustType::of::<TypeId>(),
    ];
    for (i, a) in ids.iter().enumerate() {
        for b in &ids[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

/// REQ-LOW-lowering-041: a constant is of its type's width, so its type
/// holds it.
#[test]
fn constant_when_width_differs_from_type_then_internal_error() {
    assert!(is_internal_error_naming(
        Expr::constant(Const::I64(1), ScalarType::BOOL, span()),
        "Expr::constant"
    ));
    assert!(Expr::constant(Const::F32(1.5), ScalarType::F64, span()).is_err());
    assert!(Expr::constant(Const::F64(1.5), ScalarType::F64, span()).is_ok());
}

/// REQ-LOW-lowering-041: an integer becomes a constant only of a type that
/// holds it.
#[test]
fn integer_when_value_outside_type_then_internal_error() {
    let unsigned = ScalarType::I32(Signedness::Unsigned);
    assert!(Expr::integer(-1, unsigned, span()).is_err());
    assert!(Expr::integer(i128::from(u32::MAX), unsigned, span()).is_ok());
    assert!(is_internal_error_naming(
        Expr::integer(1 << 31, ScalarType::BOOL, span()),
        "Expr::integer"
    ));
    assert!(Expr::integer(1, ScalarType::Ref, span()).is_err());
}

/// An unsigned constant is stored as its bit pattern and reads back as the
/// unsigned value it stands for.
#[test]
fn integer_when_unsigned_above_signed_range_then_bit_pattern_reads_back() {
    let unsigned = ScalarType::I32(Signedness::Unsigned);
    let constant = Expr::integer(4_000_000_000, unsigned, span()).unwrap();
    assert!(matches!(
        constant.kind(),
        ExprKind::Const(Const::I32(-294_967_296))
    ));
    assert_eq!(constant.to_string(), "(const i32u 4000000000)");
    let wide = Expr::integer(
        i128::from(u64::MAX),
        ScalarType::I64(Signedness::Unsigned),
        span(),
    );
    assert_eq!(
        wide.unwrap().to_string(),
        "(const i64u 18446744073709551615)"
    );
}

/// An integer at a real type is rounded once, from the integer, as a literal
/// written at that type is.
#[test]
fn integer_when_real_type_then_rounded_once() {
    let value = (1i128 << 60) + 1;
    let constant = Expr::integer(value, ScalarType::F32, span()).unwrap();
    assert!(matches!(constant.kind(), ExprKind::Const(Const::F32(v)) if *v == value as f32));
}

/// A real becomes a constant only of a real type, rounded to it.
#[test]
fn real_when_integer_type_then_internal_error() {
    assert!(is_internal_error_naming(
        Expr::real(1.5, ScalarType::BOOL, span()),
        "Expr::real"
    ));
    let narrowed = Expr::real(0.1, ScalarType::F32, span()).unwrap();
    assert!(matches!(narrowed.kind(), ExprKind::Const(Const::F32(v)) if *v == 0.1f32));
}

/// REQ-LOW-lowering-042: the operands of a binary operation have its type.
#[test]
fn binary_when_operand_types_differ_then_internal_error() {
    let wide = Expr::integer(1, ScalarType::I64(Signedness::Signed), span()).unwrap();
    assert!(is_internal_error_naming(
        Expr::binary(BinaryOp::Add, int(1), wide, span()),
        "Expr::binary"
    ));
    let sum = Expr::binary(BinaryOp::Add, int(1), int(2), span()).unwrap();
    assert_eq!(sum.ty(), ScalarType::BOOL);
}

/// REQ-LOW-lowering-042: an integer-only operator does not apply to a real.
#[test]
fn binary_when_mod_of_reals_then_internal_error() {
    let real = || Expr::real(1.0, ScalarType::F32, span()).unwrap();
    assert!(Expr::binary(BinaryOp::Mod, real(), real(), span()).is_err());
    assert!(Expr::binary(BinaryOp::Div, real(), real(), span()).is_ok());
}

/// REQ-LOW-lowering-043: a comparison compares one type and yields a `BOOL`.
#[test]
fn compare_when_types_match_then_bool_and_when_not_then_internal_error() {
    let real = Expr::real(1.0, ScalarType::F64, span()).unwrap();
    assert!(is_internal_error_naming(
        Expr::compare(CompareOp::Lt, int(1), real.clone(), span()),
        "Expr::compare"
    ));
    let compared = Expr::compare(CompareOp::Lt, real.clone(), real, span()).unwrap();
    assert_eq!(compared.ty(), ScalarType::BOOL);
}

/// REQ-LOW-lowering-044: a conversion changes type.
#[test]
fn convert_when_same_type_then_internal_error() {
    assert!(is_internal_error_naming(
        Expr::convert(int(1), ScalarType::BOOL, span()),
        "Expr::convert"
    ));
    let widened = Expr::convert(int(1), ScalarType::I64(Signedness::Signed), span()).unwrap();
    assert_eq!(widened.ty(), ScalarType::I64(Signedness::Signed));
}

/// REQ-LOW-lowering-047: a read has the operation type of the place's type.
#[test]
fn read_when_place_is_int_then_operation_type_of_int() {
    let f = fixture();
    let place = Place::var(VarId::new(0), f.int);
    let read = Expr::read(&f.types, place, span()).unwrap();
    assert_eq!(read.ty(), ScalarType::I32(Signedness::Signed));
}

/// REQ-LOW-lowering-047: an aggregate is not read as one value.
#[test]
fn read_when_place_is_program_then_internal_error() {
    let f = fixture();
    let place = Place::var(VarId::new(0), f.program);
    assert!(is_internal_error_naming(
        Expr::read(&f.types, place, span()),
        "Expr::read"
    ));
}

/// REQ-LOW-lowering-030, REQ-LOW-lowering-032: a field is reached through a
/// dereference of the instance parameter.
#[test]
fn place_when_field_through_deref_then_type_of_field() {
    let f = fixture();
    let place = Place::var(VarId::new(0), f.this)
        .deref(&f.types, &span())
        .unwrap()
        .field(&f.types, FieldIdx::new(0), &span())
        .unwrap();
    assert_eq!(place.ty(), f.int);
    assert_eq!(place.to_string(), "v0^.f0");
}

/// REQ-LOW-lowering-030: a field index lies within the type's fields.
#[test]
fn place_when_field_index_outside_type_then_internal_error() {
    let f = fixture();
    let program = Place::var(VarId::new(0), f.program);
    assert!(is_internal_error_naming(
        program.clone().field(&f.types, FieldIdx::new(1), &span()),
        "Place::field"
    ));
    let scalar = Place::var(VarId::new(0), f.dint);
    assert!(scalar.field(&f.types, FieldIdx::new(0), &span()).is_err());
}

/// REQ-LOW-lowering-032: only a reference is dereferenced.
#[test]
fn place_when_deref_of_non_reference_then_internal_error() {
    let f = fixture();
    let place = Place::var(VarId::new(0), f.program);
    assert!(is_internal_error_naming(
        place.deref(&f.types, &span()),
        "Place::deref"
    ));
}

/// REQ-LOW-lowering-060: an assigned value has the place's operation type.
#[test]
fn assign_when_value_type_differs_from_place_then_internal_error() {
    let f = fixture();
    let place = Place::var(VarId::new(0), f.lreal);
    assert!(is_internal_error_naming(
        Stmt::assign(&f.types, place, int(1), span()),
        "Stmt::assign"
    ));
}

/// REQ-LOW-lowering-046: a value stored narrower than its operation width is
/// truncated to the place's width, a constant within its range, or a read
/// of a place whose range lies within it.
#[test]
fn assign_when_narrow_place_then_value_must_fit() {
    let f = fixture();
    let int_place = || Place::var(VarId::new(0), f.int);
    let read = |ty| Expr::read(&f.types, Place::var(VarId::new(1), ty), span()).unwrap();

    let sum = Expr::binary(BinaryOp::Add, int(1), int(2), span()).unwrap();
    assert!(is_internal_error_naming(
        Stmt::assign(&f.types, int_place(), sum.clone(), span()),
        "Stmt::assign"
    ));
    let truncated = Expr::truncate(16, sum, span()).unwrap();
    assert!(Stmt::assign(&f.types, int_place(), truncated, span()).is_ok());

    assert!(Stmt::assign(&f.types, int_place(), int(32767), span()).is_ok());
    assert!(Stmt::assign(&f.types, int_place(), int(32768), span()).is_err());

    assert!(Stmt::assign(&f.types, int_place(), read(f.sint), span()).is_ok());
    assert!(Stmt::assign(&f.types, int_place(), read(f.dint), span()).is_err());
}

/// REQ-LOW-lowering-046: a `BOOL` is never narrowed.
#[test]
fn assign_when_bool_place_then_comparison_stored_as_is() {
    let f = fixture();
    let compared = Expr::compare(CompareOp::Eq, int(1), int(2), span()).unwrap();
    let place = Place::var(VarId::new(0), f.bool);
    assert!(Stmt::assign(&f.types, place, compared, span()).is_ok());
}

/// REQ-LOW-lowering-036: a non-null reference is never assigned.
#[test]
fn assign_when_non_null_reference_then_internal_error() {
    let mut f = fixture();
    let target = f.dint;
    let nullable = f.types.push(entry(TypeKind::Reference {
        target,
        nullability: Nullability::Nullable,
    }));
    let source = Expr::read(&f.types, Place::var(VarId::new(1), nullable), span()).unwrap();
    assert!(is_internal_error_naming(
        Stmt::assign(
            &f.types,
            Place::var(VarId::new(0), f.this),
            source.clone(),
            span()
        ),
        "Stmt::assign"
    ));
    assert!(Stmt::assign(
        &f.types,
        Place::var(VarId::new(0), nullable),
        source,
        span()
    )
    .is_ok());
}

/// REQ-LOW-lowering-048: the logical forms apply only to `BOOL` values.
#[test]
fn unary_when_bool_not_of_wide_integer_then_internal_error() {
    let wide = Expr::integer(1, ScalarType::I64(Signedness::Signed), span()).unwrap();
    assert!(Expr::unary(UnaryOp::BoolNot, wide.clone(), span()).is_err());
    assert!(Expr::unary(UnaryOp::BitNot, wide, span()).is_ok());
}

#[test]
fn truncate_when_not_narrower_then_internal_error() {
    assert!(is_internal_error_naming(
        Expr::truncate(32, int(1), span()),
        "Expr::truncate"
    ));
    let real = Expr::real(1.0, ScalarType::F32, span()).unwrap();
    assert!(Expr::truncate(16, real, span()).is_err());
}

#[test]
fn print_when_assignment_then_s_expression() {
    let f = fixture();
    let field = Place::var(VarId::new(2), f.this)
        .deref(&f.types, &span())
        .unwrap()
        .field(&f.types, FieldIdx::new(0), &span())
        .unwrap();
    let sum = Expr::binary(
        BinaryOp::Add,
        Expr::read(&f.types, field.clone(), span()).unwrap(),
        int(1),
        span(),
    )
    .unwrap();
    let stmt = Stmt::assign(
        &f.types,
        field,
        Expr::truncate(16, sum, span()).unwrap(),
        span(),
    )
    .unwrap();
    assert_eq!(
        stmt.to_string(),
        "assign v2^.f0 (truncate 16 (add i32s (read i32s v2^.f0) (const i32s 1)))"
    );
}

#[test]
fn type_table_when_operation_types_then_follow_adr_0001() {
    let f = fixture();
    assert_eq!(
        f.types.operation_type(f.sint),
        Some(ScalarType::I32(Signedness::Signed))
    );
    assert_eq!(f.types.operation_type(f.lreal), Some(ScalarType::F64));
    assert_eq!(f.types.operation_type(f.bool), Some(ScalarType::BOOL));
    assert_eq!(f.types.operation_type(f.this), Some(ScalarType::Ref));
    assert_eq!(f.types.operation_type(f.program), None);
    assert_eq!(f.types.narrowed_storage_bits(f.int), Some(16));
    assert_eq!(f.types.narrowed_storage_bits(f.dint), None);
    assert_eq!(f.types.narrowed_storage_bits(f.bool), None);
}

/// REQ-LOW-lowering-082: the crate denies wildcard arms over its enums. A test
/// cannot observe a lint that is not configured, so it checks the crate root.
#[test]
fn crate_root_when_read_then_denies_wildcard_enum_match_arm() {
    assert!(include_str!("../lib.rs").contains("#![deny(clippy::wildcard_enum_match_arm)]"));
}
