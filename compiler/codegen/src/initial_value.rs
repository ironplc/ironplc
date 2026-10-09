//! Reads the value a variable starts with from its initializer.
//!
//! The analyzer completes every declaration's initializer with the value
//! the declaration starts with: every field and element is listed, every
//! scalar is a literal of the kind its type stores (see
//! `specs/design/initial-values.md`). Reading that value is mechanical, and
//! that is all this module does: it decides nothing. An initializer whose
//! value slot is empty, or holds a value its type cannot, never reaches
//! codegen from the analyzer, so it is an internal error rather than a
//! default.
//!
//! The value is read into [`InitialValue`], the form the stores are emitted
//! from: a scalar as the number its storage holds, a string as its
//! characters and length, an array nested as its type is.

use std::collections::HashMap;

use ironplc_analyzer::semantic_type::ByteSized;
use ironplc_analyzer::SemanticType;
use ironplc_container::{CharWidth, DEFAULT_STRING_MAX_LENGTH};
use ironplc_dsl::common::*;
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, ExprKind, Variable};
use ironplc_dsl::type_id::TypeId;

/// The value a variable, field or element starts with.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum InitialValue {
    /// A value of an elementary type, an enumeration or a subrange.
    Scalar(ScalarValue),
    /// A `STRING` or `WSTRING`.
    String(StringValue),
    /// A reference: `NULL`, or the variable it refers to.
    Reference(ReferenceValue),
    /// An array's elements, every one of them, in storage order (the last
    /// subscript varies fastest). The elements of an array of arrays are
    /// arrays.
    Array(Vec<InitialValue>),
    /// A structure's fields, or a function block instance's input, output
    /// and internal variables, every one of them, in declaration order.
    Structure(Vec<FieldValue>),
    /// A member of a structure or function block instance initializer that
    /// is an expression evaluated when the variable is initialized
    /// (`(PT := pDevice^.Delta)`), rather than a constant.
    Expression(Box<Expr>),
}

/// A scalar value and the type it is stored as.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScalarValue {
    /// The elementary type whose representation holds the value: the type
    /// itself for an elementary type, the base type of a subrange and the
    /// underlying type of an enumeration.
    pub storage: SemanticType,
    pub value: Scalar,
}

/// A scalar value, in the form its type stores it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Scalar {
    /// A `BOOL`.
    Bool(bool),
    /// An integer, a bit string, an enumeration's ordinal, a duration or a
    /// time of day in milliseconds, or a date or a date and time in seconds
    /// since 1970-01-01. The value is the one the type holds, not a bit
    /// pattern: a `DWORD` of all ones is 4294967295.
    Integer(i128),
    /// A `REAL`.
    Real32(f32),
    /// An `LREAL`.
    Real64(f64),
}

/// A string value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StringValue {
    /// `STRING` (one byte per character) or `WSTRING` (two).
    pub width: StringType,
    /// The most characters the variable holds: the declared length, or the
    /// default length for a string declared without one.
    pub max_length: u16,
    /// The characters the variable starts with.
    pub chars: Vec<char>,
}

/// The value of a reference.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ReferenceValue {
    /// `NULL`: refers to nothing.
    Null,
    /// `REF(x)`: refers to the variable `x`.
    To(Variable),
}

/// The value of one field of a structure or function block instance.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FieldValue {
    /// The field's name, as the initializer spells it.
    pub name: Id,
    pub value: InitialValue,
}

impl Scalar {
    /// Whether every bit of the value is zero. A negative zero real is not.
    pub(crate) fn is_zero(&self) -> bool {
        match self {
            Scalar::Bool(value) => !value,
            Scalar::Integer(value) => *value == 0,
            Scalar::Real32(value) => value.to_bits() == 0,
            Scalar::Real64(value) => value.to_bits() == 0,
        }
    }
}

/// The type of every variable of each user-defined function block, by the
/// block's name and then the variable's, both lower case.
///
/// The representation of a function block in the type environment leaves
/// out the variables whose types it does not lay out (strings, arrays), so
/// the value of an instance is read against the block's declarations.
pub(crate) type BlockMembers = HashMap<String, HashMap<String, SemanticType>>;

/// The types of the variables of the function blocks `blocks`.
pub(crate) fn block_members(
    blocks: &[&FunctionBlockDeclaration],
    types: &HashMap<TypeId, SemanticType>,
) -> BlockMembers {
    blocks
        .iter()
        .map(|block| {
            let members = block
                .variables
                .iter()
                .filter_map(|decl| {
                    let name = decl.identifier.symbolic_id()?.to_string().to_lowercase();
                    Some((name, declared_type(decl, types)?))
                })
                .collect();
            (block.name.name.to_string().to_lowercase(), members)
        })
        .collect()
}

/// The value `decl` starts with, read from its completed initializer.
pub(crate) fn read(
    decl: &VarDecl,
    types: &HashMap<TypeId, SemanticType>,
    blocks: &BlockMembers,
) -> Result<InitialValue, Diagnostic> {
    let span = decl.identifier.span();
    let representation = declared_type(decl, types)
        .ok_or_else(|| missing(&span, "Declaration has no resolved type"))?;
    Reader {
        span: &span,
        blocks,
    }
    .declaration(&decl.initializer, &representation)
}

/// The type `decl` declares. A sized string has the unsized string's type
/// id, so its length is read from its initializer.
fn declared_type(decl: &VarDecl, types: &HashMap<TypeId, SemanticType>) -> Option<SemanticType> {
    match &decl.initializer {
        InitialValueAssignmentKind::String(initializer) => Some(string_type(initializer)),
        _ => decl.type_id.and_then(|id| types.get(&id)).cloned(),
    }
}

/// The representation of the string type a string initializer declares.
fn string_type(initializer: &StringInitializer) -> SemanticType {
    SemanticType::String {
        max_len: initializer
            .length
            .as_ref()
            .and_then(|length| length.as_integer())
            .map(|length| length.value),
        char_width: match initializer.width {
            StringType::String => CharWidth::Narrow,
            StringType::WString => CharWidth::Wide,
        },
    }
}

/// An internal error at `span`: the analyzer completes every initializer,
/// so one this module cannot read is a compiler defect.
fn missing(span: &SourceSpan, message: &str) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(span.clone(), message))
}

/// Reads the values of one declaration, which is where any error points.
struct Reader<'a> {
    span: &'a SourceSpan,
    blocks: &'a BlockMembers,
}

impl Reader<'_> {
    fn error(&self, message: &str) -> Diagnostic {
        missing(self.span, message)
    }

    /// The value the completed `initializer` gives a variable of
    /// `representation`.
    fn declaration(
        &self,
        initializer: &InitialValueAssignmentKind,
        representation: &SemanticType,
    ) -> Result<InitialValue, Diagnostic> {
        use InitialValueAssignmentKind as Kind;
        match (initializer, representation) {
            (
                Kind::String(StringInitializer {
                    initial_value: Some(literal),
                    ..
                }),
                _,
            ) => self.string(literal, representation),
            (
                Kind::Simple(SimpleInitializer {
                    initial_value: Some(constant),
                    ..
                }),
                _,
            ) => self.constant(constant, representation),
            (
                Kind::Subrange(SubrangeInitialValueAssignment {
                    initial_value: Some(value),
                    ..
                }),
                _,
            ) => self.constant(
                &ConstantKind::IntegerLiteral(IntegerLiteral {
                    value: value.clone(),
                    data_type: None,
                }),
                representation,
            ),
            (
                Kind::EnumeratedValues(EnumeratedValuesInitializer {
                    initial_value: Some(value),
                    ..
                })
                | Kind::EnumeratedType(EnumeratedInitialValueAssignment {
                    initial_value: Some(value),
                    ..
                }),
                _,
            ) => self.enumerated(value, representation),
            (
                Kind::Reference(ReferenceInitializer {
                    initial_value: Some(value),
                    ..
                }),
                _,
            ) => Ok(InitialValue::Reference(match value {
                ReferenceInitialValue::Null(_) => ReferenceValue::Null,
                ReferenceInitialValue::Ref(variable) => ReferenceValue::To(variable.clone()),
            })),
            (Kind::Array(array), SemanticType::Array { .. }) => {
                self.array(&array.initial_values, representation)
            }
            (Kind::Structure(structure), _) => {
                self.members(&structure.elements_init, representation)
            }
            (Kind::FunctionBlock(block), _) => self.members(&block.init, representation),
            _ => Err(self.error("Declaration's initializer holds no starting value")),
        }
    }

    /// The value of an array of `representation` whose elements, listed
    /// flat, are `elements`.
    fn array(
        &self,
        elements: &[ArrayInitialElementKind],
        representation: &SemanticType,
    ) -> Result<InitialValue, Diagnostic> {
        let mut counts = Vec::new();
        let mut leaf = representation;
        while let SemanticType::Array { element_type, .. } = leaf {
            let count = leaf
                .array_total_elements()
                .ok_or_else(|| self.error("Array has no element count"))?;
            counts.push(count as usize);
            leaf = element_type;
        }
        let total: usize = counts.iter().product();
        if elements.len() != total {
            return Err(self.error("Array initializer does not list every element"));
        }
        let values = elements
            .iter()
            .map(|element| self.element(element, leaf))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(nest(values, &counts))
    }

    /// The value of one element, of `representation`, of an array.
    fn element(
        &self,
        element: &ArrayInitialElementKind,
        representation: &SemanticType,
    ) -> Result<InitialValue, Diagnostic> {
        match element {
            ArrayInitialElementKind::Constant(constant) => self.constant(constant, representation),
            ArrayInitialElementKind::EnumValue(value) => self.enumerated(value, representation),
            ArrayInitialElementKind::Structure(members) => self.members(members, representation),
            ArrayInitialElementKind::Expression(expr) => Ok(expression(expr, representation)),
            ArrayInitialElementKind::Repeated(_) => {
                Err(self.error("Array initializer has an unexpanded repetition"))
            }
        }
    }

    /// The value of a structure or function block instance of
    /// `representation` whose fields `members` list.
    fn members(
        &self,
        members: &[StructureElementInit],
        representation: &SemanticType,
    ) -> Result<InitialValue, Diagnostic> {
        let fields = representation
            .member_fields()
            .ok_or_else(|| self.error("Member initializer for a type without fields"))?;
        let expected = match representation {
            // An instance holds a value for each field that is not a
            // `VAR_IN_OUT` or `VAR_EXTERNAL`; which those are is the
            // analyzer's to say, so only the names are checked.
            SemanticType::FunctionBlock { .. } => None,
            _ => Some(fields.len()),
        };
        if expected.is_some_and(|count| count != members.len()) {
            return Err(self.error("Structure initializer does not list every field"));
        }
        let values = members
            .iter()
            .map(|member| {
                let field_type = fields
                    .iter()
                    .find(|field| field.name == member.name)
                    .map(|field| &field.field_type)
                    .or_else(|| self.block_member(representation, &member.name))
                    .ok_or_else(|| self.error("Member initializer names no field"))?;
                Ok(FieldValue {
                    name: member.name.clone(),
                    value: self.member(&member.init, field_type)?,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        Ok(InitialValue::Structure(values))
    }

    /// The type of the variable `name` of the user-defined function block
    /// `representation`, as the block declares it.
    fn block_member(&self, representation: &SemanticType, name: &Id) -> Option<&SemanticType> {
        let SemanticType::FunctionBlock { name: block, .. } = representation else {
            return None;
        };
        self.blocks
            .get(&block.to_lowercase())?
            .get(&name.to_string().to_lowercase())
    }

    /// The value one member of a structure initializer gives a field of
    /// `representation`.
    fn member(
        &self,
        init: &StructInitialValueAssignmentKind,
        representation: &SemanticType,
    ) -> Result<InitialValue, Diagnostic> {
        match init {
            StructInitialValueAssignmentKind::Constant(constant) => {
                self.constant(constant, representation)
            }
            StructInitialValueAssignmentKind::EnumeratedValue(value) => {
                self.enumerated(value, representation)
            }
            StructInitialValueAssignmentKind::Array(elements) => {
                self.array(elements, representation)
            }
            StructInitialValueAssignmentKind::Structure(members) => {
                self.members(members, representation)
            }
            StructInitialValueAssignmentKind::Expression(expr) => {
                Ok(expression(expr, representation))
            }
            StructInitialValueAssignmentKind::LateBound(_) => {
                Err(self.error("Member initializer is still late bound"))
            }
        }
    }

    /// The value the literal `constant` gives a place of `representation`.
    fn constant(
        &self,
        constant: &ConstantKind,
        representation: &SemanticType,
    ) -> Result<InitialValue, Diagnostic> {
        if let ConstantKind::CharacterString(literal) = constant {
            return self.string(literal, representation);
        }
        let storage = storage(representation);
        let value = match (constant, storage) {
            (ConstantKind::Boolean(literal), SemanticType::Bool) => {
                Scalar::Bool(literal.value == Boolean::True)
            }
            (
                ConstantKind::IntegerLiteral(literal),
                SemanticType::Int { .. } | SemanticType::UInt { .. } | SemanticType::Bytes { .. },
            ) => Scalar::Integer(
                i128::try_from(literal.value.clone())
                    .map_err(|_| self.error("Integer initial value out of range"))?,
            ),
            (
                ConstantKind::BitStringLiteral(literal),
                SemanticType::Int { .. } | SemanticType::UInt { .. } | SemanticType::Bytes { .. },
            ) => Scalar::Integer(
                i128::try_from(literal.value.value)
                    .map_err(|_| self.error("Integer initial value out of range"))?,
            ),
            (ConstantKind::RealLiteral(literal), SemanticType::Real { size }) => match size {
                ByteSized::B64 => Scalar::Real64(literal.value),
                _ => Scalar::Real32(literal.value as f32),
            },
            (ConstantKind::Duration(literal), SemanticType::Time { .. }) => {
                Scalar::Integer(literal.interval.whole_milliseconds())
            }
            (ConstantKind::TimeOfDay(literal), SemanticType::TimeOfDay { .. }) => {
                Scalar::Integer(i128::from(literal.whole_milliseconds()))
            }
            (ConstantKind::Date(literal), SemanticType::Date { .. }) => {
                Scalar::Integer(i128::from(literal.seconds_since_epoch()))
            }
            (ConstantKind::DateAndTime(literal), SemanticType::DateAndTime { .. }) => {
                Scalar::Integer(i128::from(literal.seconds_since_epoch()))
            }
            _ => return Err(self.error("Initial value is not a literal of its type")),
        };
        Ok(InitialValue::Scalar(ScalarValue {
            storage: storage.clone(),
            value,
        }))
    }

    /// The value the member `value` gives a place of the enumeration
    /// `representation`: its ordinal.
    fn enumerated(
        &self,
        value: &EnumeratedValue,
        representation: &SemanticType,
    ) -> Result<InitialValue, Diagnostic> {
        let ordinal = representation
            .enumeration_members()
            .and_then(|members| members.ordinal_of(&value.value))
            .ok_or_else(|| self.error("Enumerated initial value is not a member"))?;
        Ok(InitialValue::Scalar(ScalarValue {
            storage: storage(representation).clone(),
            value: Scalar::Integer(i128::from(ordinal)),
        }))
    }

    /// The value the string literal `literal` gives a place of the string
    /// type `representation`.
    fn string(
        &self,
        literal: &CharacterStringLiteral,
        representation: &SemanticType,
    ) -> Result<InitialValue, Diagnostic> {
        let SemanticType::String {
            max_len,
            char_width,
        } = representation
        else {
            return Err(self.error("String initial value for a type that is not a string"));
        };
        let max_length = match max_len {
            Some(length) => {
                u16::try_from(*length).map_err(|_| self.error("String length out of range"))?
            }
            None => DEFAULT_STRING_MAX_LENGTH,
        };
        if literal.value.len() > usize::from(max_length) {
            return Err(self.error("String initial value longer than the string"));
        }
        Ok(InitialValue::String(StringValue {
            width: match char_width {
                CharWidth::Narrow => StringType::String,
                CharWidth::Wide => StringType::WString,
            },
            max_length,
            chars: literal.value.clone(),
        }))
    }
}

/// The value an expression member or element gives a place of
/// `representation`: `NULL` for a reference, else the expression, which is
/// evaluated when the variable is initialized.
fn expression(expr: &Expr, representation: &SemanticType) -> InitialValue {
    match (&expr.kind, representation) {
        (ExprKind::Null(_), SemanticType::Reference { .. }) => {
            InitialValue::Reference(ReferenceValue::Null)
        }
        _ => InitialValue::Expression(Box::new(expr.clone())),
    }
}

/// The elementary type a value of `representation` is stored as: the type
/// itself for an elementary type, the base type of a subrange and the
/// underlying type of an enumeration.
fn storage(representation: &SemanticType) -> &SemanticType {
    match representation {
        SemanticType::Subrange { base_type, .. } => storage(base_type),
        SemanticType::Enumeration {
            underlying_type, ..
        } => storage(underlying_type),
        other => other,
    }
}

/// Builds the nested value of an array of arrays from `values` listed flat:
/// `counts` holds the number of elements at each level, outermost first.
fn nest(values: Vec<InitialValue>, counts: &[usize]) -> InitialValue {
    match counts {
        [] | [_] => InitialValue::Array(values),
        [_, inner @ ..] => {
            let size: usize = inner.iter().product();
            let mut elements = Vec::with_capacity(counts[0]);
            let mut values = values.into_iter();
            for _ in 0..counts[0] {
                let chunk: Vec<InitialValue> = values.by_ref().take(size).collect();
                elements.push(nest(chunk, inner));
            }
            InitialValue::Array(elements)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer(value: i128) -> InitialValue {
        InitialValue::Scalar(ScalarValue {
            storage: SemanticType::Int {
                size: ByteSized::B16,
            },
            value: Scalar::Integer(value),
        })
    }

    #[test]
    fn is_zero_when_negative_zero_real_then_false() {
        assert!(!Scalar::Real64(-0.0).is_zero());
        assert!(Scalar::Real32(0.0).is_zero());
    }

    #[test]
    fn nest_when_two_levels_then_inner_arrays() {
        let value = nest(
            vec![integer(1), integer(2), integer(3), integer(4)],
            &[2, 2],
        );

        assert_eq!(
            value,
            InitialValue::Array(vec![
                InitialValue::Array(vec![integer(1), integer(2)]),
                InitialValue::Array(vec![integer(3), integer(4)]),
            ])
        );
    }

    #[test]
    fn read_when_initializer_has_no_value_then_internal_error() {
        let decl = VarDecl::simple("x", "INT");

        assert!(read(&decl, &HashMap::new(), &HashMap::new()).is_err());
    }
}
