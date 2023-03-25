//! Provides definitions of objects from IEC 61131-3 common elements.
//!
//! See section 2.
use core::str::FromStr;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::num::TryFromIntError;
use time::Duration;

use crate::core::{Id, SourceLoc, SourcePosition};
use crate::sfc::Network;
use crate::textual::*;

/// Numeric liberals declared by 2.2.1. Numeric literals define
/// how data is expressed and are distinct from but associated with
/// data types.

/// Integer liberal. The representation is of the largest possible integer
/// and later bound to smaller types depend on context.
#[derive(Debug, Clone, PartialEq)]
pub struct Integer {
    pub position: SourceLoc,
    /// The value in the maximum possible size. An integer is inherently
    /// an unsigned value.
    pub value: u128,
}

#[derive(Debug)]
pub struct TryFromIntegerError();

impl TryFrom<Integer> for u8 {
    type Error = TryFromIntegerError;
    fn try_from(value: Integer) -> Result<u8, Self::Error> {
        value.value.try_into().map_err(|e| TryFromIntegerError {})
    }
}

impl TryFrom<Integer> for u32 {
    type Error = TryFromIntegerError;
    fn try_from(value: Integer) -> Result<u32, Self::Error> {
        value.value.try_into().map_err(|e| TryFromIntegerError {})
    }
}

impl TryFrom<Integer> for i128 {
    type Error = TryFromIntegerError;
    fn try_from(value: Integer) -> Result<i128, Self::Error> {
        value.value.try_into().map_err(|e| TryFromIntegerError {})
    }
}

impl TryFrom<Integer> for f64 {
    type Error = TryFromIntegerError;
    fn try_from(value: Integer) -> Result<f64, Self::Error> {
        let res: Result<u32, _> = value.value.try_into();
        let val = res.map_err(|e| TryFromIntegerError {})?;

        let res: Result<f64, _> = val.try_into();
        res.map_err(|e| TryFromIntegerError {})
    }
}

impl TryFrom<Integer> for f32 {
    type Error = TryFromIntegerError;
    fn try_from(value: Integer) -> Result<f32, Self::Error> {
        let res: Result<u32, _> = value.value.try_into();
        let val = res.map_err(|e| TryFromIntegerError {})?;

        let res: Result<f64, _> = val.try_into();
        let val = res.map_err(|e| TryFromIntegerError {})?;

        // TODO how to do this
        let val: f32 = val as f32;
        Ok(val)
    }
}

impl Integer {
    pub fn new(a: &str, position: SourceLoc) -> Integer {
        let without_underscore: String = a.chars().filter(|c| c.is_ascii_digit()).collect();
        without_underscore
            .as_str()
            .parse::<u128>()
            .map(|value| Integer { position, value })
            .unwrap()
    }

    pub fn hex(a: &str, position: SourceLoc) -> Integer {
        let without_underscore: String = a.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        u128::from_str_radix(without_underscore.as_str(), 16)
            .map(|value| Integer { position, value })
            .unwrap()
    }

    pub fn octal(a: &str, position: SourceLoc) -> Integer {
        let without_underscore: String = a.chars().filter(|c| matches!(c, '0'..='7')).collect();
        u128::from_str_radix(without_underscore.as_str(), 8)
            .map(|value| Integer { position, value })
            .unwrap()
    }

    pub fn binary(a: &str, position: SourceLoc) -> Integer {
        let without_underscore: String = a.chars().filter(|c| matches!(c, '0'..='1')).collect();
        u128::from_str_radix(without_underscore.as_str(), 2)
            .map(|value| Integer { position, value })
            .unwrap()
    }
}

impl fmt::Display for Integer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}", self.value))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SignedInteger {
    pub value: Integer,
    pub is_neg: bool,
}

impl SignedInteger {
    pub fn new(a: &str, position: SourceLoc) -> SignedInteger {
        match a.chars().next() {
            Some('+') => SignedInteger {
                value: Integer::new(a.get(1..).unwrap(), position),
                is_neg: false,
            },
            Some('-') => SignedInteger {
                value: Integer::new(a.get(1..).unwrap(), position),
                is_neg: true,
            },
            _ => SignedInteger {
                value: Integer::new(a, position),
                is_neg: false,
            },
        }
    }
}

impl From<Integer> for SignedInteger {
    fn from(value: Integer) -> SignedInteger {
        SignedInteger {
            value,
            is_neg: false,
        }
    }
}

impl TryFrom<SignedInteger> for u8 {
    type Error = TryFromIntegerError;
    fn try_from(value: SignedInteger) -> Result<u8, Self::Error> {
        value.value.try_into().map_err(|e| TryFromIntegerError {})
    }
}

impl TryFrom<SignedInteger> for u32 {
    type Error = TryFromIntegerError;
    fn try_from(value: SignedInteger) -> Result<u32, Self::Error> {
        value.value.try_into().map_err(|e| TryFromIntegerError {})
    }
}

impl TryFrom<SignedInteger> for i128 {
    type Error = TryFromIntegerError;
    fn try_from(value: SignedInteger) -> Result<i128, Self::Error> {
        let mut primitive = value.value.try_into().map_err(|e| TryFromIntegerError {})?;
        if value.is_neg {
            primitive *= -1;
        }
        Ok(primitive)
    }
}

impl TryFrom<SignedInteger> for f64 {
    type Error = TryFromIntegerError;
    fn try_from(value: SignedInteger) -> Result<f64, Self::Error> {
        let res: Result<u32, _> = value.value.try_into();
        let val = res.map_err(|e| TryFromIntegerError {})?;

        let res: Result<f64, _> = val.try_into();
        res.map_err(|e| TryFromIntegerError {})
    }
}

impl TryFrom<SignedInteger> for f32 {
    type Error = TryFromIntegerError;
    fn try_from(value: SignedInteger) -> Result<f32, Self::Error> {
        let res: Result<u32, _> = value.value.try_into();
        let val = res.map_err(|e| TryFromIntegerError {})?;

        let res: Result<f64, _> = val.try_into();
        let val = res.map_err(|e| TryFromIntegerError {})?;

        // TODO how to do this
        let val: f32 = val as f32;
        Ok(val)
    }
}

impl fmt::Display for SignedInteger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_neg {
            f.write_fmt(format_args!("-{}", self.value))
        } else {
            f.write_fmt(format_args!("{}", self.value))
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Float {
    pub value: f64,
    pub data_type: Option<ElementaryTypeName>,
}

/// Elementary type names.
///
/// See section 2.3.1.
#[derive(Debug, PartialEq, Clone)]
pub enum ElementaryTypeName {
    BOOL,
    SINT,
    INT,
    DINT,
    LINT,
    USINT,
    UINT,
    UDINT,
    ULINT,
    REAL,
    LREAL,
    TIME,
    DATE,
    TimeOfDay,
    DateAndTime,
    STRING,
    BYTE,
    WORD,
    DWORD,
    LWORD,
    WSTRING,
}

impl From<ElementaryTypeName> for Id {
    fn from(value: ElementaryTypeName) -> Id {
        match value {
            ElementaryTypeName::BOOL => Id::from("BOOL"),
            ElementaryTypeName::SINT => Id::from("SINT"),
            ElementaryTypeName::INT => Id::from("INT"),
            ElementaryTypeName::DINT => Id::from("DINT"),
            ElementaryTypeName::LINT => Id::from("LINT"),
            ElementaryTypeName::USINT => Id::from("USINT"),
            ElementaryTypeName::UINT => Id::from("UINT"),
            ElementaryTypeName::UDINT => Id::from("UDINT"),
            ElementaryTypeName::ULINT => Id::from("ULINT"),
            ElementaryTypeName::REAL => Id::from("REAL"),
            ElementaryTypeName::LREAL => Id::from("LREAL"),
            ElementaryTypeName::TIME => Id::from("TIME"),
            ElementaryTypeName::DATE => Id::from("DATE"),
            ElementaryTypeName::TimeOfDay => Id::from("TIME_OF_DAY"),
            ElementaryTypeName::DateAndTime => Id::from("DATE_AND_TIME"),
            ElementaryTypeName::STRING => Id::from("STRING"),
            ElementaryTypeName::BYTE => Id::from("BYTE"),
            ElementaryTypeName::WORD => Id::from("WORD"),
            ElementaryTypeName::DWORD => Id::from("DWORD"),
            ElementaryTypeName::LWORD => Id::from("LWORD"),
            ElementaryTypeName::WSTRING => Id::from("WSTRING"),
        }
    }
}

/// Kinds of derived data types.
///
/// See section 2.3.3.1
#[derive(Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum DataTypeDeclarationKind {
    /// Derived data type the restricts permitted values from a set of identifiers.
    Enumeration(EnumerationDeclaration),
    /// Derived data type that restricts permitted values to a smaller range
    /// of the parent data type.
    Subrange(SubrangeDeclaration),
    ///
    Simple(SimpleDeclaration),
    /// Derived data type that specifies required storage space for each instance.
    Array(ArrayDeclaration),
    Structure(StructureDeclaration),
    StructureInitialization(StructureInitializationDeclaration),
    String(StringDeclaration),
    /// Data declaration that is ambiguous at parse time and must be
    /// resolved to a data type declaration after parsing all types.
    LateBound(LateBoundDeclaration),
}

/// Type declarations that are indistinguishable as parsing time.
/// These are one of the following without an initial value:
/// * enumeration
/// * structure
/// * simple
#[derive(Debug, PartialEq)]
pub struct LateBoundDeclaration {
    /// The type name of this declaration. Other library elements
    /// refer to this this type with this name.
    pub data_type_name: Id,
    /// The referenced type name.
    ///
    /// For example, if this is an alias then this is the underlying
    /// type.
    pub base_type_name: Id,
}

/// See section 2.3.3.1.
#[derive(Debug, PartialEq)]
pub struct EnumerationDeclaration {
    pub type_name: Id,
    // TODO need to understand when the context name matters in the definition
    pub spec_init: EnumeratedSpecificationInit,
}

/// The specification of an enumeration with a possible default value.
///
/// See section 2.3.3.1.
#[derive(Debug, PartialEq)]
pub struct EnumeratedSpecificationInit {
    pub spec: EnumeratedSpecificationKind,
    pub default: Option<EnumeratedValue>,
}

impl EnumeratedSpecificationInit {
    pub fn values_and_default(values: Vec<&str>, default: &str) -> Self {
        EnumeratedSpecificationInit {
            spec: EnumeratedSpecificationKind::Values(EnumeratedSpecificationValues {
                values: values.into_iter().map(EnumeratedValue::new).collect(),
                position: SourceLoc::default(),
            }),
            default: Some(EnumeratedValue::new(default)),
        }
    }
}

/// See section 2.3.3.1.
#[derive(Debug, PartialEq)]
pub enum EnumeratedSpecificationKind {
    /// Enumeration declaration that renames another enumeration.
    TypeName(Id),
    /// Enumeration declaration that provides a list of values.
    ///
    /// Order of the values is important because the order declares the
    /// default value if no default is specified directly.
    Values(EnumeratedSpecificationValues),
}

impl EnumeratedSpecificationKind {
    pub fn from_values(values: Vec<&'static str>) -> EnumeratedSpecificationKind {
        let values = values
            .iter()
            .map(|v| EnumeratedValue {
                type_name: None,
                value: Id::from(v),
                position: SourceLoc::default(),
            })
            .collect();
        EnumeratedSpecificationKind::Values(EnumeratedSpecificationValues {
            values,
            position: SourceLoc::default(),
        })
    }

    pub fn values(
        values: Vec<EnumeratedValue>,
        position: SourceLoc,
    ) -> EnumeratedSpecificationKind {
        EnumeratedSpecificationKind::Values(EnumeratedSpecificationValues { values, position })
    }
}

/// See section 2.3.3.1.
#[derive(Debug, PartialEq)]
pub struct EnumeratedSpecificationValues {
    pub values: Vec<EnumeratedValue>,
    pub position: SourceLoc,
}

/// A particular value in a enumeration.
///
/// May include a type name (especially where the enumeration would be
/// ambiguous.)
///
/// See section 2.3.3.1.
#[derive(Debug, PartialEq, Clone)]
pub struct EnumeratedValue {
    pub type_name: Option<Id>,
    pub value: Id,
    pub position: SourceLoc,
}

impl EnumeratedValue {
    pub fn new(value: &str) -> Self {
        EnumeratedValue {
            type_name: None,
            value: Id::from(value),
            position: SourceLoc::default(),
        }
    }

    pub fn with_position(mut self, position: SourceLoc) -> Self {
        self.position = position;
        self
    }
}

impl SourcePosition for EnumeratedValue {
    fn position(&self) -> &SourceLoc {
        &self.position
    }
}

/// Subrange declaration narrows a type definition to the values in a smaller
/// range. Permitted values are the inclusive range minimum through maximum
/// specified values, that is, `[minimum, maximum]`.
///
/// See section 2.3.3.1.
#[derive(Debug, PartialEq)]
pub struct SubrangeDeclaration {
    pub type_name: Id,
    pub spec: SubrangeSpecification,
}

/// The specification for a subrange. The specification restricts an integer
/// type to a subset of the integer range.
///
/// See section 2.3.3.1.
#[derive(Debug, PartialEq, Clone)]
pub struct SubrangeSpecification {
    /// The parent type that is being restricted.
    /// TODO how can this be restricted to integer type names?
    pub type_name: ElementaryTypeName,
    pub subrange: Subrange,
    pub default: Option<SignedInteger>,
}

/// The specification for a simple declared type.
///
/// See section 2.3.3.1.
#[derive(Debug, PartialEq, Clone)]
pub struct SimpleDeclaration {
    pub type_name: Id,
    pub spec_and_init: InitialValueAssignmentKind,
}

/// Derived data type that
#[derive(Debug, PartialEq)]
pub struct ArrayDeclaration {
    pub type_name: Id,
    pub spec: ArraySpecificationKind,
    pub init: Vec<ArrayInitialElementKind>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayInitialElementKind {
    Constant(Constant),
    EnumValue(EnumeratedValue),
    Repeated(Integer, Box<Option<ArrayInitialElementKind>>),
}

impl ArrayInitialElementKind {
    pub fn repeated(size: Integer, init: Option<ArrayInitialElementKind>) -> Self {
        ArrayInitialElementKind::Repeated(size, Box::new(init))
    }
}

/// Structure declaration creates a combination of multiple elements (each having
/// a specific type) as a single unit. Components are accessed by a name. Structures
/// may be nested but must not contain an instance of itself.
///
/// See section 2.3.3.1.
#[derive(Debug, PartialEq)]
pub struct StructureDeclaration {
    /// The name of the structure.
    pub type_name: Id,
    /// The elements (components) of the structure declaration.
    pub elements: Vec<StructureElementDeclaration>,
}

/// Declares an element contained within a structure.
///
/// See section 2.3.3.1.
#[derive(Debug, PartialEq)]
pub struct StructureElementDeclaration {
    pub name: Id,
    pub init: InitialValueAssignmentKind,
}

/// See section 2.3.3.1.
#[derive(Debug, PartialEq, Clone)]
pub struct StructureInitializationDeclaration {
    pub type_name: Id,
    pub elements_init: Vec<StructureElementInit>,
}

/// Initializes a particular element in a structured type.
///
/// See section 2.3.3.1.
#[derive(Debug, PartialEq, Clone)]
pub struct StructureElementInit {
    /// The name of the element in the structure to initialize.
    pub name: Id,
    pub init: StructInitialValueAssignmentKind,
}

#[derive(Debug, PartialEq, Clone)]
pub enum StringKind {
    /// String of single-byte characters
    String,
    /// String of double-byte characters
    WString,
}

/// Declares a string type with restricted length.
///
/// See section 2.3.3.1.
#[derive(Debug, PartialEq)]
pub struct StringDeclaration {
    pub type_name: Id,
    pub length: Integer,
    /// The size of a single 'character'
    pub width: StringKind,
    pub init: Option<String>,
}

/// Location prefix for directly represented variables.
///
/// See section 2.4.1.1.
#[derive(Debug, PartialEq, Clone)]
pub enum LocationPrefix {
    /// Input location
    I,
    /// Output location
    Q,
    /// Memory location
    M,
}

/// Size prefix for directly represented variables. Defines how many bits
/// are associated with the variable.
///
/// See section 2.4.1.1.
#[derive(Debug, PartialEq, Clone)]
pub enum SizePrefix {
    /// Unspecified (indicated by asterisk)
    Unspecified,
    /// Single bit size
    Nil,
    /// Single bit size
    X,
    /// 8-bit size
    B,
    /// 16-bit size
    W,
    /// 32-bit size
    D,
    /// 64-bit size
    L,
}

/// Array specification defines a size/shape of an array.
#[derive(Debug, Clone, PartialEq)]
pub enum ArraySpecificationKind {
    Type(Id),
    Subranges(Vec<Subrange>, Id),
}

/// Subrange of an array.
///
/// See section 2.4.2.1.
#[derive(Debug, Clone, PartialEq)]
pub struct Subrange {
    pub start: SignedInteger,
    pub end: SignedInteger,
}

/// Container for structures that have variables.
///
/// Several different structures own variables and implementing this trait
/// allows a common handling of those items.
pub trait HasVariables {
    fn variables(&self) -> &Vec<VarDecl>;
}

/// Declaration (that does not permit a location).
///
/// See section 2.4.3.
#[derive(Debug, PartialEq, Clone)]
pub struct VarDecl {
    pub name: Id,
    pub var_type: VariableType,
    pub qualifier: DeclarationQualifier,
    pub initializer: InitialValueAssignmentKind,
    pub position: SourceLoc,
}

impl VarDecl {
    /// Creates a variable declaration for simple type and no initialization.
    /// The declaration has type `VAR` and no qualifier.
    pub fn simple(name: &str, type_name: &str) -> Self {
        Self {
            name: Id::from(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::simple_uninitialized(type_name),
            position: SourceLoc::default(),
        }
    }

    pub fn string(
        name: &str,
        var_type: VariableType,
        qualifier: DeclarationQualifier,
        loc: SourceLoc,
    ) -> Self {
        Self {
            name: Id::from(name),
            var_type,
            qualifier,
            initializer: InitialValueAssignmentKind::String(StringInitializer {
                length: None,
                width: StringKind::String,
                initial_value: None,
            }),
            position: loc,
        }
    }

    /// Creates a variable declaration for enumeration without having an initial value.
    /// The declaration has type `VAR` and no qualifier.
    pub fn uninitialized_enumerated(name: &str, type_name: &str) -> Self {
        VarDecl {
            name: Id::from(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::EnumeratedType(
                EnumeratedInitialValueAssignment {
                    type_name: Id::from(type_name),
                    initial_value: None,
                },
            ),
            position: SourceLoc::default(),
        }
    }

    /// Creates a variable declaration for enumeration having an initial value.
    /// The declaration has type `VAR` and no qualifier.
    pub fn enumerated(name: &str, type_name: &str, initial_value: &str) -> Self {
        VarDecl {
            name: Id::from(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::EnumeratedType(
                EnumeratedInitialValueAssignment {
                    type_name: Id::from(type_name),
                    initial_value: Some(EnumeratedValue {
                        type_name: None,
                        value: Id::from(initial_value),
                        position: SourceLoc::default(),
                    }),
                },
            ),
            position: SourceLoc::default(),
        }
    }

    /// Creates a variable declaration for a function block.
    /// The declaration has type `VAR` and no qualifier.
    pub fn function_block(name: &str, type_name: &str) -> Self {
        VarDecl {
            name: Id::from(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::FunctionBlock(
                FunctionBlockInitialValueAssignment {
                    type_name: Id::from(type_name),
                },
            ),
            position: SourceLoc::default(),
        }
    }

    /// Creates a variable declaration for a structure.
    pub fn structure(name: &str, type_name: &str) -> Self {
        VarDecl {
            name: Id::from(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::Structure(
                StructureInitializationDeclaration {
                    type_name: Id::from(type_name),
                    elements_init: vec![],
                },
            ),
            position: SourceLoc::default(),
        }
    }

    /// Creates a variable declaration that is ambiguous on the type.
    /// The declaration has type `VAR` and no qualifier.
    ///
    /// The language has some ambiguity for types. The late bound represents
    /// a placeholder that is later resolved once all types are known.
    pub fn late_bound(name: &str, type_name: &str) -> Self {
        VarDecl {
            name: Id::from(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::LateResolvedType(Id::from(type_name)),
            position: SourceLoc::default(),
        }
    }

    /// Assigns the type of the variable declaration.
    pub fn with_type(mut self, var_type: VariableType) -> Self {
        self.var_type = var_type;
        self
    }

    /// Assigns the qualifier of the variable declaration.
    pub fn with_qualifier(mut self, qualifier: DeclarationQualifier) -> Self {
        self.qualifier = qualifier;
        self
    }
}

/// Keywords for declarations.
///
/// IEC 61131-3 defines groups that can contain multiple variables. These
/// groups introduce complexity in parsing and in iterating. This
/// implementation treats the groups as labels on individual variables; in
/// effect, there are no groups.
///
/// See section 2.4.3.
#[derive(Debug, PartialEq, Clone)]
pub enum VariableType {
    /// Local to a POU.
    Var,
    /// Local to a POU. Does not need to be maintained
    /// between calls to a POU.
    VarTemp,
    /// Variable that is visible to a calling POU as an input.
    Input,
    /// Variable that is visible to calling POU and can only
    /// be ready from the calling POU. It can be written to
    /// by the POU that defines the variable.
    Output,
    /// Variable that is visible to calling POU and is readable
    /// writeable by the calling POU.
    InOut,
    /// Enables a POU to read and (possibly) write to a global
    /// variable.
    External,
    /// A variable that may be read and written by multiple
    /// POUs that also declare the variable as external.
    Global,
    /// Configurations for communication channels.
    Access,
    /// TODO is this really a type or just a variation on the set fields?
    Located,
}

/// Qualifier types for definitions.
///
/// IEC 61131-3 defines groups that share common qualifiers. These
/// groups introduce complexity in parsing and in iterating. This
/// implementation treats the groups as labels on individual variables; in
/// effect, there are no groups.
///
/// See section 2.4.3.
#[derive(Debug, PartialEq, Clone)]
pub enum DeclarationQualifier {
    // TODO Some of these are not valid for some contexts - should there be multiple
    // qualifier classes, indicate some how, or fail?
    Unspecified,
    Constant,
    /// Stored so that the value is retained through power loss.
    Retain,
    /// Stored so that the value is NOT retained through power loss.
    NonRetain,
}

pub struct LocatedVarDecl {
    pub name: Option<Id>,
    pub qualifier: DeclarationQualifier,
    pub location: AddressAssignment,
    pub initializer: InitialValueAssignmentKind,
    pub position: SourceLoc,
}

/// Location assignment for a variable.
///
/// See section 2.4.3.1.
#[derive(PartialEq, Clone)]
pub struct AddressAssignment {
    pub location: LocationPrefix,
    pub size: SizePrefix,
    pub address: Vec<u32>,
}

impl fmt::Debug for AddressAssignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AddressAssignment")
            .field("location", &self.location)
            .field("size", &self.size)
            .finish()
    }
}

impl fmt::Display for AddressAssignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AddressAssignment")
            .field("location", &self.location)
            .field("size", &self.size)
            .finish()
    }
}

/// Container for initial value assignments. The initial value specifies a
/// "coarse grained assignment",
///
/// Declarations of variables can be associated with an initial value. The
/// initial value assignment is not necessarily compatible with the associated
/// variable.
///
/// See section 2.4.3.2.
#[derive(PartialEq, Clone, Debug)]
pub enum InitialValueAssignmentKind {
    /// Represents no type initializer.
    ///
    /// Some types allow no initializer and this avoids nesting of the
    /// enumeration with an Option enumeration.
    None,
    Simple(SimpleInitializer),
    String(StringInitializer),
    EnumeratedValues(EnumeratedValuesInitializer),
    EnumeratedType(EnumeratedInitialValueAssignment),
    FunctionBlock(FunctionBlockInitialValueAssignment),
    Subrange(SubrangeSpecification),
    Structure(StructureInitializationDeclaration),
    Array(ArrayInitialValueAssignment),
    /// Type that is ambiguous until have discovered type
    /// definitions. Value is the name of the type.
    LateResolvedType(Id),
}

impl InitialValueAssignmentKind {
    /// Creates an initial value with
    pub fn simple_uninitialized(type_name: &str) -> Self {
        InitialValueAssignmentKind::Simple(SimpleInitializer {
            type_name: Id::from(type_name),
            initial_value: None,
        })
    }

    /// Creates an initial value from the initializer.
    pub fn simple(type_name: &str, value: Constant) -> Self {
        InitialValueAssignmentKind::Simple(SimpleInitializer {
            type_name: Id::from(type_name),
            initial_value: Some(value),
        })
    }

    /// Creates an initial value consisting of an enumeration definition and
    /// possible initial value for the enumeration.
    pub fn enumerated_values(
        values: Vec<EnumeratedValue>,
        initial_value: Option<EnumeratedValue>,
    ) -> Self {
        InitialValueAssignmentKind::EnumeratedValues(EnumeratedValuesInitializer {
            values,
            initial_value,
        })
    }
}

/// Container for initial value assignments in structures.
///
/// Initial value assignments in structures are similar to initial value
/// assignments outside of structures except that they cannot have a
/// specification (the specification is with the structure) and that the
/// initialization is required.
///
/// See section 2.4.3.2.
#[derive(PartialEq, Clone, Debug)]
pub enum StructInitialValueAssignmentKind {
    Constant(Constant),
    EnumeratedValue(EnumeratedValue),
    Array(Vec<ArrayInitialElementKind>),
    Structure(Vec<StructureElementInit>),
}

#[derive(PartialEq, Clone, Debug)]
pub enum Boolean {
    True,
    False,
}

/// Container for elementary constants.
///
/// See section 2.2.
#[derive(PartialEq, Clone, Debug)]
pub enum Constant {
    // TODO these need values
    IntegerLiteral(i128),
    RealLiteral(Float),
    CharacterString(),
    Duration(Duration),
    TimeOfDay(),
    Date(),
    DateAndTime(),
    Boolean(Boolean),
}

#[derive(PartialEq, Clone, Debug)]
pub struct EnumeratedInitialValueAssignment {
    pub type_name: Id,
    pub initial_value: Option<EnumeratedValue>,
}

#[derive(PartialEq, Clone, Debug)]
pub struct SimpleInitializer {
    pub type_name: Id,
    pub initial_value: Option<Constant>,
}

/// Provides the initialization of a string variable declaration.
///
/// See sections 2.4.3.1 and 2.4.3.2.
#[derive(PartialEq, Clone, Debug)]
pub struct StringInitializer {
    /// Maximum length of the string.
    pub length: Option<Integer>,
    /// The size of a single 'character'
    pub width: StringKind,
    /// Default value of the string. If not specified, then
    /// the default value is the empty string.
    pub initial_value: Option<Vec<char>>,
}

#[derive(PartialEq, Clone, Debug)]
pub struct EnumeratedValuesInitializer {
    pub values: Vec<EnumeratedValue>,
    pub initial_value: Option<EnumeratedValue>,
}

#[derive(PartialEq, Clone, Debug)]
pub struct FunctionBlockInitialValueAssignment {
    pub type_name: Id,
}

/// See section 2.4.3.2. #6
#[derive(PartialEq, Clone, Debug)]
pub struct ArrayInitialValueAssignment {
    pub spec: ArraySpecificationKind,
    pub initial_values: Vec<ArrayInitialElementKind>,
}

/// Container for top-level elements that are valid top-level declarations in
/// a library.
///
/// The library element flattens data type declaration blocks so that each
/// enumeration is for a single data type declaration.
#[derive(Debug, PartialEq)]
pub enum LibraryElement {
    DataTypeDeclaration(DataTypeDeclarationKind),
    FunctionDeclaration(FunctionDeclaration),
    // TODO
    FunctionBlockDeclaration(FunctionBlockDeclaration),
    ProgramDeclaration(ProgramDeclaration),
    ConfigurationDeclaration(ConfigurationDeclaration),
}

///Function Program Organization Unit Declaration
///
/// A function is stateless and has no "memory". Functions
/// consists of a series of statements that provide outputs through the
/// return value and bound variables.
///
/// See section 2.5.1.
#[derive(Debug, PartialEq, Clone)]
pub struct FunctionDeclaration {
    pub name: Id,
    pub return_type: Id,
    pub variables: Vec<VarDecl>,
    pub body: Vec<StmtKind>,
}

impl HasVariables for FunctionDeclaration {
    fn variables(&self) -> &Vec<VarDecl> {
        &self.variables
    }
}

/// Function Block Program Organization Unit Declaration
///
/// A function block declaration (as distinct from a particular
/// instance of a function block). The Function block instance is stateful
/// and variables retain values between invocations.
///
/// See section 2.5.2.
#[derive(Debug, PartialEq, Clone)]
pub struct FunctionBlockDeclaration {
    pub name: Id,
    pub variables: Vec<VarDecl>,
    pub body: FunctionBlockBody,
    pub position: SourceLoc,
}

impl HasVariables for FunctionBlockDeclaration {
    fn variables(&self) -> &Vec<VarDecl> {
        &self.variables
    }
}

/// "Program" Program Organization Unit Declaration Declaration
///
/// Programs assembled the units into a whole that embodies a measurement
/// or control objective.
///
/// See section 2.5.3.
#[derive(Debug, PartialEq)]
pub struct ProgramDeclaration {
    pub type_name: Id,
    pub variables: Vec<VarDecl>,
    // TODO located variables
    // TODO other stuff here
    pub body: FunctionBlockBody,
}

impl HasVariables for ProgramDeclaration {
    fn variables(&self) -> &Vec<VarDecl> {
        &self.variables
    }
}

/// Sequential function chart.
///
/// See section 2.6.
#[derive(Debug, PartialEq, Clone)]
pub struct Sfc {
    pub networks: Vec<Network>,
}

/// Resource assigns tasks to a particular CPU.
///
/// See section 2.7.1.
#[derive(Debug, PartialEq)]
pub struct ResourceDeclaration {
    /// Symbolic name for a CPU
    pub name: Id,
    /// The identifier for a CPU
    pub resource: Id,
    /// Global variables in the scope of the resource.
    ///
    /// Global variables are not in scope for other resources.
    pub global_vars: Vec<VarDecl>,
    /// Defines the configuration of programs on this resource.
    pub tasks: Vec<TaskConfiguration>,
    /// Defines runnable programs.
    ///
    /// A runnable program can be associated with a task configuration
    /// by name.
    pub programs: Vec<ProgramConfiguration>,
}

impl HasVariables for ResourceDeclaration {
    fn variables(&self) -> &Vec<VarDecl> {
        &self.global_vars
    }
}

/// Program configurations.
///
/// See section 2.7.1.
#[derive(Debug, PartialEq)]
pub struct ProgramConfiguration {
    pub name: Id,
    pub task_name: Option<Id>,
    pub type_name: Id,
}

/// Configuration declaration,
///
/// See section 2.7.1.
#[derive(Debug, PartialEq)]
pub struct ConfigurationDeclaration {
    pub name: Id,
    pub global_var: Vec<VarDecl>,
    pub resource_decl: Vec<ResourceDeclaration>,
}

impl HasVariables for ConfigurationDeclaration {
    fn variables(&self) -> &Vec<VarDecl> {
        &self.global_var
    }
}

/// Task configuration.
///
/// See section 2.7.2.
#[derive(Debug, PartialEq)]
pub struct TaskConfiguration {
    pub name: Id,
    pub priority: u32,
    // TODO this might not be optional
    pub interval: Option<Duration>,
}

/// Container for type types of elements that can compose the body of a
/// function block.
///
/// See section 2.5.2.
#[derive(Debug, PartialEq, Clone)]
pub enum FunctionBlockBody {
    Sfc(Sfc),
    Statements(Statements),
    /// A function block that has no body (and is therefore no known type).
    ///
    /// This type is not strictly valid, but highly useful and can be detected
    /// with a semantic rule.
    Empty(),
}

impl FunctionBlockBody {
    /// Creates a function body that is composed of statements.
    pub fn stmts(stmts: Vec<StmtKind>) -> FunctionBlockBody {
        FunctionBlockBody::Statements(Statements { body: stmts })
    }

    /// Creates a function body that is composed of a sequential function block.
    pub fn sfc(networks: Vec<Network>) -> FunctionBlockBody {
        FunctionBlockBody::Sfc(Sfc { networks })
    }

    /// Creates an empty function body.
    pub fn empty() -> FunctionBlockBody {
        FunctionBlockBody::Empty()
    }
}

/// Container for a library that contains top-level elements. Libraries are
/// typically represented as a file resource.
#[derive(Debug, PartialEq)]
pub struct Library {
    pub elements: Vec<LibraryElement>,
}

impl Library {
    pub fn new(elements: Vec<LibraryElement>) -> Self {
        Library { elements }
    }
}
