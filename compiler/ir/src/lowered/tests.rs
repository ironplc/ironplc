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

fn name(text: &str) -> crate::execution::DebugName {
    crate::execution::DebugName::new(text, span())
}

/// A program type with one `INT` field, the types a test needs, and one
/// variable of each type.
struct Fixture {
    b: ProgramBuilder,
    dint: VarId,
    int: VarId,
    sint: VarId,
    bool: VarId,
    lreal: VarId,
    program: VarId,
    this: VarId,
    reference: VarId,
}

fn fixture() -> Fixture {
    let mut b = ProgramBuilder::new();
    let push = |b: &mut ProgramBuilder, kind| b.push_type(entry(kind), &span()).unwrap();
    let dint = push(
        &mut b,
        TypeKind::Integer {
            width: IntWidth::B32,
            signed: true,
        },
    );
    let int = push(
        &mut b,
        TypeKind::Integer {
            width: IntWidth::B16,
            signed: true,
        },
    );
    let sint = push(
        &mut b,
        TypeKind::Integer {
            width: IntWidth::B8,
            signed: true,
        },
    );
    let bool = push(&mut b, TypeKind::Bool);
    let lreal = push(
        &mut b,
        TypeKind::Real {
            width: WideWidth::B64,
        },
    );
    let main = b.push_pou(name("main"));
    let program = push(
        &mut b,
        TypeKind::Program {
            fields: vec![Field::new("x", span(), Some(Section::Local), int)],
            body: main,
        },
    );
    let this = push(
        &mut b,
        TypeKind::Reference {
            target: program,
            nullability: Nullability::NonNull,
        },
    );
    let reference = push(
        &mut b,
        TypeKind::Reference {
            target: dint,
            nullability: Nullability::Nullable,
        },
    );
    let var = |b: &mut ProgramBuilder, text: &str, ty| {
        b.push_variable(name(text), Owner::Pou(main), ty).unwrap()
    };
    Fixture {
        dint: var(&mut b, "d", dint),
        int: var(&mut b, "i", int),
        sint: var(&mut b, "s", sint),
        bool: var(&mut b, "b", bool),
        lreal: var(&mut b, "r", lreal),
        program: var(&mut b, "p", program),
        this: var(&mut b, "this", this),
        reference: var(&mut b, "ref", reference),
        b,
    }
}

impl Fixture {
    fn place(&self, var: VarId) -> Place {
        self.b.var(var, &span()).unwrap()
    }

    fn read(&self, var: VarId) -> Expr {
        self.b.read(self.place(var), span()).unwrap()
    }

    fn ty(&self, var: VarId) -> TypeId {
        self.b.variables().get(var).unwrap().ty()
    }

    /// The field `x` of the instance `this` refers to.
    fn field(&self) -> Place {
        let program = self.b.deref(self.place(self.this), &span()).unwrap();
        let x = self.b.field_idx(program.ty(), 0).unwrap();
        self.b.field(program, x, &span()).unwrap()
    }
}

/// The operation type of a `DINT`.
const DINT: ScalarType = ScalarType::I32(Signedness::Signed);

fn int(value: i128) -> Expr {
    Expr::integer(value, DINT, span()).unwrap()
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
        Expr::constant(Const::I64(1), DINT, span()),
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
        Expr::integer(1 << 31, DINT, span()),
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
        Expr::real(1.5, DINT, span()),
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
    assert_eq!(sum.ty(), DINT);
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
    assert_eq!(compared.ty(), ScalarType::Bool);
}

/// REQ-LOW-lowering-044: a conversion changes type.
#[test]
fn convert_when_same_type_then_internal_error() {
    assert!(is_internal_error_naming(
        Expr::convert(int(1), DINT, span()),
        "Expr::convert"
    ));
    let widened = Expr::convert(int(1), ScalarType::I64(Signedness::Signed), span()).unwrap();
    assert_eq!(widened.ty(), ScalarType::I64(Signedness::Signed));
}

/// REQ-LOW-lowering-163: a checked conversion narrows an integer to a type
/// that does not hold every value of the operand's, and prints as one.
#[test]
fn convert_checked_when_type_may_not_hold_value_then_checked_and_otherwise_internal_error() {
    let lint = Expr::convert(int(1), ScalarType::I64(Signedness::Signed), span()).unwrap();
    let checked = Expr::convert_checked(lint, DINT, span()).unwrap();
    assert_eq!(checked.ty(), DINT);
    assert_eq!(
        checked.to_string(),
        "(convert_checked i32s (convert i64s (const i32s 1)))"
    );
    let udint = Expr::convert(int(1), ScalarType::I32(Signedness::Unsigned), span()).unwrap();
    assert!(Expr::convert_checked(udint, DINT, span()).is_ok());

    assert!(is_internal_error_naming(
        Expr::convert_checked(int(1), ScalarType::I64(Signedness::Signed), span()),
        "Expr::convert_checked"
    ));
    assert!(is_internal_error_naming(
        Expr::convert_checked(int(1), DINT, span()),
        "Expr::convert_checked"
    ));
    let real = Expr::real(1.0, ScalarType::F64, span()).unwrap();
    assert!(is_internal_error_naming(
        Expr::convert_checked(real, DINT, span()),
        "Expr::convert_checked"
    ));
}

/// REQ-LOW-lowering-047: a read has the operation type of the place's type.
#[test]
fn read_when_place_is_int_then_operation_type_of_int() {
    let f = fixture();
    assert_eq!(f.read(f.int).ty(), ScalarType::I32(Signedness::Signed));
}

/// REQ-LOW-lowering-047: an aggregate is not read as one value.
#[test]
fn read_when_place_is_program_then_internal_error() {
    let f = fixture();
    assert!(is_internal_error_naming(
        f.b.read(f.place(f.program), span()),
        "Expr::read"
    ));
}

/// A place rooted at a variable takes its type from the variable table.
#[test]
fn place_when_variable_then_type_of_variable() {
    let f = fixture();
    assert_eq!(f.place(f.int).ty(), f.ty(f.int));
}

/// A variable another table allocated is not a place of this one.
#[test]
fn place_when_variable_not_in_table_then_internal_error() {
    let f = fixture();
    assert!(is_internal_error_naming(
        ProgramBuilder::new().var(f.int, &span()),
        "Place::var"
    ));
}

/// REQ-LOW-lowering-026: a variable's type is in the type table.
#[test]
fn variable_when_type_not_in_table_then_internal_error() {
    let f = fixture();
    let result = ProgramBuilder::new().push_variable(name("x"), Owner::Global, f.ty(f.int));
    assert!(is_internal_error_naming(result, "VariableTable::push"));
}

/// REQ-LOW-lowering-030, REQ-LOW-lowering-032: a field is reached through a
/// dereference of the instance parameter.
#[test]
fn place_when_field_through_deref_then_type_of_field() {
    let f = fixture();
    let field = f.field();
    assert_eq!(field.ty(), f.ty(f.int));
    assert_eq!(field.to_string(), "v6^.f0");
}

/// REQ-LOW-lowering-030: a field index names a field of the type, and only
/// the type table makes one.
#[test]
fn field_idx_when_position_outside_type_then_none() {
    let f = fixture();
    let program = f.ty(f.program);
    assert!(f.b.field_idx(program, 0).is_some());
    assert!(f.b.field_idx(program, 1).is_none());
    let dint = f.ty(f.dint);
    assert!(f.b.field_idx(dint, 0).is_none());
}

/// REQ-LOW-lowering-030: a field index of one type does not name a field of
/// another.
#[test]
fn place_when_field_of_another_type_then_internal_error() {
    let f = fixture();
    let program = f.ty(f.program);
    let x = f.b.field_idx(program, 0).unwrap();
    assert!(is_internal_error_naming(
        f.b.field(f.place(f.dint), x, &span()),
        "Place::field"
    ));
}

/// REQ-LOW-lowering-032: only a reference is dereferenced.
#[test]
fn place_when_deref_of_non_reference_then_internal_error() {
    let f = fixture();
    assert!(is_internal_error_naming(
        f.b.deref(f.place(f.program), &span()),
        "Place::deref"
    ));
}

/// REQ-LOW-lowering-060: an assigned value has the place's operation type.
#[test]
fn assign_when_value_type_differs_from_place_then_internal_error() {
    let f = fixture();
    assert!(is_internal_error_naming(
        f.b.assign(f.place(f.lreal), int(1), span()),
        "Stmt::assign"
    ));
}

/// REQ-LOW-lowering-046: a value stored narrower than its operation width is
/// truncated to the place's width, a constant within its range, or a read
/// of a place whose range lies within it.
#[test]
fn assign_when_narrow_place_then_value_must_fit() {
    let f = fixture();
    let assign = |value| f.b.assign(f.place(f.int), value, span());

    let sum = Expr::binary(BinaryOp::Add, int(1), int(2), span()).unwrap();
    assert!(is_internal_error_naming(
        assign(sum.clone()),
        "Stmt::assign"
    ));
    let to_byte = Expr::truncate(NarrowWidth::B8, sum.clone(), span()).unwrap();
    assert!(assign(to_byte).is_err());
    let truncated = Expr::truncate(NarrowWidth::B16, sum, span()).unwrap();
    assert!(assign(truncated).is_ok());

    assert!(assign(int(32767)).is_ok());
    assert!(assign(int(32768)).is_err());

    assert!(assign(f.read(f.sint)).is_ok());
    assert!(assign(f.read(f.dint)).is_err());
}

/// REQ-LOW-lowering-046: a `BOOL` is never narrowed.
#[test]
fn assign_when_bool_place_then_comparison_stored_as_is() {
    let f = fixture();
    let compared = Expr::compare(CompareOp::Eq, int(1), int(2), span()).unwrap();
    assert!(f.b.assign(f.place(f.bool), compared, span()).is_ok());
    assert!(f.b.assign(f.place(f.bool), int(1), span()).is_err());
}

/// REQ-LOW-lowering-036: a non-null reference is never assigned.
#[test]
fn assign_when_non_null_reference_then_internal_error() {
    let f = fixture();
    let source = f.read(f.reference);
    assert!(is_internal_error_naming(
        f.b.assign(f.place(f.this), source.clone(), span()),
        "Stmt::assign"
    ));
    assert!(f.b.assign(f.place(f.reference), source, span()).is_ok());
}

/// References are compared for equality only.
#[test]
fn compare_when_references_ordered_then_internal_error() {
    let f = fixture();
    let read = || f.read(f.reference);
    assert!(Expr::compare(CompareOp::Eq, read(), read(), span()).is_ok());
    assert!(Expr::compare(CompareOp::Ne, read(), read(), span()).is_ok());
    assert!(is_internal_error_naming(
        Expr::compare(CompareOp::Lt, read(), read(), span()),
        "Expr::compare"
    ));
}

/// A reference to a place and the null reference are values of type `Ref`,
/// compared for equality and stored in a nullable reference; neither is
/// stored in a non-null one (REQ-LOW-lowering-036).
#[test]
fn ref_to_and_null_when_stored_then_nullable_reference_only() {
    let f = fixture();
    let to_d = Expr::ref_to(f.place(f.dint), span());
    let null = Expr::null(span());
    assert_eq!(to_d.ty(), ScalarType::Ref);
    assert_eq!(null.ty(), ScalarType::Ref);
    assert_eq!(to_d.to_string(), "(ref_to v0)");
    assert_eq!(null.to_string(), "(null)");
    assert!(Expr::compare(CompareOp::Eq, f.read(f.reference), null.clone(), span()).is_ok());
    assert!(f.b.assign(f.place(f.reference), to_d, span()).is_ok());
    assert!(f
        .b
        .assign(f.place(f.reference), null.clone(), span())
        .is_ok());
    assert!(is_internal_error_naming(
        f.b.assign(f.place(f.this), null, span()),
        "Stmt::assign"
    ));
}

/// REQ-LOW-lowering-048: the logical forms apply only to a `BOOL`, and the
/// bitwise forms only to an integer.
#[test]
fn unary_when_not_of_wrong_type_then_internal_error() {
    let truth = Expr::boolean(true, span());
    assert!(Expr::unary(UnaryOp::BoolNot, truth.clone(), span()).is_ok());
    assert!(is_internal_error_naming(
        Expr::unary(UnaryOp::BitNot, truth, span()),
        "Expr::unary"
    ));
    assert!(Expr::unary(UnaryOp::BoolNot, int(1), span()).is_err());
    assert!(Expr::unary(UnaryOp::BitNot, int(1), span()).is_ok());
}

/// REQ-LOW-lowering-048: `AND`, `OR` and `XOR` are logical on a `BOOL` and
/// bitwise on an integer, and never the other way round.
#[test]
fn binary_when_logical_operator_of_wrong_type_then_internal_error() {
    let truth = || Expr::boolean(true, span());
    assert!(Expr::binary(BinaryOp::BoolAnd, truth(), truth(), span()).is_ok());
    assert!(Expr::binary(BinaryOp::BitAnd, truth(), truth(), span()).is_err());
    assert!(is_internal_error_naming(
        Expr::binary(BinaryOp::BoolOr, int(1), int(2), span()),
        "Expr::binary"
    ));
    assert!(Expr::binary(BinaryOp::BitOr, int(1), int(2), span()).is_ok());
    assert!(Expr::binary(BinaryOp::Add, truth(), truth(), span()).is_err());
}

/// A `BOOL` is a type of its own: it is neither an integer constant nor
/// converted to a number or from a real.
#[test]
fn boolean_when_used_as_integer_then_internal_error() {
    assert_eq!(Expr::boolean(true, span()).to_string(), "(const bool true)");
    assert!(Expr::integer(1, ScalarType::Bool, span()).is_err());
    assert!(Expr::convert(int(1), ScalarType::Bool, span()).is_err());
    assert!(Expr::convert(Expr::boolean(true, span()), ScalarType::F64, span()).is_err());
    assert!(Expr::truncate(NarrowWidth::B8, Expr::boolean(true, span()), span()).is_err());
}

/// `BOOL_TO_INT` converts a `BOOL` to an integer.
#[test]
fn convert_when_boolean_to_integer_then_integer() {
    let converted = Expr::convert(Expr::boolean(true, span()), DINT, span()).unwrap();
    assert_eq!(converted.ty(), DINT);
    assert_eq!(converted.to_string(), "(convert i32s (const bool true))");
}

#[test]
fn truncate_when_not_narrower_then_internal_error() {
    assert!(is_internal_error_naming(
        Expr::truncate(NarrowWidth::B32, int(1), span()),
        "Expr::truncate"
    ));
    let wide = Expr::integer(1, ScalarType::I64(Signedness::Signed), span()).unwrap();
    assert!(Expr::truncate(NarrowWidth::B32, wide, span()).is_ok());
    let real = Expr::real(1.0, ScalarType::F32, span()).unwrap();
    assert!(Expr::truncate(NarrowWidth::B16, real, span()).is_err());
}

#[test]
fn print_when_assignment_then_s_expression() {
    let f = fixture();
    let field = f.field();
    let sum = Expr::binary(
        BinaryOp::Add,
        f.b.read(field.clone(), span()).unwrap(),
        int(1),
        span(),
    )
    .unwrap();
    let truncated = Expr::truncate(NarrowWidth::B16, sum, span()).unwrap();
    let stmt = f.b.assign(field, truncated, span()).unwrap();
    assert_eq!(
        stmt.to_string(),
        "assign v6^.f0 (truncate 16 (add i32s (read i32s v6^.f0) (const i32s 1)))"
    );
}

#[test]
fn type_table_when_operation_types_then_follow_adr_0001() {
    let f = fixture();
    let ty = |var| f.ty(var);
    let operated = |var| f.b.types().operation_type(ty(var));
    assert_eq!(operated(f.sint), Some(ScalarType::I32(Signedness::Signed)));
    assert_eq!(operated(f.lreal), Some(ScalarType::F64));
    assert_eq!(operated(f.bool), Some(ScalarType::Bool));
    assert_eq!(operated(f.this), Some(ScalarType::Ref));
    assert_eq!(operated(f.program), None);
    assert_eq!(
        f.b.types().narrowed_storage(ty(f.int)),
        Some(NarrowWidth::B16)
    );
    assert_eq!(f.b.types().narrowed_storage(ty(f.dint)), None);
    assert_eq!(f.b.types().narrowed_storage(ty(f.bool)), None);
}

/// A type refers only to types already in the table.
#[test]
fn type_table_when_reference_to_unknown_type_then_internal_error() {
    let f = fixture();
    let mut other = ProgramBuilder::new();
    let target = f.ty(f.program);
    let result = other.push_type(
        entry(TypeKind::Reference {
            target,
            nullability: Nullability::Nullable,
        }),
        &span(),
    );
    assert!(is_internal_error_naming(result, "TypeTable::push"));
}

/// A subrange lies within an integer base type.
#[test]
fn type_table_when_subrange_outside_base_then_internal_error() {
    let mut f = fixture();
    let int = f.ty(f.int);
    let lreal = f.ty(f.lreal);
    let mut subrange = |base, min, max| {
        f.b.push_type(entry(TypeKind::Subrange { base, min, max }), &span())
    };
    assert!(subrange(int, 0, 100).is_ok());
    assert!(is_internal_error_naming(
        subrange(int, 0, 40_000),
        "TypeTable::push"
    ));
    assert!(subrange(int, 5, 1).is_err());
    assert!(subrange(lreal, 0, 1).is_err());
}

/// REQ-LOW-ir-192: an enumeration's underlying type is an integer or a bit
/// string type in the table.
#[test]
fn type_table_when_enumeration_underlying_not_integer_then_internal_error() {
    let mut f = fixture();
    let int = f.ty(f.int);
    let bool = f.ty(f.bool);
    let lreal = f.ty(f.lreal);
    let word =
        f.b.push_type(
            entry(TypeKind::BitString {
                width: IntWidth::B16,
            }),
            &span(),
        )
        .unwrap();
    let mut enumeration = |underlying| {
        f.b.push_type(
            entry(TypeKind::Enumeration {
                underlying,
                values: vec![],
            }),
            &span(),
        )
    };
    assert!(enumeration(int).is_ok());
    assert!(enumeration(word).is_ok());
    assert!(is_internal_error_naming(
        enumeration(bool),
        "TypeTable::push"
    ));
    assert!(enumeration(lreal).is_err());
}

/// An array has dimensions, each with its lower bound at most its upper.
#[test]
fn type_table_when_array_without_dimensions_then_internal_error() {
    let mut f = fixture();
    let element = f.ty(f.dint);
    let mut array = |dimensions| {
        f.b.push_type(
            entry(TypeKind::Array {
                element,
                dimensions,
            }),
            &span(),
        )
    };
    assert!(array(vec![(1, 10)]).is_ok());
    assert!(array(vec![]).is_err());
    assert!(array(vec![(10, 1)]).is_err());
}

/// A structure's fields have no section, and a program's each have one.
#[test]
fn type_table_when_field_section_wrong_then_internal_error() {
    let mut f = fixture();
    let int = f.ty(f.int);
    let field = |section| vec![Field::new("x", span(), section, int)];
    assert!(f
        .b
        .push_type(
            entry(TypeKind::Structure {
                fields: field(None)
            }),
            &span()
        )
        .is_ok());
    assert!(is_internal_error_naming(
        f.b.push_type(
            entry(TypeKind::Structure {
                fields: field(Some(Section::Local))
            }),
            &span()
        ),
        "TypeTable::push"
    ));
}

/// An id from one program names nothing in another, even at the same
/// position.
#[test]
fn ids_when_from_another_program_then_name_nothing() {
    let f = fixture();
    let other = fixture();
    assert!(f.b.types().get(other.ty(other.int)).is_none());
    assert!(f.b.variables().get(other.int).is_none());
    assert!(is_internal_error_naming(
        f.b.var(other.int, &span()),
        "Place::var"
    ));
}

/// An expression reads the places of one program only.
#[test]
fn binary_when_operands_from_two_programs_then_internal_error() {
    let f = fixture();
    let other = fixture();
    assert!(is_internal_error_naming(
        Expr::binary(
            BinaryOp::Add,
            f.read(f.dint),
            other.read(other.dint),
            span()
        ),
        "Expr::binary"
    ));
    assert!(Expr::binary(BinaryOp::Add, f.read(f.dint), int(1), span()).is_ok());
}

/// A value read from another program is not stored in this one.
#[test]
fn assign_when_value_from_another_program_then_internal_error() {
    let f = fixture();
    let other = fixture();
    assert!(is_internal_error_naming(
        f.b.assign(f.place(f.dint), other.read(other.dint), span()),
        "Stmt::assign"
    ));
}

/// A field id of one program does not name a field in another.
#[test]
fn field_when_field_idx_from_another_program_then_internal_error() {
    let f = fixture();
    let other = fixture();
    let program = other.b.deref(other.place(other.this), &span()).unwrap();
    let x = other.b.field_idx(program.ty(), 0).unwrap();
    let here = f.b.deref(f.place(f.this), &span()).unwrap();
    assert!(is_internal_error_naming(
        f.b.field(here, x, &span()),
        "Place::field"
    ));
}

/// A program type's body, and a variable's owner, are POUs of the same
/// program.
#[test]
fn builder_when_pou_from_another_program_then_internal_error() {
    let mut f = fixture();
    let mut other = ProgramBuilder::new();
    let foreign = other.push_pou(name("elsewhere"));
    let result = f.b.push_type(
        entry(TypeKind::Program {
            fields: vec![],
            body: foreign,
        }),
        &span(),
    );
    assert!(is_internal_error_naming(
        result,
        "ProgramBuilder::push_type"
    ));
    let ty = f.ty(f.dint);
    assert!(is_internal_error_naming(
        f.b.push_variable(name("x"), Owner::Pou(foreign), ty),
        "ProgramBuilder::push_variable"
    ));
}

/// REQ-LOW-lowering-082: the crate denies wildcard arms over its enums. A test
/// cannot observe a lint that is not configured, so it checks the crate root.
#[test]
fn crate_root_when_read_then_denies_wildcard_enum_match_arm() {
    assert!(include_str!("../lib.rs").contains("#![deny(clippy::wildcard_enum_match_arm)]"));
}

mod bits;

mod calls;

mod control;

mod copies;

mod declarations;

mod declared_types;

mod fb_calls;

mod in_place;

mod methods;

mod places;

mod strings;

mod whole_program;
