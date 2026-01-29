//! Transformation from PLCopen XML schema to IronPLC DSL
//!
//! This module transforms parsed PLCopen XML structures into
//! IronPLC's internal DSL representation.

use ironplc_dsl::{
    common::{
        ArrayDeclaration, ArraySpecificationKind, ArraySubranges, DataTypeDeclarationKind,
        DeclarationQualifier, ElementaryTypeName, EnumeratedSpecificationInit,
        EnumeratedSpecificationKind, EnumeratedSpecificationValues, EnumeratedValue,
        EnumerationDeclaration, FunctionBlockBodyKind, FunctionBlockDeclaration,
        FunctionDeclaration, InitialValueAssignmentKind, Integer, Library, LibraryElementKind,
        ProgramDeclaration, SignedInteger, SimpleDeclaration, SimpleInitializer,
        StructureDeclaration, StructureElementDeclaration, Subrange, SubrangeDeclaration,
        SubrangeSpecification, SubrangeSpecificationKind, TypeName, VarDecl, VariableIdentifier,
        VariableType,
    },
    core::{FileId, Id, SourceSpan},
    diagnostic::{Diagnostic, Label},
    textual::{Statements, StmtKind},
};
use ironplc_parser::options::ParseOptions;
use ironplc_problems::Problem;

use super::position::StBodyPositions;
use super::schema::{
    ArrayType, DataType, DataTypeDecl, Dimension, EnumType, Interface, Pou, PouType, Project,
    StructType, SubrangeSigned, SubrangeUnsigned, VarList, Variable,
};

/// Create a SourceSpan for the given file with no position info
fn file_span(file_id: &FileId) -> SourceSpan {
    SourceSpan::range(0, 0).with_file_id(file_id)
}

/// Transform a parsed PLCopen XML project into an IronPLC Library
///
/// This is the main entry point for XML -> DSL transformation.
///
/// The `st_positions` map provides line/column offsets for ST body content,
/// enabling accurate error positions when parsing embedded ST code.
pub fn transform_project(
    project: &Project,
    file_id: &FileId,
    st_positions: &StBodyPositions,
) -> Result<Library, Diagnostic> {
    let mut library = Library::new();

    // Transform data types
    for dt in &project.types.data_types.data_type {
        let decl = transform_data_type_decl(dt, file_id)?;
        library
            .elements
            .push(LibraryElementKind::DataTypeDeclaration(decl));
    }

    // Transform POUs
    for pou in &project.types.pous.pou {
        let elem = transform_pou(pou, file_id, st_positions)?;
        library.elements.push(elem);
    }

    Ok(library)
}

/// Transform a data type declaration
fn transform_data_type_decl(
    decl: &DataTypeDecl,
    file_id: &FileId,
) -> Result<DataTypeDeclarationKind, Diagnostic> {
    let type_name = TypeName::from(decl.name.as_str());

    match &*decl.base_type {
        DataType::Enum(enum_type) => transform_enum_decl(&type_name, enum_type),
        DataType::Array(array_type) => transform_array_decl(&type_name, array_type, file_id),
        DataType::Struct(struct_type) => transform_struct_decl(&type_name, struct_type, file_id),
        DataType::SubrangeSigned(subrange) => {
            transform_subrange_signed_decl(&type_name, subrange, file_id)
        }
        DataType::SubrangeUnsigned(subrange) => {
            transform_subrange_unsigned_decl(&type_name, subrange, file_id)
        }
        // Elementary types as type aliases
        _ => {
            let base_type_name = transform_data_type(&decl.base_type, file_id)?;
            Ok(DataTypeDeclarationKind::Simple(SimpleDeclaration {
                type_name,
                spec_and_init: InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: base_type_name,
                    initial_value: None,
                }),
            }))
        }
    }
}

/// Transform an enumeration declaration
fn transform_enum_decl(
    type_name: &TypeName,
    enum_type: &EnumType,
) -> Result<DataTypeDeclarationKind, Diagnostic> {
    let values: Vec<EnumeratedValue> = enum_type
        .values
        .value
        .iter()
        .map(|v| EnumeratedValue {
            type_name: Some(type_name.clone()),
            value: Id::from(v.name.as_str()),
        })
        .collect();

    let spec = EnumeratedSpecificationKind::Values(EnumeratedSpecificationValues { values });

    Ok(DataTypeDeclarationKind::Enumeration(
        EnumerationDeclaration {
            type_name: type_name.clone(),
            spec_init: EnumeratedSpecificationInit {
                spec,
                default: None,
            },
        },
    ))
}

/// Transform an array declaration
fn transform_array_decl(
    type_name: &TypeName,
    array_type: &ArrayType,
    file_id: &FileId,
) -> Result<DataTypeDeclarationKind, Diagnostic> {
    let base_type_name = transform_data_type(&array_type.base_type, file_id)?;
    let subranges = transform_dimensions(&array_type.dimension, file_id)?;

    Ok(DataTypeDeclarationKind::Array(ArrayDeclaration {
        type_name: type_name.clone(),
        spec: ArraySpecificationKind::Subranges(ArraySubranges {
            ranges: subranges,
            type_name: base_type_name,
        }),
        init: vec![],
    }))
}

/// Transform array dimensions to subranges
fn transform_dimensions(
    dimensions: &[Dimension],
    file_id: &FileId,
) -> Result<Vec<Subrange>, Diagnostic> {
    dimensions
        .iter()
        .map(|dim| {
            let span = file_span(file_id);
            let lower = dim
                .lower
                .parse::<i128>()
                .map_err(|_| invalid_value_error(&dim.lower, "array lower bound", file_id))?;
            let upper = dim
                .upper
                .parse::<i128>()
                .map_err(|_| invalid_value_error(&dim.upper, "array upper bound", file_id))?;

            Ok(Subrange {
                start: SignedInteger {
                    value: Integer {
                        span: span.clone(),
                        value: lower.unsigned_abs(),
                    },
                    is_neg: lower < 0,
                },
                end: SignedInteger {
                    value: Integer {
                        span,
                        value: upper.unsigned_abs(),
                    },
                    is_neg: upper < 0,
                },
            })
        })
        .collect()
}

/// Transform a structure declaration
fn transform_struct_decl(
    type_name: &TypeName,
    struct_type: &StructType,
    file_id: &FileId,
) -> Result<DataTypeDeclarationKind, Diagnostic> {
    let elements: Result<Vec<_>, _> = struct_type
        .variable
        .iter()
        .map(|member| {
            let member_type = transform_data_type(&member.member_type, file_id)?;
            Ok(StructureElementDeclaration {
                name: Id::from(member.name.as_str()),
                init: InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: member_type,
                    initial_value: None,
                }),
            })
        })
        .collect();

    Ok(DataTypeDeclarationKind::Structure(StructureDeclaration {
        type_name: type_name.clone(),
        elements: elements?,
    }))
}

/// Transform a signed subrange declaration
fn transform_subrange_signed_decl(
    type_name: &TypeName,
    subrange: &SubrangeSigned,
    file_id: &FileId,
) -> Result<DataTypeDeclarationKind, Diagnostic> {
    let base_type = transform_base_type_to_elementary(&subrange.base_type, file_id)?;
    let span = file_span(file_id);

    let lower = subrange
        .lower
        .parse::<i128>()
        .map_err(|_| invalid_value_error(&subrange.lower, "subrange lower bound", file_id))?;
    let upper = subrange
        .upper
        .parse::<i128>()
        .map_err(|_| invalid_value_error(&subrange.upper, "subrange upper bound", file_id))?;

    Ok(DataTypeDeclarationKind::Subrange(SubrangeDeclaration {
        type_name: type_name.clone(),
        spec: SubrangeSpecificationKind::Specification(SubrangeSpecification {
            type_name: base_type,
            subrange: Subrange {
                start: SignedInteger {
                    value: Integer {
                        span: span.clone(),
                        value: lower.unsigned_abs(),
                    },
                    is_neg: lower < 0,
                },
                end: SignedInteger {
                    value: Integer {
                        span,
                        value: upper.unsigned_abs(),
                    },
                    is_neg: upper < 0,
                },
            },
        }),
        default: None,
    }))
}

/// Transform an unsigned subrange declaration
fn transform_subrange_unsigned_decl(
    type_name: &TypeName,
    subrange: &SubrangeUnsigned,
    file_id: &FileId,
) -> Result<DataTypeDeclarationKind, Diagnostic> {
    let base_type = transform_base_type_to_elementary(&subrange.base_type, file_id)?;
    let span = file_span(file_id);

    let lower = subrange
        .lower
        .parse::<u128>()
        .map_err(|_| invalid_value_error(&subrange.lower, "subrange lower bound", file_id))?;
    let upper = subrange
        .upper
        .parse::<u128>()
        .map_err(|_| invalid_value_error(&subrange.upper, "subrange upper bound", file_id))?;

    Ok(DataTypeDeclarationKind::Subrange(SubrangeDeclaration {
        type_name: type_name.clone(),
        spec: SubrangeSpecificationKind::Specification(SubrangeSpecification {
            type_name: base_type,
            subrange: Subrange {
                start: SignedInteger {
                    value: Integer {
                        span: span.clone(),
                        value: lower,
                    },
                    is_neg: false,
                },
                end: SignedInteger {
                    value: Integer { span, value: upper },
                    is_neg: false,
                },
            },
        }),
        default: None,
    }))
}

/// Transform a DataType to an ElementaryTypeName for subranges
fn transform_base_type_to_elementary(
    type_element: &super::schema::TypeElement,
    file_id: &FileId,
) -> Result<ElementaryTypeName, Diagnostic> {
    let span = file_span(file_id);
    match &type_element.inner {
        DataType::SInt => Ok(ElementaryTypeName::SINT),
        DataType::Int => Ok(ElementaryTypeName::INT),
        DataType::DInt => Ok(ElementaryTypeName::DINT),
        DataType::LInt => Ok(ElementaryTypeName::LINT),
        DataType::USInt => Ok(ElementaryTypeName::USINT),
        DataType::UInt => Ok(ElementaryTypeName::UINT),
        DataType::UDInt => Ok(ElementaryTypeName::UDINT),
        DataType::ULInt => Ok(ElementaryTypeName::ULINT),
        _ => Err(Diagnostic::problem(
            Problem::SyntaxError,
            Label::span(
                span,
                format!(
                    "Subrange base type must be an integer type, found: {}",
                    type_element.inner.type_name()
                ),
            ),
        )),
    }
}

/// Transform a PLCopen DataType to a DSL TypeName
fn transform_data_type(
    type_element: &super::schema::TypeElement,
    _file_id: &FileId,
) -> Result<TypeName, Diagnostic> {
    match &type_element.inner {
        // Elementary types
        DataType::Bool => Ok(ElementaryTypeName::BOOL.into()),
        DataType::Byte => Ok(ElementaryTypeName::BYTE.into()),
        DataType::Word => Ok(ElementaryTypeName::WORD.into()),
        DataType::DWord => Ok(ElementaryTypeName::DWORD.into()),
        DataType::LWord => Ok(ElementaryTypeName::LWORD.into()),
        DataType::SInt => Ok(ElementaryTypeName::SINT.into()),
        DataType::Int => Ok(ElementaryTypeName::INT.into()),
        DataType::DInt => Ok(ElementaryTypeName::DINT.into()),
        DataType::LInt => Ok(ElementaryTypeName::LINT.into()),
        DataType::USInt => Ok(ElementaryTypeName::USINT.into()),
        DataType::UInt => Ok(ElementaryTypeName::UINT.into()),
        DataType::UDInt => Ok(ElementaryTypeName::UDINT.into()),
        DataType::ULInt => Ok(ElementaryTypeName::ULINT.into()),
        DataType::Real => Ok(ElementaryTypeName::REAL.into()),
        DataType::LReal => Ok(ElementaryTypeName::LREAL.into()),
        DataType::Time => Ok(ElementaryTypeName::TIME.into()),
        DataType::Date => Ok(ElementaryTypeName::DATE.into()),
        DataType::DateAndTime => Ok(ElementaryTypeName::DateAndTime.into()),
        DataType::TimeOfDay => Ok(ElementaryTypeName::TimeOfDay.into()),
        DataType::String { .. } => Ok(ElementaryTypeName::STRING.into()),
        DataType::WString { .. } => Ok(ElementaryTypeName::WSTRING.into()),

        // Derived type (reference to another type)
        DataType::Derived(derived) => Ok(TypeName::from(derived.name.as_str())),

        // Complex types that need context
        DataType::Array(_) => Err(Diagnostic::todo(file!(), line!())),
        DataType::Enum(_) => Err(Diagnostic::todo(file!(), line!())),
        DataType::Struct(_) => Err(Diagnostic::todo(file!(), line!())),

        // Generic types (usually for library functions)
        DataType::Any
        | DataType::AnyDerived
        | DataType::AnyElementary
        | DataType::AnyMagnitude
        | DataType::AnyNum
        | DataType::AnyReal
        | DataType::AnyInt
        | DataType::AnyBit
        | DataType::AnyString
        | DataType::AnyDate => Err(Diagnostic::todo(file!(), line!())),

        // Subranges and pointers
        DataType::SubrangeSigned(_) | DataType::SubrangeUnsigned(_) => {
            Err(Diagnostic::todo(file!(), line!()))
        }
        DataType::Pointer(_) => Err(Diagnostic::todo(file!(), line!())),
    }
}

/// Transform a POU (Program Organization Unit)
fn transform_pou(
    pou: &Pou,
    file_id: &FileId,
    st_positions: &StBodyPositions,
) -> Result<LibraryElementKind, Diagnostic> {
    match pou.pou_type {
        PouType::Function => transform_function(pou, file_id, st_positions),
        PouType::FunctionBlock => transform_function_block(pou, file_id, st_positions),
        PouType::Program => transform_program(pou, file_id, st_positions),
    }
}

/// Transform a function declaration
fn transform_function(
    pou: &Pou,
    file_id: &FileId,
    st_positions: &StBodyPositions,
) -> Result<LibraryElementKind, Diagnostic> {
    let name = Id::from(pou.name.as_str());
    let span = file_span(file_id);

    // Get return type (required for functions)
    let return_type = if let Some(ref interface) = pou.interface {
        if let Some(ref rt) = interface.return_type {
            transform_data_type(rt, file_id)?
        } else {
            // Functions must have a return type
            return Err(Diagnostic::problem(
                Problem::SyntaxError,
                Label::span(
                    span,
                    format!("Function '{}' is missing a return type", pou.name),
                ),
            ));
        }
    } else {
        return Err(Diagnostic::problem(
            Problem::SyntaxError,
            Label::span(
                span,
                format!("Function '{}' is missing interface", pou.name),
            ),
        ));
    };

    let variables = transform_interface(pou.interface.as_ref(), file_id)?;
    let body = transform_body_statements(pou, file_id, st_positions)?;

    Ok(LibraryElementKind::FunctionDeclaration(
        FunctionDeclaration {
            name,
            return_type,
            variables,
            edge_variables: vec![],
            body,
        },
    ))
}

/// Transform a function block declaration
fn transform_function_block(
    pou: &Pou,
    file_id: &FileId,
    st_positions: &StBodyPositions,
) -> Result<LibraryElementKind, Diagnostic> {
    let name = TypeName::from(pou.name.as_str());
    let span = file_span(file_id);

    let variables = transform_interface(pou.interface.as_ref(), file_id)?;
    let body = transform_body(pou, file_id, st_positions)?;

    Ok(LibraryElementKind::FunctionBlockDeclaration(
        FunctionBlockDeclaration {
            name,
            variables,
            edge_variables: vec![],
            body,
            span,
        },
    ))
}

/// Transform a program declaration
fn transform_program(
    pou: &Pou,
    file_id: &FileId,
    st_positions: &StBodyPositions,
) -> Result<LibraryElementKind, Diagnostic> {
    let name = Id::from(pou.name.as_str());

    let variables = transform_interface(pou.interface.as_ref(), file_id)?;
    let body = transform_body(pou, file_id, st_positions)?;

    Ok(LibraryElementKind::ProgramDeclaration(ProgramDeclaration {
        name,
        variables,
        access_variables: vec![],
        body,
    }))
}

/// Transform a POU interface (variables)
fn transform_interface(
    interface: Option<&Interface>,
    file_id: &FileId,
) -> Result<Vec<VarDecl>, Diagnostic> {
    let Some(interface) = interface else {
        return Ok(vec![]);
    };

    let mut vars = vec![];

    // Input variables
    for var_list in &interface.input_vars {
        vars.extend(transform_var_list(var_list, VariableType::Input, file_id)?);
    }

    // Output variables
    for var_list in &interface.output_vars {
        vars.extend(transform_var_list(var_list, VariableType::Output, file_id)?);
    }

    // In/Out variables
    for var_list in &interface.in_out_vars {
        vars.extend(transform_var_list(var_list, VariableType::InOut, file_id)?);
    }

    // Local variables
    for var_list in &interface.local_vars {
        vars.extend(transform_var_list(var_list, VariableType::Var, file_id)?);
    }

    // Temp variables
    for var_list in &interface.temp_vars {
        vars.extend(transform_var_list(
            var_list,
            VariableType::VarTemp,
            file_id,
        )?);
    }

    // External variables
    for var_list in &interface.external_vars {
        vars.extend(transform_var_list(
            var_list,
            VariableType::External,
            file_id,
        )?);
    }

    // Global variables (at POU level)
    for var_list in &interface.global_vars {
        vars.extend(transform_var_list(var_list, VariableType::Global, file_id)?);
    }

    Ok(vars)
}

/// Transform a variable list
fn transform_var_list(
    var_list: &VarList,
    var_type: VariableType,
    file_id: &FileId,
) -> Result<Vec<VarDecl>, Diagnostic> {
    let qualifier = if var_list.constant {
        DeclarationQualifier::Constant
    } else if var_list.retain {
        DeclarationQualifier::Retain
    } else if var_list.nonretain {
        DeclarationQualifier::NonRetain
    } else {
        DeclarationQualifier::Unspecified
    };

    var_list
        .variable
        .iter()
        .map(|v| transform_variable(v, var_type.clone(), qualifier.clone(), file_id))
        .collect()
}

/// Transform a variable declaration
fn transform_variable(
    var: &Variable,
    var_type: VariableType,
    qualifier: DeclarationQualifier,
    file_id: &FileId,
) -> Result<VarDecl, Diagnostic> {
    let identifier = VariableIdentifier::Symbol(Id::from(var.name.as_str()));
    let type_name = transform_data_type(&var.var_type, file_id)?;

    let initializer = InitialValueAssignmentKind::Simple(SimpleInitializer {
        type_name,
        initial_value: None, // TODO: Handle initial values
    });

    Ok(VarDecl {
        identifier,
        var_type,
        qualifier,
        initializer,
    })
}

/// Transform POU body to FunctionBlockBodyKind
fn transform_body(
    pou: &Pou,
    file_id: &FileId,
    st_positions: &StBodyPositions,
) -> Result<FunctionBlockBodyKind, Diagnostic> {
    let Some(ref body) = pou.body else {
        return Ok(FunctionBlockBodyKind::Empty);
    };

    if let Some(st_text) = body.st_text() {
        let stmts = parse_st_body(st_text, file_id, &pou.name, st_positions)?;
        Ok(FunctionBlockBodyKind::Statements(Statements {
            body: stmts,
        }))
    } else if body.sfc.is_some() {
        // SFC support is planned for Phase 3
        Err(Diagnostic::todo(file!(), line!()))
    } else {
        Ok(FunctionBlockBodyKind::Empty)
    }
}

/// Transform POU body to Vec<StmtKind> (for functions)
fn transform_body_statements(
    pou: &Pou,
    file_id: &FileId,
    st_positions: &StBodyPositions,
) -> Result<Vec<StmtKind>, Diagnostic> {
    let Some(ref body) = pou.body else {
        return Ok(vec![]);
    };

    if let Some(st_text) = body.st_text() {
        parse_st_body(st_text, file_id, &pou.name, st_positions)
    } else {
        Ok(vec![])
    }
}

/// Parse ST body text using the ST parser
///
/// Uses the position information from roxmltree to provide accurate
/// line/column offsets for error reporting.
fn parse_st_body(
    st_text: &str,
    file_id: &FileId,
    pou_name: &str,
    st_positions: &StBodyPositions,
) -> Result<Vec<StmtKind>, Diagnostic> {
    let options = ParseOptions::default();

    // Look up the position for this POU's ST body
    let (line_offset, col_offset) = st_positions
        .get(pou_name)
        .map(|pos| (pos.line, pos.col))
        .unwrap_or((0, 0));

    ironplc_parser::parse_st_statements(st_text, file_id, &options, line_offset, col_offset)
}

/// Create an error diagnostic for invalid values
fn invalid_value_error(value: &str, context: &str, file_id: &FileId) -> Diagnostic {
    Diagnostic::problem(
        Problem::SyntaxError,
        Label::span(
            file_span(file_id),
            format!("Invalid {}: '{}'", context, value),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::position::find_st_body_positions;

    fn test_file_id() -> FileId {
        FileId::from_string("test.xml")
    }

    fn parse_project(xml: &str) -> Project {
        quick_xml::de::from_str(xml).unwrap()
    }

    fn get_st_positions(xml: &str) -> StBodyPositions {
        find_st_body_positions(xml).unwrap_or_default()
    }

    fn minimal_project_header() -> &'static str {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="Test" productName="Test" productVersion="1.0" creationDateTime="2024-01-01T00:00:00"/>
  <contentHeader name="TestProject">
    <coordinateInfo>
      <fbd><scaling x="1" y="1"/></fbd>
      <ld><scaling x="1" y="1"/></ld>
      <sfc><scaling x="1" y="1"/></sfc>
    </coordinateInfo>
  </contentHeader>"#
    }

    #[test]
    fn transform_when_empty_project_then_returns_empty_library() {
        let xml = format!(
            "{}\n  <types>\n    <dataTypes/>\n    <pous/>\n  </types>\n</project>",
            minimal_project_header()
        );
        let project = parse_project(&xml);
        let positions = get_st_positions(&xml);
        let library = transform_project(&project, &test_file_id(), &positions).unwrap();

        assert_eq!(library.elements.len(), 0);
    }

    #[test]
    fn transform_when_enum_type_then_creates_enumeration_declaration() {
        let xml = format!(
            r#"{}
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
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let positions = get_st_positions(&xml);
        let library = transform_project(&project, &test_file_id(), &positions).unwrap();

        assert_eq!(library.elements.len(), 1);
        let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
            enum_decl,
        )) = &library.elements[0]
        else {
            panic!("Expected enumeration declaration");
        };
        assert_eq!(enum_decl.type_name.to_string(), "TrafficLight");
    }

    #[test]
    fn transform_when_array_type_then_creates_array_declaration() {
        let xml = format!(
            r#"{}
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
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let positions = get_st_positions(&xml);
        let library = transform_project(&project, &test_file_id(), &positions).unwrap();

        assert_eq!(library.elements.len(), 1);
        let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Array(array_decl)) =
            &library.elements[0]
        else {
            panic!("Expected array declaration");
        };
        assert_eq!(array_decl.type_name.to_string(), "IntArray");
    }

    #[test]
    fn transform_when_struct_type_then_creates_structure_declaration() {
        let xml = format!(
            r#"{}
  <types>
    <dataTypes>
      <dataType name="Point">
        <baseType>
          <struct>
            <variable name="X">
              <type><REAL/></type>
            </variable>
            <variable name="Y">
              <type><REAL/></type>
            </variable>
          </struct>
        </baseType>
      </dataType>
    </dataTypes>
    <pous/>
  </types>
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let positions = get_st_positions(&xml);
        let library = transform_project(&project, &test_file_id(), &positions).unwrap();

        assert_eq!(library.elements.len(), 1);
        let LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(
            struct_decl,
        )) = &library.elements[0]
        else {
            panic!("Expected structure declaration");
        };
        assert_eq!(struct_decl.type_name.to_string(), "Point");
        assert_eq!(struct_decl.elements.len(), 2);
    }

    #[test]
    fn transform_when_function_block_then_creates_declaration() {
        let xml = format!(
            r#"{}
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
IF Reset THEN Count := 0; END_IF;
            </xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let positions = get_st_positions(&xml);
        let library = transform_project(&project, &test_file_id(), &positions).unwrap();

        assert_eq!(library.elements.len(), 1);
        let LibraryElementKind::FunctionBlockDeclaration(fb_decl) = &library.elements[0] else {
            panic!("Expected function block declaration");
        };
        assert_eq!(fb_decl.name.to_string(), "Counter");
        assert_eq!(fb_decl.variables.len(), 2);
    }

    #[test]
    fn transform_when_program_then_creates_declaration() {
        let xml = format!(
            r#"{}
  <types>
    <dataTypes/>
    <pous>
      <pou name="Main" pouType="program">
        <interface>
          <localVars>
            <variable name="x">
              <type><INT/></type>
            </variable>
          </localVars>
        </interface>
      </pou>
    </pous>
  </types>
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let positions = get_st_positions(&xml);
        let library = transform_project(&project, &test_file_id(), &positions).unwrap();

        assert_eq!(library.elements.len(), 1);
        let LibraryElementKind::ProgramDeclaration(prog_decl) = &library.elements[0] else {
            panic!("Expected program declaration");
        };
        assert_eq!(prog_decl.name.to_string(), "Main");
        assert_eq!(prog_decl.variables.len(), 1);
    }

    #[test]
    fn transform_when_function_block_with_st_body_then_parses_statements() {
        let xml = format!(
            r#"{}
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
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let positions = get_st_positions(&xml);
        let library = transform_project(&project, &test_file_id(), &positions).unwrap();

        assert_eq!(library.elements.len(), 1);
        let LibraryElementKind::FunctionBlockDeclaration(fb_decl) = &library.elements[0] else {
            panic!("Expected function block declaration");
        };

        // Verify the body has statements
        let FunctionBlockBodyKind::Statements(stmts) = &fb_decl.body else {
            panic!("Expected statements body");
        };
        assert!(!stmts.body.is_empty(), "Expected parsed statements");
    }
}
