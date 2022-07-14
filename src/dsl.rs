use core::str::FromStr;
use std::fmt;
use time::Duration;

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

    pub fn from(a: &str) -> Integer {
        Integer {
            value: String::from(a),
        }
    }
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

// TODO rename this to VarDecl
#[derive(Debug, PartialEq, Clone)]
pub struct VarInit {
    pub name: String,
    pub storage_class: StorageClass,
    pub initializer: Option<TypeInitializer>,
    // TODO this need much more
}

#[derive(Debug, PartialEq, Clone)]
pub struct LocatedVarInit {
    pub name: Option<String>,
    pub storage_class: StorageClass,
    pub at: DirectVariable,
    pub initializer: TypeInitializer,
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
    RealLiteral(),
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
pub struct TypeInitializer {
    pub type_name: String,
    pub initial_value: Option<Initializer>,
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
    Unspecified(),
    Constant(),
    Retain(),
    NonRetain(),
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
    pub values: Vec<String>,
    pub default: Option<String>,
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
