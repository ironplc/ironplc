//! Provides definitions of objects from IEC 61131-3 common elements.
//!
//! See section 2.
use core::str::FromStr;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::num::TryFromIntError;
use time::Duration;

use crate::common_sfc::Network;
use crate::core::{Id, SourceLoc};
use crate::textual::*;

/// Numeric liberals declared by 2.2.1. Numeric literals define
/// how data is expressed and are distinct from but associated with
/// data types.

/// Integer liberal. The representation is of the largest possible integer
/// and later bound to smaller types depend on context.
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
        value.value.try_into().map_err(|e| TryFromIntegerError {})
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

#[derive(Debug, PartialEq, Clone)]
pub struct Float {
    pub value: f64,
    pub data_type: Option<Id>,
}

/// Derived data types declared by 2.3.3.
pub enum TypeDefinitionKind {
    /// Defines a type that can take one of a set number of values.
    Enumeration,
    FunctionBlock,
    Function,
    /// Defines a type composed of sub-elements.
    Structure,
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

impl LocationPrefix {
    pub fn from_char(l: char) -> LocationPrefix {
        match l {
            'I' => LocationPrefix::I,
            'Q' => LocationPrefix::Q,
            'M' => LocationPrefix::M,
            // TODO error message
            _ => panic!(),
        }
    }
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

impl SizePrefix {
    pub fn from_char(s: char) -> SizePrefix {
        match s {
            'X' => SizePrefix::X,
            'B' => SizePrefix::B,
            'W' => SizePrefix::W,
            'D' => SizePrefix::D,
            'L' => SizePrefix::L,
            // TODO error message
            _ => panic!(),
        }
    }
}

/// Declaration (that does not permit a location).
///
/// See section 2.4.3.
#[derive(Debug, PartialEq, Clone)]
pub struct VarDecl {
    pub name: Id,
    pub var_type: VariableType,
    pub qualifier: DeclarationQualifier,
    pub initializer: InitialValueAssignment,
    pub position: SourceLoc,
}

impl VarDecl {
    /// Creates a variable declaration for simple type and no initialization.
    pub fn simple_input(name: &str, type_name: &str, loc: SourceLoc) -> Self {
        Self::simple(name, type_name, VariableType::Input, loc)
    }

    /// Creates a variable declaration for simple type and no initialization.
    pub fn simple_output(name: &str, type_name: &str, loc: SourceLoc) -> Self {
        Self::simple(name, type_name, VariableType::Output, loc)
    }

    /// Creates a variable declaration for simple type and no initialization.
    pub fn simple_var(name: &str, type_name: &str, loc: SourceLoc) -> Self {
        Self::simple(name, type_name, VariableType::Var, loc)
    }

    /// Creates a variable declaration for simple type and no initialization.
    pub fn simple_external(name: &str, type_name: &str, loc: SourceLoc) -> Self {
        Self::simple(name, type_name, VariableType::External, loc)
    }

    /// Creates a variable declaration for simple type and no initialization.
    pub fn simple(name: &str, type_name: &str, var_type: VariableType, loc: SourceLoc) -> Self {
        Self {
            name: Id::from(name),
            var_type,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignment::simple_uninitialized(type_name),
            position: loc,
        }
    }

    /// Creates a variable declaration for enumeration having an initial value.
    pub fn enumerated_input(
        name: &str,
        type_name: &str,
        initial_value: &str,
        loc: SourceLoc,
    ) -> Self {
        VarDecl {
            name: Id::from(name),
            var_type: VariableType::Input,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignment::EnumeratedType(EnumeratedInitialValueAssignment {
                type_name: Id::from(type_name),
                initial_value: Some(Id::from(initial_value)),
            }),
            position: loc,
        }
    }

    /// Creates a variable declaration for a function block.
    pub fn function_block_var(name: &str, type_name: &str, loc: SourceLoc) -> Self {
        VarDecl {
            name: Id::from(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignment::FunctionBlock(
                FunctionBlockInitialValueAssignment {
                    type_name: Id::from(type_name),
                },
            ),
            position: loc,
        }
    }

    /// Creates a variable declaration that is ambiguous on the type.
    ///
    /// The language has some ambiguity for types. The late bound represents
    /// a placeholder that is later resolved once all types are known.
    pub fn late_bound_input(name: &str, type_name: &str, loc: SourceLoc) -> Self {
        Self::late_bound(name, type_name, VariableType::Input, loc)
    }

    /// Creates a variable declaration that is ambiguous on the type.
    ///
    /// The language has some ambiguity for types. The late bound represents
    /// a placeholder that is later resolved once all types are known.
    pub fn late_bound_var(name: &str, type_name: &str, loc: SourceLoc) -> Self {
        Self::late_bound(name, type_name, VariableType::Var, loc)
    }

    /// Creates a variable declaration that is ambiguous on the type.
    ///
    /// The language has some ambiguity for types. The late bound represents
    /// a placeholder that is later resolved once all types are known.
    pub fn late_bound(name: &str, type_name: &str, var_type: VariableType, loc: SourceLoc) -> Self {
        VarDecl {
            name: Id::from(name),
            var_type,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignment::LateResolvedType(Id::from(type_name)),
            position: loc,
        }
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
    pub initializer: InitialValueAssignment,
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
pub enum InitialValueAssignment {
    /// Represents no type initializer.
    ///
    /// Some types allow no initializer and this avoids nesting of the
    /// enumeration with an Option enumeration.
    None,
    Simple(SimpleInitializer),
    EnumeratedValues(EnumeratedValuesInitializer),
    EnumeratedType(EnumeratedInitialValueAssignment),
    FunctionBlock(FunctionBlockInitialValueAssignment),
    Structure {
        // TODO
        type_name: Id,
    },
    /// Type that is ambiguous until have discovered type
    /// definitions. Value is the name of the type.
    LateResolvedType(Id),
}

impl InitialValueAssignment {
    /// Creates an initial value with
    pub fn simple_uninitialized(type_name: &str) -> InitialValueAssignment {
        InitialValueAssignment::Simple(SimpleInitializer {
            type_name: Id::from(type_name),
            initial_value: None,
        })
    }

    /// Creates an initial value from the initializer.
    pub fn simple(type_name: &str, value: Initializer) -> InitialValueAssignment {
        InitialValueAssignment::Simple(SimpleInitializer {
            type_name: Id::from(type_name),
            initial_value: Some(value),
        })
    }

    /// Creates an initial value consisting of an enumeration definition and
    /// possible initial value for the enumeration.
    pub fn enumerated_values(
        values: Vec<Id>,
        initial_value: Option<Id>,
        position: SourceLoc,
    ) -> InitialValueAssignment {
        InitialValueAssignment::EnumeratedValues(EnumeratedValuesInitializer {
            values,
            initial_value,
            position,
        })
    }
}

/// Container for elementary constants.
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
}

#[derive(PartialEq, Clone)]
pub enum Initializer {
    Simple(Constant),
    Subrange(),
    Enumerated(),
    Array(),
    InitializedStructure(),
    SingleByteString(),
    DoubleByteString(),
}

impl fmt::Debug for Initializer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Initializer").finish()
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct EnumeratedInitialValueAssignment {
    pub type_name: Id,
    pub initial_value: Option<Id>,
}

#[derive(PartialEq, Clone, Debug)]
pub struct SimpleInitializer {
    pub type_name: Id,
    pub initial_value: Option<Initializer>,
}

#[derive(PartialEq, Clone, Debug)]
pub struct EnumeratedValuesInitializer {
    pub values: Vec<Id>,
    pub initial_value: Option<Id>,
    pub position: SourceLoc,
}

#[derive(PartialEq, Clone, Debug)]
pub struct FunctionBlockInitialValueAssignment {
    pub type_name: Id,
}

/// Container for top-level elements that are valid top-level declarations in
/// a library.
#[derive(Debug, PartialEq)]
pub enum LibraryElement {
    DataTypeDeclaration(Vec<EnumerationDeclaration>),
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

#[derive(Debug, PartialEq)]
pub struct EnumerationDeclaration {
    pub name: Id,
    // TODO need to understand when the context name matters in the definition
    pub spec: EnumeratedSpecificationKind,
    pub default: Option<Id>,
}

#[derive(Debug, PartialEq)]
pub struct EnumeratedSpecificationValues {
    pub ids: Vec<Id>,
    pub position: SourceLoc,
}

#[derive(Debug, PartialEq)]
pub enum EnumeratedSpecificationKind {
    TypeName(Id),
    /// Enumeration declaration that provides a list of values.
    ///
    /// Order of the values is important because the order declares the
    /// default value if no default is specified directly.
    Values(EnumeratedSpecificationValues),
}

impl EnumeratedSpecificationKind {
    pub fn values(values: Vec<Id>, position: SourceLoc) -> EnumeratedSpecificationKind {
        EnumeratedSpecificationKind::Values(EnumeratedSpecificationValues {
            ids: values,
            position,
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Statements {
    pub body: Vec<StmtKind>,
}

/// Container for type types of elements that can compose the body of a
/// function block.
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
