use core::str::FromStr;
use std::fmt;
use time::Duration;

use crate::ast::*;
use crate::sfc::Network;

use derive::EnumKind;

pub enum TypeDefinitionKind {
    Enumeration,
    FunctionBlock,
    Function,
    Structure,
}

#[derive(Debug, PartialEq, EnumKind)]
#[enum_kind(SomeEnumKind)]
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
    pub name: String,
    pub storage_class: StorageClass,
    pub at: Option<At>,
    pub initializer: Option<TypeInitializer>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct VarInitDecl {
    pub name: String,
    pub storage_class: StorageClass,
    pub initializer: Option<TypeInitializer>,
    // TODO this need much more
}

impl VarInitDecl {
    pub fn simple(name: &str, type_name: &str) -> VarInitDecl {
        VarInitDecl {
            name: String::from(name),
            storage_class: StorageClass::Unspecified,
            initializer: Some(TypeInitializer::Simple {
                type_name: String::from(type_name),
                initial_value: None,
            }),
        }
    }

    pub fn enumerated(name: &str, type_name: &str, initial_value: &str) -> VarInitDecl {
        VarInitDecl {
            name: String::from(name),
            storage_class: StorageClass::Unspecified,
            initializer: Some(TypeInitializer::EnumeratedType {
                type_name: String::from(type_name),
                initial_value: Some(String::from(initial_value)),
            }),
        }
    }

    pub fn function_block(name: &str, type_name: &str) -> VarInitDecl {
        VarInitDecl {
            name: String::from(name),
            storage_class: StorageClass::Unspecified,
            initializer: Some(TypeInitializer::FunctionBlock {
                type_name: String::from(type_name),
            }),
        }
    }

    pub fn late_bound(name: &str, type_name: &str) -> VarInitDecl {
        VarInitDecl {
            name: String::from(name),
            storage_class: StorageClass::Unspecified,
            initializer: Some(TypeInitializer::LateResolvedType(String::from(type_name))),
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
    pub name: String,
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
pub enum TypeInitializer {
    Simple {
        type_name: String,
        initial_value: Option<Initializer>,
    },
    EnumeratedValues {
        values: Vec<String>,
        default: Option<String>,
    },
    EnumeratedType {
        type_name: String,
        initial_value: Option<String>,
    },
    FunctionBlock {
        type_name: String,
    },
    Structure {
        // TODO
        type_name: String,
    },
    // A type that is ambiguous until we have discovered type
    // definitions. Value is the name of the type.
    LateResolvedType(String),
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
    pub name: String,
    pub tasks: Vec<TaskConfiguration>,
    pub programs: Vec<ProgramConfiguration>,
}

#[derive(Debug, PartialEq)]
pub struct ProgramConfiguration {
    pub name: String,
    pub task_name: Option<String>,
    pub type_name: String,
}

#[derive(Debug, PartialEq)]
pub struct ConfigurationDeclaration {
    pub name: String,
    pub global_var: Vec<Declaration>,
    pub resource_decl: Vec<ResourceDeclaration>,
}

#[derive(Debug, PartialEq)]
pub struct EnumerationDeclaration {
    pub name: String,
    // TODO need to understand when the context name matters in the definition
    pub initializer: TypeInitializer,
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
    pub name: String,
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
    pub name: String,
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
    pub type_name: String,
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
