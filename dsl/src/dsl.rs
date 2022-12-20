use core::str::FromStr;
use std::fmt;
use time::Duration;
use std::hash::{Hash, Hasher};

use crate::ast::*;
use crate::sfc::Network;

pub enum TypeDefinitionKind {
    Enumeration,
    FunctionBlock,
    Function,
    Structure,
}

#[derive(Debug, PartialEq)]
pub enum LibraryElement {
    DataTypeDeclaration(Vec<EnumerationDeclaration>),
    FunctionDeclaration(FunctionDeclaration),
    // TODO
    FunctionBlockDeclaration(FunctionBlockDeclaration),
    ProgramDeclaration(ProgramDeclaration),
    ConfigurationDeclaration(ConfigurationDeclaration),
}

pub struct Integer {
    value: String,
}

impl Integer {
    pub fn try_from<T: FromStr>(&self) -> T {
        let v: String = self.value.chars().filter(|c| c.is_digit(10)).collect();
        match v.parse::<T>() {
            Ok(v) => return v,
            Err(_) => panic!("out of range"),
        }
    }

    pub fn as_type<T: FromStr>(&self) -> T {
        self.try_from::<T>()
    }

    pub fn num_chars(&self) -> u8 {
        let value: String = self.value.chars().filter(|c| c.is_digit(10)).collect();
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
        let val = self.value.try_from::<T>();
        // TODO
        //if self.is_neg {
        //    val *= -1;
        //}
        val
    }
    pub fn from(a: &str) -> SignedInteger {
        match a.chars().nth(0) {
            Some('+') => {
                return SignedInteger {
                    value: Integer::from(a.get(1..).unwrap()),
                    is_neg: false,
                }
            }
            Some('-') => {
                return SignedInteger {
                    value: Integer::from(a.get(1..).unwrap()),
                    is_neg: true,
                }
            }
            _ => {
                return SignedInteger {
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
    pub data_type: Option<String>,
}

// TODO I don't know if I need to support multiple storage classes for the
// same value.
// 2.4.3 Declaration
#[derive(Debug, PartialEq, Clone)]
pub struct Declaration {
    pub name: Id,
    pub storage_class: StorageClass,
    pub at: Option<At>,
    pub initializer: Option<TypeInitializer>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct VarInitDecl {
    pub name: Id,
    pub storage_class: StorageClass,
    pub initializer: Option<TypeInitializer>,
    // TODO this need much more
}

impl VarInitDecl {
    /// Creates a variable declaration for simple type and no initialization.
    pub fn simple(name: &str, type_name: &str) -> VarInitDecl {
        VarInitDecl {
            name: Id::from(name),
            storage_class: StorageClass::Unspecified,
            initializer: Some(TypeInitializer::Simple {
                type_name: Id::from(type_name),
                initial_value: None,
            }),
        }
    }

    /// Creates a variable declaration for enumeration having an initial value.
    pub fn enumerated(name: &str, type_name: &str, initial_value: &str) -> VarInitDecl {
        VarInitDecl {
            name: Id::from(name),
            storage_class: StorageClass::Unspecified,
            initializer: Some(TypeInitializer::EnumeratedType(EnumeratedTypeInitializer {
                type_name: Id::from(type_name),
                initial_value: Some(Id::from(initial_value)),
            })),
        }
    }

    /// Creates a variable declaration for a function block.
    pub fn function_block(name: &str, type_name: &str) -> VarInitDecl {
        VarInitDecl {
            name: Id::from(name),
            storage_class: StorageClass::Unspecified,
            initializer: Some(TypeInitializer::FunctionBlock {
                type_name: Id::from(type_name),
            }),
        }
    }

    /// Creates a variable declaration that is ambiguous on the type.
    /// 
    /// The language has some ambiguity for types. The late bound represents
    /// a placeholder that is later resolved once all types are known.
    pub fn late_bound(name: &str, type_name: &str) -> VarInitDecl {
        VarInitDecl {
            name: Id::from(name),
            storage_class: StorageClass::Unspecified,
            initializer: Some(TypeInitializer::LateResolvedType(Id::from(type_name))),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct LocatedVarInit {
    pub name: Option<String>,
    pub storage_class: StorageClass,
    pub at: DirectVariable,
    pub initializer: TypeInitializer,
}

#[derive(Debug, PartialEq, Clone)]
pub enum VarInitKind {
    VarInit(VarInitDecl),
    LocatedVarInit(LocatedVarInit),
}

impl VarInitKind {
    pub fn simple(name: &str, type_name: &str) -> VarInitKind {
        VarInitKind::VarInit(VarInitDecl::simple(name, type_name))
    }

    pub fn enumerated(name: &str, type_name: &str, initial_value: &str) -> VarInitKind {
        VarInitKind::VarInit(VarInitDecl::enumerated(name, type_name, initial_value))
    }
    pub fn late_bound(name: &str, type_name: &str) -> VarInitKind {
        VarInitKind::VarInit(VarInitDecl::late_bound(name, type_name))
    }
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
pub enum TypeInitializer {
    Simple {
        type_name: Id,
        initial_value: Option<Initializer>,
    },
    EnumeratedValues {
        values: Vec<Id>,
        default: Option<Id>,
    },
    EnumeratedType(EnumeratedTypeInitializer),
    FunctionBlock {
        type_name: Id,
    },
    Structure {
        // TODO
        type_name: Id,
    },
    /// Type that is ambiguous until have discovered type
    /// definitions. Value is the name of the type.
    LateResolvedType(Id),
}

impl TypeInitializer {
    pub fn simple(type_name: &str, value: Initializer) -> TypeInitializer {
        TypeInitializer::Simple {
            type_name: Id::from(type_name),
            initial_value: Some(value)
        }
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
            'I' => return LocationPrefix::I,
            'Q' => return LocationPrefix::Q,
            'M' => return LocationPrefix::M,
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
            'X' => return SizePrefix::X,
            'B' => return SizePrefix::B,
            'W' => return SizePrefix::W,
            'D' => return SizePrefix::D,
            'L' => return SizePrefix::L,
            // TODO error message
            _ => panic!(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum StorageClass {
    // TODO Some of these are not valid for some contexts - should there be multiple
    // storage classes, indicate some how, or fail?
    Unspecified,
    Constant,
    Retain,
    NonRetain,
}

#[derive(Debug, PartialEq)]
pub struct ResourceDeclaration {
    pub name: Id,
    pub tasks: Vec<TaskConfiguration>,
    pub programs: Vec<ProgramConfiguration>,
}

#[derive(Debug, PartialEq)]
pub struct ProgramConfiguration {
    pub name: Id,
    pub task_name: Option<String>,
    pub type_name: String,
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
    pub default: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum EnumeratedSpecificationKind {
    TypeName(Id),
    /// Enumeration declaration that provides a list of values.
    /// 
    /// Order of the values is important because the order declares the
    /// default value if no default is specified directly.
    Values(Vec<Id>),
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
}

impl FunctionBlockBody {
    pub fn stmts(stmts: Vec<StmtKind>) -> FunctionBlockBody {
        FunctionBlockBody::Statements(Statements { body: stmts })
    }

    pub fn sfc(networks: Vec<Network>) -> FunctionBlockBody {
        FunctionBlockBody::Sfc(Sfc { networks: networks })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct FunctionDeclaration {
    pub name: Id,
    pub return_type: String,
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
        Library { elems: elems }
    }
}
