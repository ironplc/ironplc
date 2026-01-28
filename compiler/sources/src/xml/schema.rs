//! PLCopen XML schema structures
//!
//! These structs map to the PLCopen TC6 XML v2.01 schema.
//! Only the elements needed for IronPLC are implemented.

use serde::Deserialize;

/// The root project element
#[derive(Debug, Deserialize)]
#[serde(rename = "project")]
pub struct Project {
    #[serde(rename = "fileHeader")]
    pub file_header: FileHeader,

    #[serde(rename = "contentHeader")]
    pub content_header: ContentHeader,

    #[serde(rename = "types")]
    pub types: Types,

    #[serde(rename = "instances", default)]
    pub instances: Option<Instances>,
}

/// File header with metadata about the exporting tool
#[derive(Debug, Deserialize)]
pub struct FileHeader {
    #[serde(rename = "@companyName")]
    pub company_name: String,

    #[serde(rename = "@companyURL", default)]
    pub company_url: Option<String>,

    #[serde(rename = "@productName")]
    pub product_name: String,

    #[serde(rename = "@productVersion")]
    pub product_version: String,

    #[serde(rename = "@productRelease", default)]
    pub product_release: Option<String>,

    #[serde(rename = "@creationDateTime")]
    pub creation_date_time: String,

    #[serde(rename = "@contentDescription", default)]
    pub content_description: Option<String>,
}

/// Content header with project metadata
#[derive(Debug, Deserialize)]
pub struct ContentHeader {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@version", default)]
    pub version: Option<String>,

    #[serde(rename = "@modificationDateTime", default)]
    pub modification_date_time: Option<String>,

    #[serde(rename = "@organization", default)]
    pub organization: Option<String>,

    #[serde(rename = "@author", default)]
    pub author: Option<String>,

    #[serde(rename = "@language", default)]
    pub language: Option<String>,

    #[serde(rename = "Comment", default)]
    pub comment: Option<String>,

    #[serde(rename = "coordinateInfo")]
    pub coordinate_info: CoordinateInfo,
}

/// Coordinate information for graphical editors (we ignore the details)
#[derive(Debug, Deserialize)]
pub struct CoordinateInfo {
    #[serde(rename = "fbd")]
    pub fbd: Scaling,

    #[serde(rename = "ld")]
    pub ld: Scaling,

    #[serde(rename = "sfc")]
    pub sfc: Scaling,
}

/// Scaling information (we just capture it, don't use it)
#[derive(Debug, Deserialize)]
pub struct Scaling {
    #[serde(rename = "scaling")]
    pub scaling: ScalingValue,
}

#[derive(Debug, Deserialize)]
pub struct ScalingValue {
    #[serde(rename = "@x")]
    pub x: String,

    #[serde(rename = "@y")]
    pub y: String,
}

/// Container for types (data types and POUs)
#[derive(Debug, Deserialize)]
pub struct Types {
    #[serde(rename = "dataTypes")]
    pub data_types: DataTypes,

    #[serde(rename = "pous")]
    pub pous: Pous,
}

/// Container for data type declarations
#[derive(Debug, Deserialize)]
pub struct DataTypes {
    #[serde(rename = "dataType", default)]
    pub data_type: Vec<DataTypeDecl>,
}

/// A data type declaration
#[derive(Debug, Deserialize)]
pub struct DataTypeDecl {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "baseType")]
    pub base_type: TypeElement,

    #[serde(rename = "initialValue", default)]
    pub initial_value: Option<Value>,
}

/// Wrapper for type elements in XML
///
/// In PLCopen XML, type information is wrapped in elements like `<type>`, `<baseType>`.
/// This wrapper uses `$value` to capture the inner DataType enum.
#[derive(Debug, Deserialize)]
pub struct TypeElement {
    #[serde(rename = "$value")]
    pub inner: DataType,
}

impl TypeElement {
    /// Get the inner data type
    pub fn data_type(&self) -> &DataType {
        &self.inner
    }
}

impl std::ops::Deref for TypeElement {
    type Target = DataType;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// A data type reference (exactly one variant)
///
/// In PLCopen XML, a type element contains exactly one child element
/// indicating the type. This enum represents that constraint.
#[derive(Debug, Deserialize)]
pub enum DataType {
    // Elementary types (empty elements)
    #[serde(rename = "BOOL")]
    Bool,
    #[serde(rename = "BYTE")]
    Byte,
    #[serde(rename = "WORD")]
    Word,
    #[serde(rename = "DWORD")]
    DWord,
    #[serde(rename = "LWORD")]
    LWord,
    #[serde(rename = "SINT")]
    SInt,
    #[serde(rename = "INT")]
    Int,
    #[serde(rename = "DINT")]
    DInt,
    #[serde(rename = "LINT")]
    LInt,
    #[serde(rename = "USINT")]
    USInt,
    #[serde(rename = "UINT")]
    UInt,
    #[serde(rename = "UDINT")]
    UDInt,
    #[serde(rename = "ULINT")]
    ULInt,
    #[serde(rename = "REAL")]
    Real,
    #[serde(rename = "LREAL")]
    LReal,
    #[serde(rename = "TIME")]
    Time,
    #[serde(rename = "DATE")]
    Date,
    #[serde(rename = "DT")]
    DateAndTime,
    #[serde(rename = "TOD")]
    TimeOfDay,

    // String types with optional length
    #[serde(rename = "string")]
    String {
        #[serde(rename = "@length", default)]
        length: Option<String>,
    },
    #[serde(rename = "wstring")]
    WString {
        #[serde(rename = "@length", default)]
        length: Option<String>,
    },

    // Generic ANY types
    #[serde(rename = "ANY")]
    Any,
    #[serde(rename = "ANY_DERIVED")]
    AnyDerived,
    #[serde(rename = "ANY_ELEMENTARY")]
    AnyElementary,
    #[serde(rename = "ANY_MAGNITUDE")]
    AnyMagnitude,
    #[serde(rename = "ANY_NUM")]
    AnyNum,
    #[serde(rename = "ANY_REAL")]
    AnyReal,
    #[serde(rename = "ANY_INT")]
    AnyInt,
    #[serde(rename = "ANY_BIT")]
    AnyBit,
    #[serde(rename = "ANY_STRING")]
    AnyString,
    #[serde(rename = "ANY_DATE")]
    AnyDate,

    // Derived types
    #[serde(rename = "derived")]
    Derived(DerivedType),
    #[serde(rename = "array")]
    Array(Box<ArrayType>),
    #[serde(rename = "enum")]
    Enum(EnumType),
    #[serde(rename = "struct")]
    Struct(StructType),
    #[serde(rename = "subrangeSigned")]
    SubrangeSigned(Box<SubrangeSigned>),
    #[serde(rename = "subrangeUnsigned")]
    SubrangeUnsigned(Box<SubrangeUnsigned>),
    #[serde(rename = "pointer")]
    Pointer(Box<PointerType>),
}

/// Reference to a named type
#[derive(Debug, Deserialize)]
pub struct DerivedType {
    #[serde(rename = "@name")]
    pub name: String,
}

/// Array type
#[derive(Debug, Deserialize)]
pub struct ArrayType {
    #[serde(rename = "dimension")]
    pub dimension: Vec<Dimension>,

    #[serde(rename = "baseType")]
    pub base_type: TypeElement,
}

/// Array dimension
#[derive(Debug, Deserialize)]
pub struct Dimension {
    #[serde(rename = "@lower")]
    pub lower: String,

    #[serde(rename = "@upper")]
    pub upper: String,
}

/// Enumeration type
#[derive(Debug, Deserialize)]
pub struct EnumType {
    #[serde(rename = "values")]
    pub values: EnumValues,

    #[serde(rename = "baseType", default)]
    pub base_type: Option<Box<TypeElement>>,
}

/// Container for enumeration values
#[derive(Debug, Deserialize)]
pub struct EnumValues {
    #[serde(rename = "value")]
    pub value: Vec<EnumValue>,
}

/// Single enumeration value
#[derive(Debug, Deserialize)]
pub struct EnumValue {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@value", default)]
    pub value: Option<String>,
}

/// Structure type
#[derive(Debug, Deserialize)]
pub struct StructType {
    #[serde(rename = "variable")]
    pub variable: Vec<StructMember>,
}

/// Structure member (uses same structure as variable)
#[derive(Debug, Deserialize)]
pub struct StructMember {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "type")]
    pub member_type: TypeElement,

    #[serde(rename = "initialValue", default)]
    pub initial_value: Option<Value>,
}

/// Signed subrange type
#[derive(Debug, Deserialize)]
pub struct SubrangeSigned {
    #[serde(rename = "@lower")]
    pub lower: String,

    #[serde(rename = "@upper")]
    pub upper: String,

    #[serde(rename = "baseType")]
    pub base_type: TypeElement,
}

/// Unsigned subrange type
#[derive(Debug, Deserialize)]
pub struct SubrangeUnsigned {
    #[serde(rename = "@lower")]
    pub lower: String,

    #[serde(rename = "@upper")]
    pub upper: String,

    #[serde(rename = "baseType")]
    pub base_type: TypeElement,
}

/// Pointer type
#[derive(Debug, Deserialize)]
pub struct PointerType {
    #[serde(rename = "baseType")]
    pub base_type: TypeElement,
}

/// Container for POUs
#[derive(Debug, Deserialize)]
pub struct Pous {
    #[serde(rename = "pou", default)]
    pub pou: Vec<Pou>,
}

/// Program Organization Unit
#[derive(Debug, Deserialize)]
pub struct Pou {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@pouType")]
    pub pou_type: PouType,

    #[serde(rename = "@globalId", default)]
    pub global_id: Option<String>,

    #[serde(rename = "interface", default)]
    pub interface: Option<Interface>,

    #[serde(rename = "body", default)]
    pub body: Option<Body>,

    #[serde(rename = "actions", default)]
    pub actions: Option<Actions>,

    #[serde(rename = "transitions", default)]
    pub transitions: Option<Transitions>,
}

/// POU type enumeration
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PouType {
    Function,
    FunctionBlock,
    Program,
}

/// POU interface (variable declarations)
#[derive(Debug, Deserialize, Default)]
pub struct Interface {
    #[serde(rename = "returnType", default)]
    pub return_type: Option<TypeElement>,

    #[serde(rename = "localVars", default)]
    pub local_vars: Vec<VarList>,

    #[serde(rename = "tempVars", default)]
    pub temp_vars: Vec<VarList>,

    #[serde(rename = "inputVars", default)]
    pub input_vars: Vec<VarList>,

    #[serde(rename = "outputVars", default)]
    pub output_vars: Vec<VarList>,

    #[serde(rename = "inOutVars", default)]
    pub in_out_vars: Vec<VarList>,

    #[serde(rename = "externalVars", default)]
    pub external_vars: Vec<VarList>,

    #[serde(rename = "globalVars", default)]
    pub global_vars: Vec<VarList>,
}

/// List of variables with shared attributes
#[derive(Debug, Deserialize)]
pub struct VarList {
    #[serde(rename = "@name", default)]
    pub name: Option<String>,

    #[serde(rename = "@constant", default)]
    pub constant: bool,

    #[serde(rename = "@retain", default)]
    pub retain: bool,

    #[serde(rename = "@nonretain", default)]
    pub nonretain: bool,

    #[serde(rename = "@persistent", default)]
    pub persistent: bool,

    #[serde(rename = "variable", default)]
    pub variable: Vec<Variable>,
}

/// Variable declaration
#[derive(Debug, Deserialize)]
pub struct Variable {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@address", default)]
    pub address: Option<String>,

    #[serde(rename = "@globalId", default)]
    pub global_id: Option<String>,

    #[serde(rename = "type")]
    pub var_type: TypeElement,

    #[serde(rename = "initialValue", default)]
    pub initial_value: Option<Value>,
}

/// POU body (implementation)
#[derive(Debug, Deserialize)]
pub struct Body {
    #[serde(rename = "@WorksheetName", default)]
    pub worksheet_name: Option<String>,

    #[serde(rename = "@globalId", default)]
    pub global_id: Option<String>,

    #[serde(rename = "ST", default)]
    pub st: Option<FormattedText>,

    #[serde(rename = "IL", default)]
    pub il: Option<FormattedText>,

    #[serde(rename = "FBD", default)]
    pub fbd: Option<serde::de::IgnoredAny>,

    #[serde(rename = "LD", default)]
    pub ld: Option<serde::de::IgnoredAny>,

    #[serde(rename = "SFC", default)]
    pub sfc: Option<serde::de::IgnoredAny>,
}

/// Formatted text (XHTML content) - we extract the text content
#[derive(Debug, Deserialize)]
pub struct FormattedText {
    #[serde(rename = "xhtml", default)]
    pub xhtml: Option<XhtmlContent>,

    /// Fallback for plain text content
    #[serde(rename = "$text", default)]
    pub text: Option<String>,
}

/// XHTML wrapper element
#[derive(Debug, Deserialize)]
pub struct XhtmlContent {
    #[serde(rename = "$text", default)]
    pub text: Option<String>,
}

/// Container for actions
#[derive(Debug, Deserialize)]
pub struct Actions {
    #[serde(rename = "action", default)]
    pub action: Vec<Action>,
}

/// Action definition
#[derive(Debug, Deserialize)]
pub struct Action {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@globalId", default)]
    pub global_id: Option<String>,

    #[serde(rename = "body")]
    pub body: Body,
}

/// Container for transitions
#[derive(Debug, Deserialize)]
pub struct Transitions {
    #[serde(rename = "transition", default)]
    pub transition: Vec<Transition>,
}

/// Transition definition
#[derive(Debug, Deserialize)]
pub struct Transition {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@globalId", default)]
    pub global_id: Option<String>,

    #[serde(rename = "body")]
    pub body: Body,
}

/// Value type (for initial values)
#[derive(Debug, Deserialize)]
pub struct Value {
    #[serde(rename = "simpleValue", default)]
    pub simple_value: Option<SimpleValue>,

    #[serde(rename = "arrayValue", default)]
    pub array_value: Option<ArrayValue>,

    #[serde(rename = "structValue", default)]
    pub struct_value: Option<StructValue>,
}

/// Simple value
#[derive(Debug, Deserialize)]
pub struct SimpleValue {
    #[serde(rename = "@value", default)]
    pub value: Option<String>,
}

/// Array value
#[derive(Debug, Deserialize)]
pub struct ArrayValue {
    #[serde(rename = "value", default)]
    pub value: Vec<ArrayValueElement>,
}

/// Array value element with optional repetition
#[derive(Debug, Deserialize)]
pub struct ArrayValueElement {
    #[serde(rename = "@repetitionValue", default)]
    pub repetition_value: Option<String>,

    #[serde(flatten)]
    pub value: Value,
}

/// Struct value
#[derive(Debug, Deserialize)]
pub struct StructValue {
    #[serde(rename = "value", default)]
    pub value: Vec<StructValueElement>,
}

/// Struct value element
#[derive(Debug, Deserialize)]
pub struct StructValueElement {
    #[serde(rename = "@member")]
    pub member: String,

    #[serde(flatten)]
    pub value: Value,
}

/// Container for instances (configurations)
#[derive(Debug, Deserialize)]
pub struct Instances {
    #[serde(rename = "configurations")]
    pub configurations: Configurations,
}

/// Container for configurations
#[derive(Debug, Deserialize)]
pub struct Configurations {
    #[serde(rename = "configuration", default)]
    pub configuration: Vec<Configuration>,
}

/// Configuration declaration
#[derive(Debug, Deserialize)]
pub struct Configuration {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@globalId", default)]
    pub global_id: Option<String>,

    #[serde(rename = "resource", default)]
    pub resource: Vec<Resource>,

    #[serde(rename = "globalVars", default)]
    pub global_vars: Vec<VarList>,
}

/// Resource declaration
#[derive(Debug, Deserialize)]
pub struct Resource {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@globalId", default)]
    pub global_id: Option<String>,

    #[serde(rename = "task", default)]
    pub task: Vec<Task>,

    #[serde(rename = "globalVars", default)]
    pub global_vars: Vec<VarList>,

    #[serde(rename = "pouInstance", default)]
    pub pou_instance: Vec<PouInstance>,
}

/// Task configuration
#[derive(Debug, Deserialize)]
pub struct Task {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@priority")]
    pub priority: String,

    #[serde(rename = "@interval", default)]
    pub interval: Option<String>,

    #[serde(rename = "@single", default)]
    pub single: Option<String>,

    #[serde(rename = "@globalId", default)]
    pub global_id: Option<String>,

    #[serde(rename = "pouInstance", default)]
    pub pou_instance: Vec<PouInstance>,
}

/// POU instance (program instance)
#[derive(Debug, Deserialize)]
pub struct PouInstance {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@typeName")]
    pub type_name: String,

    #[serde(rename = "@globalId", default)]
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

impl FormattedText {
    /// Extract the text content from the formatted text
    pub fn text_content(&self) -> Option<&str> {
        // Try xhtml content first
        if let Some(ref xhtml) = self.xhtml {
            if let Some(ref text) = xhtml.text {
                return Some(text.as_str());
            }
        }
        // Fall back to plain text
        self.text.as_deref()
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
        } else if self.fbd.is_some() {
            Some("FBD")
        } else if self.ld.is_some() {
            Some("LD")
        } else if self.sfc.is_some() {
            Some("SFC")
        } else {
            None
        }
    }

    /// Get the ST body text if present
    pub fn st_text(&self) -> Option<&str> {
        self.st.as_ref().and_then(|st| st.text_content())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_when_minimal_project_then_succeeds() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="Test" productName="Test" productVersion="1.0" creationDateTime="2024-01-01T00:00:00"/>
  <contentHeader name="TestProject">
    <coordinateInfo>
      <fbd><scaling x="1" y="1"/></fbd>
      <ld><scaling x="1" y="1"/></ld>
      <sfc><scaling x="1" y="1"/></sfc>
    </coordinateInfo>
  </contentHeader>
  <types>
    <dataTypes/>
    <pous/>
  </types>
</project>"#;

        let project: Project = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(project.file_header.company_name, "Test");
        assert_eq!(project.content_header.name, "TestProject");
        assert!(project.types.pous.pou.is_empty());
    }

    #[test]
    fn parse_when_function_block_with_variables_then_extracts_correctly() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="Test" productName="Test" productVersion="1.0" creationDateTime="2024-01-01T00:00:00"/>
  <contentHeader name="TestProject">
    <coordinateInfo>
      <fbd><scaling x="1" y="1"/></fbd>
      <ld><scaling x="1" y="1"/></ld>
      <sfc><scaling x="1" y="1"/></sfc>
    </coordinateInfo>
  </contentHeader>
  <types>
    <dataTypes/>
    <pous>
      <pou name="Counter" pouType="functionBlock">
        <interface>
          <inputVars>
            <variable name="Reset">
              <type><BOOL/></type>
            </variable>
          </inputVars>
          <outputVars>
            <variable name="Count">
              <type><INT/></type>
            </variable>
          </outputVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">
IF Reset THEN
  Count := 0;
END_IF;
            </xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;

        let project: Project = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(project.types.pous.pou.len(), 1);
        let pou = &project.types.pous.pou[0];
        assert_eq!(pou.name, "Counter");
        assert_eq!(pou.pou_type, PouType::FunctionBlock);

        let interface = pou.interface.as_ref().unwrap();
        assert_eq!(interface.input_vars.len(), 1);
        assert_eq!(interface.input_vars[0].variable.len(), 1);
        assert_eq!(interface.input_vars[0].variable[0].name, "Reset");
        assert!(matches!(*interface.input_vars[0].variable[0].var_type, DataType::Bool));

        let body = pou.body.as_ref().unwrap();
        assert!(body.is_st());
        assert!(body.st_text().unwrap().contains("IF Reset THEN"));
    }

    #[test]
    fn parse_when_enumeration_type_then_extracts_values() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="Test" productName="Test" productVersion="1.0" creationDateTime="2024-01-01T00:00:00"/>
  <contentHeader name="TestProject">
    <coordinateInfo>
      <fbd><scaling x="1" y="1"/></fbd>
      <ld><scaling x="1" y="1"/></ld>
      <sfc><scaling x="1" y="1"/></sfc>
    </coordinateInfo>
  </contentHeader>
  <types>
    <dataTypes>
      <dataType name="TrafficLight">
        <baseType>
          <enum>
            <values>
              <value name="Red"/>
              <value name="Yellow"/>
              <value name="Green"/>
            </values>
          </enum>
        </baseType>
      </dataType>
    </dataTypes>
    <pous/>
  </types>
</project>"#;

        let project: Project = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(project.types.data_types.data_type.len(), 1);
        let dt = &project.types.data_types.data_type[0];
        assert_eq!(dt.name, "TrafficLight");

        let DataType::Enum(enum_type) = &*dt.base_type else {
            panic!("Expected enum type");
        };
        assert_eq!(enum_type.values.value.len(), 3);
        assert_eq!(enum_type.values.value[0].name, "Red");
        assert_eq!(enum_type.values.value[1].name, "Yellow");
        assert_eq!(enum_type.values.value[2].name, "Green");
    }

    #[test]
    fn parse_when_array_type_then_extracts_dimensions() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="Test" productName="Test" productVersion="1.0" creationDateTime="2024-01-01T00:00:00"/>
  <contentHeader name="TestProject">
    <coordinateInfo>
      <fbd><scaling x="1" y="1"/></fbd>
      <ld><scaling x="1" y="1"/></ld>
      <sfc><scaling x="1" y="1"/></sfc>
    </coordinateInfo>
  </contentHeader>
  <types>
    <dataTypes>
      <dataType name="IntArray">
        <baseType>
          <array>
            <dimension lower="0" upper="9"/>
            <baseType><INT/></baseType>
          </array>
        </baseType>
      </dataType>
    </dataTypes>
    <pous/>
  </types>
</project>"#;

        let project: Project = quick_xml::de::from_str(xml).unwrap();

        let dt = &project.types.data_types.data_type[0];
        assert_eq!(dt.name, "IntArray");

        let DataType::Array(array) = &*dt.base_type else {
            panic!("Expected array type");
        };
        assert_eq!(array.dimension.len(), 1);
        assert_eq!(array.dimension[0].lower, "0");
        assert_eq!(array.dimension[0].upper, "9");
        assert!(matches!(*array.base_type, DataType::Int));
    }
}
