//! Tests of what a variable's and a field's declaration says of it: its
//! section, its direct address and its retention.

use super::*;

/// `%IX0.1`.
fn input_bit() -> DirectAddress {
    DirectAddress {
        area: AddressArea::Input,
        size: AddressSize::Bit,
        path: vec![0, 1],
    }
}

/// REQ-LOW-lowering-006, REQ-LOW-lowering-023, REQ-LOW-lowering-024: a
/// variable keeps the section, direct address and retention it is declared
/// with.
#[test]
fn push_declared_variable_when_declared_then_variable_keeps_its_declaration() {
    let mut f = fixture();
    let ty = f.ty(f.dint);
    let declared = Declared {
        section: Section::Global,
        location: Some(input_bit()),
        retention: Retention::Retain,
    };

    let var =
        f.b.push_declared_variable(name("g"), Owner::Global, declared, ty)
            .unwrap();

    let variable = f.b.variables().get(var).unwrap();
    assert_eq!(variable.section(), Section::Global);
    assert_eq!(variable.location(), Some(&input_bit()));
    assert_eq!(variable.retention(), Retention::Retain);
}

/// A temporary lowering makes is declared by no one: it is a `Temporary`,
/// with no direct address and no retention.
#[test]
fn push_variable_when_temporary_then_temporary_without_address_or_retention() {
    let mut f = fixture();
    let ty = f.ty(f.dint);

    let var = f.b.push_variable(name("t"), Owner::Global, ty).unwrap();

    let variable = f.b.variables().get(var).unwrap();
    assert_eq!(variable.section(), Section::Temporary);
    assert_eq!(variable.location(), None);
    assert_eq!(variable.retention(), Retention::None);
}

/// A declared variable is refused an owner that is not a POU of the program,
/// as a temporary is.
#[test]
fn push_declared_variable_when_owner_not_a_pou_then_p9998() {
    let mut f = fixture();
    let ty = f.ty(f.dint);
    let foreign = ProgramBuilder::new().push_pou(name("other"));

    let result = f.b.push_declared_variable(
        name("x"),
        Owner::Pou(foreign),
        Declared::in_section(Section::Local),
        ty,
    );

    assert!(is_internal_error_naming(
        result,
        "ProgramBuilder::push_variable"
    ));
}

/// REQ-LOW-lowering-023, REQ-LOW-lowering-024: a field keeps the direct
/// address and retention it is declared with, as a program's located or
/// retained variable is a field of its instance; one declared with neither
/// has neither.
#[test]
fn field_declared_when_address_and_retention_then_field_keeps_them() {
    let f = fixture();
    let ty = f.ty(f.dint);

    let plain = Field::new("x", span(), Some(Section::Local), ty);
    let located = plain
        .clone()
        .declared(Some(input_bit()), Retention::Persistent);

    assert_eq!(plain.location, None);
    assert_eq!(plain.retention, Retention::None);
    assert_eq!(located.location, Some(input_bit()));
    assert_eq!(located.retention, Retention::Persistent);
}
