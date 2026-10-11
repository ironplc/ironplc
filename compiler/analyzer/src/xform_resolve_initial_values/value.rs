//! The value a place starts with, as the resolver builds it, and the syntax
//! that writes it back into a declaration.
//!
//! A [`Value`] is made of the same leaves an initializer is: literals,
//! enumerated values, `NULL`, expressions. It exists only while the pass
//! runs. Once a declaration's value is complete, it replaces the
//! declaration's initializer and nothing else keeps it, so a declaration
//! never holds two answers to what it starts with.
//!
//! The parts of a value the program did not write at the declaration --
//! the default of a field the initializer leaves out, the elements an array
//! initializer does not reach -- have synthesized spans (see
//! [`Value::synthesized`]), so that a renderer showing the program as
//! written can leave them out.

use ironplc_dsl::common::*;
use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::textual::{Expr, ExprKind};

/// The value a variable, field or element starts with.
#[derive(Clone, Debug)]
pub(crate) enum Value {
    /// A literal of the kind its type stores: `TRUE` or `FALSE` for a
    /// `BOOL`, an integer for an integer, bit string or subrange, a real
    /// for a `REAL` or `LREAL`, a string of the variable's width for a
    /// string, and a literal of the type's family for a duration, date or
    /// time of day.
    Constant(ConstantKind),
    /// A member of an enumeration.
    Enumerated(EnumeratedValue),
    /// The value of a reference: `NULL`, or the variable it refers to.
    Reference(ReferenceInitialValue),
    /// A member of a structure or function block instance initializer that
    /// is evaluated when the variable is initialized (`(PT := p^.Delta)`)
    /// rather than a constant. Only reachable with
    /// `--allow-struct-initializer-expressions`.
    Expression(Expr),
    /// An array's elements, every one of them, listed flat in storage order
    /// (the last subscript varies fastest), whatever the array's shape: an
    /// array of arrays lists the elements of its innermost arrays, as an
    /// initializer does. No element is itself an `Array`.
    Array(Vec<Value>),
    /// A structure's fields, or a function block instance's input, output
    /// and internal variables, every one of them, in declaration order.
    Structure(Vec<Field>),
}

/// The value of one field of a structure or function block instance.
#[derive(Clone, Debug)]
pub(crate) struct Field {
    /// The field's name: as the initializer spells it when the initializer
    /// names the field, and synthesized when it does not.
    pub name: Id,
    pub value: Value,
}

impl Value {
    /// This value as one the compiler supplied rather than the program
    /// wrote at the declaration: every literal, enumerated value, `NULL` and
    /// field name in it gets a synthesized span.
    pub(crate) fn synthesized(self) -> Value {
        match self {
            Value::Constant(constant) => {
                Value::Constant(constant.with_span(SourceSpan::synthesized()))
            }
            Value::Enumerated(value) => Value::Enumerated(EnumeratedValue {
                type_name: value.type_name.map(|type_name| TypeName {
                    name: type_name.name.with_position(SourceSpan::synthesized()),
                }),
                value: value.value.with_position(SourceSpan::synthesized()),
                explicit_value: None,
            }),
            Value::Reference(ReferenceInitialValue::Null(_)) => {
                Value::Reference(ReferenceInitialValue::Null(SourceSpan::synthesized()))
            }
            // A variable reference and an expression keep the spans of what
            // they refer to; the field that holds them says they were
            // supplied.
            Value::Reference(ReferenceInitialValue::Ref(_)) | Value::Expression(_) => self,
            Value::Array(values) => {
                Value::Array(values.into_iter().map(Value::synthesized).collect())
            }
            Value::Structure(fields) => Value::Structure(
                fields
                    .into_iter()
                    .map(|field| Field {
                        name: field.name.with_position(SourceSpan::synthesized()),
                        value: field.value.synthesized(),
                    })
                    .collect(),
            ),
        }
    }

    /// The value as an element of an array initializer. `None` for an
    /// array, which an element never is: arrays are listed flat.
    fn into_element(self) -> Option<ArrayInitialElementKind> {
        Some(match self {
            Value::Constant(constant) => ArrayInitialElementKind::Constant(constant),
            Value::Enumerated(value) => ArrayInitialElementKind::EnumValue(value),
            Value::Reference(reference) => {
                ArrayInitialElementKind::Expression(reference_expression(reference))
            }
            Value::Expression(expr) => ArrayInitialElementKind::Expression(expr),
            Value::Structure(fields) => ArrayInitialElementKind::Structure(members(fields)?),
            Value::Array(_) => return None,
        })
    }

    /// The value as the value of a member of a structure initializer.
    fn into_member(self) -> Option<StructInitialValueAssignmentKind> {
        Some(match self {
            Value::Constant(constant) => StructInitialValueAssignmentKind::Constant(constant),
            Value::Enumerated(value) => StructInitialValueAssignmentKind::EnumeratedValue(value),
            Value::Reference(reference) => {
                StructInitialValueAssignmentKind::Expression(reference_expression(reference))
            }
            Value::Expression(expr) => StructInitialValueAssignmentKind::Expression(expr),
            Value::Array(values) => StructInitialValueAssignmentKind::Array(elements(values)?),
            Value::Structure(fields) => {
                StructInitialValueAssignmentKind::Structure(members(fields)?)
            }
        })
    }
}

/// `values`, the elements of an array, as an array initializer.
fn elements(values: Vec<Value>) -> Option<Vec<ArrayInitialElementKind>> {
    values.into_iter().map(Value::into_element).collect()
}

/// `fields`, the fields of a structure, as the members of a structure
/// initializer.
fn members(fields: Vec<Field>) -> Option<Vec<StructureElementInit>> {
    fields
        .into_iter()
        .map(|field| {
            Some(StructureElementInit {
                name: field.name,
                init: field.value.into_member()?,
            })
        })
        .collect()
}

/// A reference value as the expression a member or element holds it as.
fn reference_expression(reference: ReferenceInitialValue) -> Expr {
    match reference {
        ReferenceInitialValue::Null(span) => Expr::new(ExprKind::Null(span)),
        ReferenceInitialValue::Ref(variable) => Expr::new(ExprKind::Ref(Box::new(variable))),
    }
}

/// `initializer`, a declaration's initializer, completed with `value`, the
/// value the declaration starts with. The initializer keeps its kind and
/// the type it names; its value slot holds `value`, complete down to every
/// field and element.
///
/// `None` when the initializer has no slot for a value of that shape, which
/// a well-typed declaration never has.
pub(super) fn complete(
    initializer: &InitialValueAssignmentKind,
    value: Value,
) -> Option<InitialValueAssignmentKind> {
    use InitialValueAssignmentKind as Kind;
    Some(match (initializer, value) {
        (Kind::Simple(simple), Value::Constant(constant)) => Kind::Simple(SimpleInitializer {
            type_name: simple.type_name.clone(),
            initial_value: Some(constant),
        }),
        (Kind::String(string), Value::Constant(ConstantKind::CharacterString(literal))) => {
            Kind::String(StringInitializer {
                initial_value: Some(literal),
                ..string.clone()
            })
        }
        (Kind::EnumeratedValues(values), Value::Enumerated(value)) => {
            Kind::EnumeratedValues(EnumeratedValuesInitializer {
                values: values.values.clone(),
                initial_value: Some(value),
            })
        }
        (Kind::EnumeratedType(enumerated), Value::Enumerated(value)) => {
            Kind::EnumeratedType(EnumeratedInitialValueAssignment {
                type_name: enumerated.type_name.clone(),
                initial_value: Some(value),
            })
        }
        (Kind::Subrange(subrange), Value::Constant(constant)) => {
            Kind::Subrange(SubrangeInitialValueAssignment {
                spec: subrange.spec.clone(),
                initial_value: Some(signed_integer(constant)?),
            })
        }
        (Kind::Reference(reference), Value::Reference(value)) => {
            Kind::Reference(ReferenceInitializer {
                target: reference.target.clone(),
                initial_value: Some(value),
                syntax: reference.syntax,
            })
        }
        (Kind::Array(array), Value::Array(values)) => Kind::Array(ArrayInitialValueAssignment {
            spec: array.spec.clone(),
            initial_values: elements(values)?,
        }),
        (Kind::Structure(structure), Value::Structure(fields)) => {
            Kind::Structure(StructureInitializationDeclaration {
                type_name: structure.type_name.clone(),
                elements_init: members(fields)?,
            })
        }
        (Kind::FunctionBlock(block), Value::Structure(fields)) => {
            Kind::FunctionBlock(FunctionBlockInitialValueAssignment {
                type_name: block.type_name.clone(),
                init: members(fields)?,
            })
        }
        _ => return None,
    })
}

/// The initializer of a variable of the type `type_name` that starts with
/// `value`, for a variable the program does not declare: a function's or
/// method's result. `shape` says which kind of initializer the type takes.
pub(super) fn initializer_of(
    type_name: &TypeName,
    shape: Shape,
    value: Value,
) -> Option<InitialValueAssignmentKind> {
    use InitialValueAssignmentKind as Kind;
    let bare = match shape {
        Shape::Simple => Kind::Simple(SimpleInitializer {
            type_name: type_name.clone(),
            initial_value: None,
        }),
        Shape::Enumerated => Kind::EnumeratedType(EnumeratedInitialValueAssignment {
            type_name: type_name.clone(),
            initial_value: None,
        }),
        Shape::Subrange => Kind::Subrange(SubrangeInitialValueAssignment::bare(
            SpecificationKind::Named(type_name.clone()),
        )),
        Shape::Reference(target) => Kind::Reference(ReferenceInitializer {
            target: ReferenceTarget::Named(target),
            initial_value: None,
            syntax: RefSyntax::RefTo,
        }),
        Shape::Array => Kind::Array(ArrayInitialValueAssignment {
            spec: SpecificationKind::Named(type_name.clone()),
            initial_values: vec![],
        }),
        Shape::Structure => Kind::Structure(StructureInitializationDeclaration {
            type_name: type_name.clone(),
            elements_init: vec![],
        }),
        Shape::FunctionBlock => Kind::FunctionBlock(FunctionBlockInitialValueAssignment {
            type_name: type_name.clone(),
            init: vec![],
        }),
    };
    complete(&bare, value)
}

/// Which kind of initializer a variable of a type takes.
pub(super) enum Shape {
    /// An elementary type, a string type or an alias of one: a constant.
    Simple,
    Enumerated,
    Subrange,
    /// A reference to the type named.
    Reference(TypeName),
    Array,
    Structure,
    FunctionBlock,
}

/// An integer constant as the signed integer a subrange initializer holds.
fn signed_integer(constant: ConstantKind) -> Option<SignedInteger> {
    match constant {
        ConstantKind::IntegerLiteral(literal) => Some(literal.value),
        ConstantKind::BitStringLiteral(literal) => Some(SignedInteger {
            value: literal.value,
            is_neg: false,
        }),
        _ => None,
    }
}
