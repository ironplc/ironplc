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
    pub base_type: DataType,

    #[serde(rename = "initialValue", default)]
    pub initial_value: Option<Value>,
}

/// A data type reference (can be elementary or derived)
#[derive(Debug, Deserialize)]
pub struct DataType {
    // Elementary types - each is an empty element
    #[serde(rename = "BOOL", default)]
    pub bool_type: Option<EmptyElement>,

    #[serde(rename = "BYTE", default)]
    pub byte_type: Option<EmptyElement>,

    #[serde(rename = "WORD", default)]
    pub word_type: Option<EmptyElement>,

    #[serde(rename = "DWORD", default)]
    pub dword_type: Option<EmptyElement>,

    #[serde(rename = "LWORD", default)]
    pub lword_type: Option<EmptyElement>,

    #[serde(rename = "SINT", default)]
    pub sint_type: Option<EmptyElement>,

    #[serde(rename = "INT", default)]
    pub int_type: Option<EmptyElement>,

    #[serde(rename = "DINT", default)]
    pub dint_type: Option<EmptyElement>,

    #[serde(rename = "LINT", default)]
    pub lint_type: Option<EmptyElement>,

    #[serde(rename = "USINT", default)]
    pub usint_type: Option<EmptyElement>,

    #[serde(rename = "UINT", default)]
    pub uint_type: Option<EmptyElement>,

    #[serde(rename = "UDINT", default)]
    pub udint_type: Option<EmptyElement>,

    #[serde(rename = "ULINT", default)]
    pub ulint_type: Option<EmptyElement>,

    #[serde(rename = "REAL", default)]
    pub real_type: Option<EmptyElement>,

    #[serde(rename = "LREAL", default)]
    pub lreal_type: Option<EmptyElement>,

    #[serde(rename = "TIME", default)]
    pub time_type: Option<EmptyElement>,

    #[serde(rename = "DATE", default)]
    pub date_type: Option<EmptyElement>,

    #[serde(rename = "DT", default)]
    pub dt_type: Option<EmptyElement>,

    #[serde(rename = "TOD", default)]
    pub tod_type: Option<EmptyElement>,

    #[serde(rename = "string", default)]
    pub string_type: Option<StringType>,

    #[serde(rename = "wstring", default)]
    pub wstring_type: Option<StringType>,

    // Derived types
    #[serde(rename = "derived", default)]
    pub derived: Option<DerivedType>,

    #[serde(rename = "array", default)]
    pub array: Option<ArrayType>,

    #[serde(rename = "enum", default)]
    pub enum_type: Option<EnumType>,

    #[serde(rename = "struct", default)]
    pub struct_type: Option<StructType>,

    #[serde(rename = "subrangeSigned", default)]
    pub subrange_signed: Option<SubrangeSigned>,

    #[serde(rename = "subrangeUnsigned", default)]
    pub subrange_unsigned: Option<SubrangeUnsigned>,

    #[serde(rename = "pointer", default)]
    pub pointer: Option<PointerType>,
}

/// Empty element marker (for types like BOOL, INT that have no content)
#[derive(Debug, Deserialize, Default)]
pub struct EmptyElement {}

/// String type with optional length
#[derive(Debug, Deserialize)]
pub struct StringType {
    #[serde(rename = "@length", default)]
    pub length: Option<String>,
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
    pub base_type: Box<DataType>,
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
    pub base_type: Option<Box<DataType>>,
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
    pub member_type: DataType,

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
    pub base_type: Box<DataType>,
}

/// Unsigned subrange type
#[derive(Debug, Deserialize)]
pub struct SubrangeUnsigned {
    #[serde(rename = "@lower")]
    pub lower: String,

    #[serde(rename = "@upper")]
    pub upper: String,

    #[serde(rename = "baseType")]
    pub base_type: Box<DataType>,
}

/// Pointer type
#[derive(Debug, Deserialize)]
pub struct PointerType {
    #[serde(rename = "baseType")]
    pub base_type: Box<DataType>,
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
    pub return_type: Option<DataType>,

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
    pub var_type: DataType,

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
        if self.bool_type.is_some() {
            "BOOL"
        } else if self.byte_type.is_some() {
            "BYTE"
        } else if self.word_type.is_some() {
            "WORD"
        } else if self.dword_type.is_some() {
            "DWORD"
        } else if self.lword_type.is_some() {
            "LWORD"
        } else if self.sint_type.is_some() {
            "SINT"
        } else if self.int_type.is_some() {
            "INT"
        } else if self.dint_type.is_some() {
            "DINT"
        } else if self.lint_type.is_some() {
            "LINT"
        } else if self.usint_type.is_some() {
            "USINT"
        } else if self.uint_type.is_some() {
            "UINT"
        } else if self.udint_type.is_some() {
            "UDINT"
        } else if self.ulint_type.is_some() {
            "ULINT"
        } else if self.real_type.is_some() {
            "REAL"
        } else if self.lreal_type.is_some() {
            "LREAL"
        } else if self.time_type.is_some() {
            "TIME"
        } else if self.date_type.is_some() {
            "DATE"
        } else if self.dt_type.is_some() {
            "DATE_AND_TIME"
        } else if self.tod_type.is_some() {
            "TIME_OF_DAY"
        } else if self.string_type.is_some() {
            "STRING"
        } else if self.wstring_type.is_some() {
            "WSTRING"
        } else if self.derived.is_some() {
            "derived"
        } else if self.array.is_some() {
            "ARRAY"
        } else if self.enum_type.is_some() {
            "enum"
        } else if self.struct_type.is_some() {
            "STRUCT"
        } else if self.subrange_signed.is_some() || self.subrange_unsigned.is_some() {
            "subrange"
        } else if self.pointer.is_some() {
            "POINTER"
        } else {
            "unknown"
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
        assert!(interface.input_vars[0].variable[0].var_type.bool_type.is_some());

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

        let enum_type = dt.base_type.enum_type.as_ref().unwrap();
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

        let array = dt.base_type.array.as_ref().unwrap();
        assert_eq!(array.dimension.len(), 1);
        assert_eq!(array.dimension[0].lower, "0");
        assert_eq!(array.dimension[0].upper, "9");
        assert!(array.base_type.int_type.is_some());
    }
}
