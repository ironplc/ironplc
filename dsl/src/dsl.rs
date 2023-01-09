use core::str::FromStr;
use std::fmt;
use std::hash::{Hash, Hasher};
use time::Duration;

use crate::ast::*;
use crate::core::{Id, SourceLoc};
use crate::sfc::Network;

/// Numeric liberals declared by 2.2.1. Numeric literals define
/// how data is expressed and are distinct from but associated with
/// data types.

/// IEC 61131-3 integer.
///
/// Underlying data type is a String to trace back to the original
/// representation if the value is not valid.
pub struct Integer {
    value: String,
}

impl Integer {
    pub fn try_from<T: FromStr>(&self) -> T {
        let v: String = self.value.chars().filter(|c| c.is_ascii_digit()).collect();
        match v.parse::<T>() {
            Ok(v) => v,
            Err(_) => panic!("out of range"),
        }
    }

    pub fn as_type<T: FromStr>(&self) -> T {
        self.try_from::<T>()
    }

    pub fn num_chars(&self) -> u8 {
        let value: String = self.value.chars().filter(|c| c.is_ascii_digit()).collect();
        // TODO This is most obviously wrong
        let value: u8 = 1;
        value
    }

    pub fn from(a: &str) -> Integer {
        Integer {
            value: String::from(a),
        }
    }
}

pub struct SignedInteger {
    value: Integer,
    is_neg: bool,
}

impl SignedInteger {
    pub fn as_type<T: FromStr>(&self) -> T {
        self.value.try_from::<T>()
        // TODO
        //if self.is_neg {
        //    val *= -1;
        //}
    }
    pub fn from(a: &str) -> SignedInteger {
        match a.chars().next() {
            Some('+') => {
                SignedInteger {
                    value: Integer::from(a.get(1..).unwrap()),
                    is_neg: false,
                }
            }
            Some('-') => {
                SignedInteger {
                    value: Integer::from(a.get(1..).unwrap()),
                    is_neg: true,
                }
            }
            _ => {
                SignedInteger {
                    value: Integer::from(a),
                    is_neg: false,
                }
            }
        }
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

// TODO I don't know if I need to support multiple qualifier classes for the
// same value.
// 2.4.3 Declaration
#[derive(Debug, PartialEq, Clone)]
pub struct Declaration {
    pub name: Id,
    pub qualifier: StorageQualifier,
    pub at: Option<At>,
    pub initializer: Option<TypeInitializer>,
}

/// 2.4.3 Qualifier types for definitions
#[derive(Debug, PartialEq, Clone)]
pub enum StorageQualifier {
    // TODO Some of these are not valid for some contexts - should there be multiple
    // qualifier classes, indicate some how, or fail?
    Unspecified,
    Constant,
    /// Stored so that the value is retained through power loss.
    Retain,
    /// Stored so that the value is NOT retained through power loss.
    NonRetain,
}

/// Defines the top-level elements that are valid declarations in a library.
#[derive(Debug, PartialEq)]
pub enum LibraryElement {
    DataTypeDeclaration(Vec<EnumerationDeclaration>),
    FunctionDeclaration(FunctionDeclaration),
    // TODO
    FunctionBlockDeclaration(FunctionBlockDeclaration),
    ProgramDeclaration(ProgramDeclaration),
    ConfigurationDeclaration(ConfigurationDeclaration),
}

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
}

#[derive(Debug, PartialEq, Clone)]
pub struct VarInitDecl {
    pub name: Id,
    pub var_type: VariableType,
    pub qualifier: StorageQualifier,
    pub initializer: TypeInitializer,
    // TODO this need much more
    pub position: SourceLoc,
}

impl VarInitDecl {
    /// Creates a variable declaration for simple type and no initialization.
    pub fn simple_input(name: &str, type_name: &str, loc: SourceLoc) -> VarInitDecl {
        VarInitDecl::simple(name, type_name, VariableType::Input, loc)
    }

    /// Creates a variable declaration for simple type and no initialization.
    pub fn simple_output(name: &str, type_name: &str, loc: SourceLoc) -> VarInitDecl {
        VarInitDecl::simple(name, type_name, VariableType::Output, loc)
    }

    /// Creates a variable declaration for simple type and no initialization.
    pub fn simple_var(name: &str, type_name: &str, loc: SourceLoc) -> VarInitDecl {
        VarInitDecl::simple(name, type_name, VariableType::Var, loc)
    }

    /// Creates a variable declaration for simple type and no initialization.
    pub fn simple_external(name: &str, type_name: &str, loc: SourceLoc) -> VarInitDecl {
        VarInitDecl::simple(name, type_name, VariableType::External, loc)
    }

    /// Creates a variable declaration for simple type and no initialization.
    pub fn simple(
        name: &str,
        type_name: &str,
        var_type: VariableType,
        loc: SourceLoc,
    ) -> VarInitDecl {
        VarInitDecl {
            name: Id::from(name),
            var_type,
            qualifier: StorageQualifier::Unspecified,
            initializer: TypeInitializer::Simple {
                type_name: Id::from(type_name),
                initial_value: None,
            },
            position: loc,
        }
    }

    /// Creates a variable declaration for enumeration having an initial value.
    pub fn enumerated_input(
        name: &str,
        type_name: &str,
        initial_value: &str,
        loc: SourceLoc,
    ) -> VarInitDecl {
        VarInitDecl {
            name: Id::from(name),
            var_type: VariableType::Input,
            qualifier: StorageQualifier::Unspecified,
            initializer: TypeInitializer::EnumeratedType(EnumeratedTypeInitializer {
                type_name: Id::from(type_name),
                initial_value: Some(Id::from(initial_value)),
            }),
            position: loc,
        }
    }

    /// Creates a variable declaration for a function block.
    pub fn function_block_var(name: &str, type_name: &str, loc: SourceLoc) -> VarInitDecl {
        VarInitDecl {
            name: Id::from(name),
            var_type: VariableType::Var,
            qualifier: StorageQualifier::Unspecified,
            initializer: TypeInitializer::FunctionBlock(FunctionBlockTypeInitializer {
                type_name: Id::from(type_name),
            }),
            position: loc,
        }
    }

    /// Creates a variable declaration that is ambiguous on the type.
    ///
    /// The language has some ambiguity for types. The late bound represents
    /// a placeholder that is later resolved once all types are known.
    pub fn late_bound_input(name: &str, type_name: &str, loc: SourceLoc) -> VarInitDecl {
        VarInitDecl::late_bound(name, type_name, VariableType::Input, loc)
    }

    /// Creates a variable declaration that is ambiguous on the type.
    ///
    /// The language has some ambiguity for types. The late bound represents
    /// a placeholder that is later resolved once all types are known.
    pub fn late_bound_var(name: &str, type_name: &str, loc: SourceLoc) -> VarInitDecl {
        VarInitDecl::late_bound(name, type_name, VariableType::Var, loc)
    }

    /// Creates a variable declaration that is ambiguous on the type.
    ///
    /// The language has some ambiguity for types. The late bound represents
    /// a placeholder that is later resolved once all types are known.
    pub fn late_bound(
        name: &str,
        type_name: &str,
        var_type: VariableType,
        loc: SourceLoc,
    ) -> VarInitDecl {
        VarInitDecl {
            name: Id::from(name),
            var_type,
            qualifier: StorageQualifier::Unspecified,
            initializer: TypeInitializer::LateResolvedType(Id::from(type_name)),
            position: loc,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct LocatedVarInit {
    pub name: Option<Id>,
    pub qualifier: StorageQualifier,
    pub at: DirectVariable,
    pub initializer: TypeInitializer,
}

#[derive(Debug, PartialEq, Clone)]
pub enum VarInitKind {
    VarInit(VarInitDecl),
    LocatedVarInit(LocatedVarInit),
}

// 2.4.3.1 Type assignment
#[derive(Debug, PartialEq, Clone)]
pub struct At {}

// 2.7.2 Tasks
#[derive(Debug, PartialEq)]
pub struct TaskConfiguration {
    pub name: Id,
    pub priority: u32,
    // TODO this might not be optional
    pub interval: Option<Duration>,
}

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
pub struct EnumeratedTypeInitializer {
    pub type_name: Id,
    pub initial_value: Option<Id>,
}

#[derive(PartialEq, Clone, Debug)]
pub struct EnumeratedValuesInitializer {
    pub values: Vec<Id>,
    pub initial_value: Option<Id>,
    pub position: SourceLoc,
}

#[derive(PartialEq, Clone, Debug)]
pub struct FunctionBlockTypeInitializer {
    pub type_name: Id,
}

#[derive(PartialEq, Clone, Debug)]
pub enum TypeInitializer {
    Simple {
        type_name: Id,
        initial_value: Option<Initializer>,
    },
    EnumeratedValues(EnumeratedValuesInitializer),
    EnumeratedType(EnumeratedTypeInitializer),
    FunctionBlock(FunctionBlockTypeInitializer),
    Structure {
        // TODO
        type_name: Id,
    },
    /// Type that is ambiguous until have discovered type
    /// definitions. Value is the name of the type.
    LateResolvedType(Id),
}

impl TypeInitializer {
    pub fn simple_uninitialized(type_name: &str) -> TypeInitializer {
        TypeInitializer::Simple {
            type_name: Id::from(type_name),
            initial_value: None,
        }
    }

    pub fn simple(type_name: &str, value: Initializer) -> TypeInitializer {
        TypeInitializer::Simple {
            type_name: Id::from(type_name),
            initial_value: Some(value),
        }
    }

    pub fn enumerated_values(
        values: Vec<Id>,
        initial_value: Option<Id>,
        position: SourceLoc,
    ) -> TypeInitializer {
        TypeInitializer::EnumeratedValues(EnumeratedValuesInitializer {
            values,
            initial_value,
            position,
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum LocationPrefix {
    I,
    Q,
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

#[derive(Debug, PartialEq, Clone)]
pub enum SizePrefix {
    Nil,
    X,
    B,
    W,
    D,
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

/// Resource assigns tasks to a particular CPU.
#[derive(Debug, PartialEq)]
pub struct ResourceDeclaration {
    /// Symbolic name for a CPU
    pub name: Id,
    /// The identifier for a CPU
    pub resource: Id,
    /// Global variables in the scope of the resource.
    ///
    /// Global variables are not in scope for other resources.
    pub global_vars: Vec<Declaration>,
    /// Defines the configuration of programs on this resource.
    pub tasks: Vec<TaskConfiguration>,
    /// Defines runnable programs.
    ///
    /// A runnable program can be associated with a task configuration
    /// by name.
    pub programs: Vec<ProgramConfiguration>,
}

#[derive(Debug, PartialEq)]
pub struct ProgramConfiguration {
    pub name: Id,
    pub task_name: Option<Id>,
    pub type_name: Id,
}

#[derive(Debug, PartialEq)]
pub struct ConfigurationDeclaration {
    pub name: Id,
    pub global_var: Vec<Declaration>,
    pub resource_decl: Vec<ResourceDeclaration>,
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

#[derive(PartialEq, Clone)]
pub struct DirectVariable {
    pub location: LocationPrefix,
    pub size: SizePrefix,
    pub address: Vec<u32>,
}

impl fmt::Debug for DirectVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirectVariable")
            .field("location", &self.location)
            .field("size", &self.size)
            .finish()
    }
}

impl fmt::Display for DirectVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirectVariable")
            .field("location", &self.location)
            .field("size", &self.size)
            .finish()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Sfc {
    pub networks: Vec<Network>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Statements {
    pub body: Vec<StmtKind>,
}

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
    pub fn stmts(stmts: Vec<StmtKind>) -> FunctionBlockBody {
        FunctionBlockBody::Statements(Statements { body: stmts })
    }

    pub fn sfc(networks: Vec<Network>) -> FunctionBlockBody {
        FunctionBlockBody::Sfc(Sfc { networks })
    }

    pub fn empty() -> FunctionBlockBody {
        FunctionBlockBody::Empty()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct FunctionDeclaration {
    pub name: Id,
    pub return_type: Id,
    // TODO rename these to be descriptive
    pub inputs: Vec<VarInitDecl>,
    pub outputs: Vec<VarInitDecl>,
    pub inouts: Vec<VarInitDecl>,
    pub vars: Vec<VarInitDecl>,
    pub externals: Vec<VarInitDecl>,
    // TODO other types
    pub body: Vec<StmtKind>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct FunctionBlockDeclaration {
    pub name: Id,
    pub inputs: Vec<VarInitDecl>,
    pub outputs: Vec<VarInitDecl>,
    pub inouts: Vec<VarInitDecl>,
    pub vars: Vec<VarInitDecl>,
    pub externals: Vec<VarInitDecl>,
    // TODO other var declarations
    pub body: FunctionBlockBody,
}

#[derive(Debug, PartialEq)]
pub struct ProgramDeclaration {
    pub type_name: Id,
    pub inputs: Vec<VarInitDecl>,
    pub outputs: Vec<VarInitDecl>,
    pub inouts: Vec<VarInitDecl>,
    pub vars: Vec<VarInitDecl>,
    // TODO other var declarations
    // TODO located var declarations
    // TODO other stuff here
    pub body: FunctionBlockBody,
}

#[derive(Debug, PartialEq)]
pub struct Library {
    pub elems: Vec<LibraryElement>,
}

impl Library {
    pub fn new(elems: Vec<LibraryElement>) -> Self {
        Library { elems }
    }
}
