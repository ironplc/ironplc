//! PLCopen XML schema structures
//!
//! These structs map to the PLCopen TC6 XML v2.01 schema.
//! Only the elements needed for IronPLC are implemented.
//! These are plain data structures populated by the roxmltree-based parser.

/// The root project element
#[derive(Debug, Default)]
pub struct Project {
    pub file_header: FileHeader,
    pub content_header: ContentHeader,
    pub types: Types,
    pub instances: Option<Instances>,
}

/// File header with metadata about the exporting tool
#[derive(Debug, Default)]
pub struct FileHeader {
    pub company_name: String,
    pub company_url: Option<String>,
    pub product_name: String,
    pub product_version: String,
    pub product_release: Option<String>,
    pub creation_date_time: String,
    pub content_description: Option<String>,
}

/// Content header with project metadata
#[derive(Debug, Default)]
pub struct ContentHeader {
    pub name: String,
    pub version: Option<String>,
    pub modification_date_time: Option<String>,
    pub organization: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub comment: Option<String>,
    pub coordinate_info: CoordinateInfo,
}

/// Coordinate information for graphical editors (we ignore the details)
#[derive(Debug, Default)]
pub struct CoordinateInfo {
    pub fbd: Scaling,
    pub ld: Scaling,
    pub sfc: Scaling,
}

/// Scaling information (we just capture it, don't use it)
#[derive(Debug, Default)]
pub struct Scaling {
    pub scaling: ScalingValue,
}

#[derive(Debug, Default)]
pub struct ScalingValue {
    pub x: String,
    pub y: String,
}

/// Container for types (data types and POUs)
#[derive(Debug, Default)]
pub struct Types {
    pub data_types: DataTypes,
    pub pous: Pous,
}

/// Container for data type declarations
#[derive(Debug, Default)]
pub struct DataTypes {
    pub data_type: Vec<DataTypeDecl>,
}

/// A data type declaration
#[derive(Debug)]
pub struct DataTypeDecl {
    pub name: String,
    pub base_type: DataType,
    pub initial_value: Option<Value>,
}

/// A data type reference (exactly one variant)
///
/// In PLCopen XML, a type element contains exactly one child element
/// indicating the type. This enum represents that constraint.
#[derive(Debug, Clone)]
pub enum DataType {
    // Elementary types (empty elements)
    Bool,
    Byte,
    Word,
    DWord,
    LWord,
    SInt,
    Int,
    DInt,
    LInt,
    USInt,
    UInt,
    UDInt,
    ULInt,
    Real,
    LReal,
    Time,
    Date,
    DateAndTime,
    TimeOfDay,

    // String types with optional length
    String { length: Option<String> },
    WString { length: Option<String> },

    // Generic ANY types
    Any,
    AnyDerived,
    AnyElementary,
    AnyMagnitude,
    AnyNum,
    AnyReal,
    AnyInt,
    AnyBit,
    AnyString,
    AnyDate,

    // Derived types
    Derived(DerivedType),
    Array(Box<ArrayType>),
    Enum(EnumType),
    Struct(StructType),
    SubrangeSigned(Box<SubrangeSigned>),
    SubrangeUnsigned(Box<SubrangeUnsigned>),
    Pointer(Box<PointerType>),
}

impl Default for DataType {
    fn default() -> Self {
        DataType::Bool
    }
}

/// Reference to a named type
#[derive(Debug, Clone)]
pub struct DerivedType {
    pub name: String,
}

/// Array type
#[derive(Debug, Clone)]
pub struct ArrayType {
    pub dimension: Vec<Dimension>,
    pub base_type: DataType,
}

/// Array dimension
#[derive(Debug, Clone)]
pub struct Dimension {
    pub lower: String,
    pub upper: String,
}

/// Enumeration type
#[derive(Debug, Clone)]
pub struct EnumType {
    pub values: EnumValues,
    pub base_type: Option<Box<DataType>>,
}

/// Container for enumeration values
#[derive(Debug, Clone)]
pub struct EnumValues {
    pub value: Vec<EnumValue>,
}

/// Single enumeration value
#[derive(Debug, Clone)]
pub struct EnumValue {
    pub name: String,
    pub value: Option<String>,
}

/// Structure type
#[derive(Debug, Clone)]
pub struct StructType {
    pub variable: Vec<StructMember>,
}

/// Structure member (uses same structure as variable)
#[derive(Debug, Clone)]
pub struct StructMember {
    pub name: String,
    pub member_type: DataType,
    pub initial_value: Option<Value>,
}

/// Signed subrange type
#[derive(Debug, Clone)]
pub struct SubrangeSigned {
    pub lower: String,
    pub upper: String,
    pub base_type: DataType,
}

/// Unsigned subrange type
#[derive(Debug, Clone)]
pub struct SubrangeUnsigned {
    pub lower: String,
    pub upper: String,
    pub base_type: DataType,
}

/// Pointer type
#[derive(Debug, Clone)]
pub struct PointerType {
    pub base_type: DataType,
}

/// Container for POUs
#[derive(Debug, Default)]
pub struct Pous {
    pub pou: Vec<Pou>,
}

/// Program Organization Unit
#[derive(Debug)]
pub struct Pou {
    pub name: String,
    pub pou_type: PouType,
    pub global_id: Option<String>,
    pub interface: Option<Interface>,
    pub body: Option<Body>,
    pub actions: Option<Actions>,
    pub transitions: Option<Transitions>,
}

/// POU type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PouType {
    Function,
    FunctionBlock,
    Program,
}

impl Default for PouType {
    fn default() -> Self {
        PouType::Program
    }
}

/// POU interface (variable declarations)
#[derive(Debug, Default)]
pub struct Interface {
    pub return_type: Option<DataType>,
    pub local_vars: Vec<VarList>,
    pub temp_vars: Vec<VarList>,
    pub input_vars: Vec<VarList>,
    pub output_vars: Vec<VarList>,
    pub in_out_vars: Vec<VarList>,
    pub external_vars: Vec<VarList>,
    pub global_vars: Vec<VarList>,
}

/// List of variables with shared attributes
#[derive(Debug, Default)]
pub struct VarList {
    pub name: Option<String>,
    pub constant: bool,
    pub retain: bool,
    pub nonretain: bool,
    pub persistent: bool,
    pub variable: Vec<Variable>,
}

/// Variable declaration
#[derive(Debug)]
pub struct Variable {
    pub name: String,
    pub address: Option<String>,
    pub global_id: Option<String>,
    pub var_type: DataType,
    pub initial_value: Option<Value>,
}

/// POU body (implementation)
#[derive(Debug, Default)]
pub struct Body {
    pub worksheet_name: Option<String>,
    pub global_id: Option<String>,
    pub st: Option<StBody>,
    pub il: Option<String>,
    pub fbd: bool,
    pub ld: bool,
    pub sfc: bool,
}

/// ST body with text content and position information
#[derive(Debug, Clone)]
pub struct StBody {
    pub text: String,
    /// Line offset (0-based) where the text content starts
    pub line_offset: usize,
    /// Column offset (0-based) where the text content starts
    pub col_offset: usize,
}

/// Container for actions
#[derive(Debug, Default)]
pub struct Actions {
    pub action: Vec<Action>,
}

/// Action definition
#[derive(Debug)]
pub struct Action {
    pub name: String,
    pub global_id: Option<String>,
    pub body: Body,
}

/// Container for transitions
#[derive(Debug, Default)]
pub struct Transitions {
    pub transition: Vec<Transition>,
}

/// Transition definition
#[derive(Debug)]
pub struct Transition {
    pub name: String,
    pub global_id: Option<String>,
    pub body: Body,
}

/// Value type (for initial values)
#[derive(Debug, Clone, Default)]
pub struct Value {
    pub simple_value: Option<SimpleValue>,
    pub array_value: Option<ArrayValue>,
    pub struct_value: Option<StructValue>,
}

/// Simple value
#[derive(Debug, Clone)]
pub struct SimpleValue {
    pub value: Option<String>,
}

/// Array value
#[derive(Debug, Clone)]
pub struct ArrayValue {
    pub value: Vec<ArrayValueElement>,
}

/// Array value element with optional repetition
#[derive(Debug, Clone)]
pub struct ArrayValueElement {
    pub repetition_value: Option<String>,
    pub value: Value,
}

/// Struct value
#[derive(Debug, Clone)]
pub struct StructValue {
    pub value: Vec<StructValueElement>,
}

/// Struct value element
#[derive(Debug, Clone)]
pub struct StructValueElement {
    pub member: String,
    pub value: Value,
}

/// Container for instances (configurations)
#[derive(Debug, Default)]
pub struct Instances {
    pub configurations: Configurations,
}

/// Container for configurations
#[derive(Debug, Default)]
pub struct Configurations {
    pub configuration: Vec<Configuration>,
}

/// Configuration declaration
#[derive(Debug)]
pub struct Configuration {
    pub name: String,
    pub global_id: Option<String>,
    pub resource: Vec<Resource>,
    pub global_vars: Vec<VarList>,
}

/// Resource declaration
#[derive(Debug)]
pub struct Resource {
    pub name: String,
    pub global_id: Option<String>,
    pub task: Vec<Task>,
    pub global_vars: Vec<VarList>,
    pub pou_instance: Vec<PouInstance>,
}

/// Task configuration
#[derive(Debug)]
pub struct Task {
    pub name: String,
    pub priority: String,
    pub interval: Option<String>,
    pub single: Option<String>,
    pub global_id: Option<String>,
    pub pou_instance: Vec<PouInstance>,
}

/// POU instance (program instance)
#[derive(Debug)]
pub struct PouInstance {
    pub name: String,
    pub type_name: String,
    pub global_id: Option<String>,
}

impl DataType {
    /// Get the type name as a string for diagnostics
    pub fn type_name(&self) -> &'static str {
        match self {
            DataType::Bool => "BOOL",
            DataType::Byte => "BYTE",
            DataType::Word => "WORD",
            DataType::DWord => "DWORD",
            DataType::LWord => "LWORD",
            DataType::SInt => "SINT",
            DataType::Int => "INT",
            DataType::DInt => "DINT",
            DataType::LInt => "LINT",
            DataType::USInt => "USINT",
            DataType::UInt => "UINT",
            DataType::UDInt => "UDINT",
            DataType::ULInt => "ULINT",
            DataType::Real => "REAL",
            DataType::LReal => "LREAL",
            DataType::Time => "TIME",
            DataType::Date => "DATE",
            DataType::DateAndTime => "DATE_AND_TIME",
            DataType::TimeOfDay => "TIME_OF_DAY",
            DataType::String { .. } => "STRING",
            DataType::WString { .. } => "WSTRING",
            DataType::Any => "ANY",
            DataType::AnyDerived => "ANY_DERIVED",
            DataType::AnyElementary => "ANY_ELEMENTARY",
            DataType::AnyMagnitude => "ANY_MAGNITUDE",
            DataType::AnyNum => "ANY_NUM",
            DataType::AnyReal => "ANY_REAL",
            DataType::AnyInt => "ANY_INT",
            DataType::AnyBit => "ANY_BIT",
            DataType::AnyString => "ANY_STRING",
            DataType::AnyDate => "ANY_DATE",
            DataType::Derived(_) => "derived",
            DataType::Array(_) => "ARRAY",
            DataType::Enum(_) => "enum",
            DataType::Struct(_) => "STRUCT",
            DataType::SubrangeSigned(_) => "subrange",
            DataType::SubrangeUnsigned(_) => "subrange",
            DataType::Pointer(_) => "POINTER",
        }
    }
}

impl Body {
    /// Check if this body uses Structured Text
    pub fn is_st(&self) -> bool {
        self.st.is_some()
    }

    /// Check if this body uses an unsupported language
    pub fn unsupported_language(&self) -> Option<&'static str> {
        if self.il.is_some() {
            Some("IL")
        } else if self.fbd {
            Some("FBD")
        } else if self.ld {
            Some("LD")
        } else if self.sfc {
            Some("SFC")
        } else {
            None
        }
    }

    /// Get the ST body if present
    pub fn st_body(&self) -> Option<&StBody> {
        self.st.as_ref()
    }

    /// Get the ST body text if present
    pub fn st_text(&self) -> Option<&str> {
        self.st.as_ref().map(|st| st.text.as_str())
    }
}
