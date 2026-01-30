//! PLCopen XML parser using roxmltree
//!
//! This module parses PLCopen TC6 XML documents into schema structs using roxmltree.
//! It extracts both data and position information in a single pass.

use ironplc_dsl::{
    core::FileId,
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;

use super::schema::{
    Action, Actions, ArrayType, Body, Configuration, Configurations, ContentHeader, CoordinateInfo,
    DataType, DataTypeDecl, DataTypes, DerivedType, Dimension, EnumType, EnumValue, EnumValues,
    FileHeader, Instances, Interface, PointerType, Pou, PouInstance, PouType, Pous, Project,
    Resource, Scaling, ScalingValue, StBody, StructMember, StructType, SubrangeSigned,
    SubrangeUnsigned, Task, Transition, Transitions, Types, Value, VarList, Variable,
};

/// Parse a PLCopen XML document into a Project struct
///
/// This function parses the XML in a single pass, extracting both the data
/// and position information for ST body content.
///
/// Returns a `Diagnostic` error with file location information if parsing fails.
pub fn parse_plcopen_xml(xml_content: &str, file_id: &FileId) -> Result<Project, Diagnostic> {
    let doc = roxmltree::Document::parse(xml_content).map_err(|e| {
        Diagnostic::problem(
            Problem::SyntaxError,
            Label::file(file_id.clone(), format!("XML parse error: {}", e)),
        )
    })?;

    let root = doc.root_element();
    if !root.has_tag_name("project") {
        return Err(Diagnostic::problem(
            Problem::SyntaxError,
            Label::file(
                file_id.clone(),
                format!(
                    "Expected root element 'project', found '{}'",
                    root.tag_name().name()
                ),
            ),
        ));
    }

    parse_project(&doc, root).map_err(|e| {
        Diagnostic::problem(Problem::SyntaxError, Label::file(file_id.clone(), e))
    })
}

fn parse_project(doc: &roxmltree::Document, node: roxmltree::Node) -> Result<Project, String> {
    let mut project = Project::default();

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "fileHeader" => project.file_header = parse_file_header(child)?,
            "contentHeader" => project.content_header = parse_content_header(child)?,
            "types" => project.types = parse_types(doc, child)?,
            "instances" => project.instances = Some(parse_instances(child)?),
            _ => {} // Ignore unknown elements
        }
    }

    Ok(project)
}

fn parse_file_header(node: roxmltree::Node) -> Result<FileHeader, String> {
    Ok(FileHeader {
        company_name: node.attribute("companyName").unwrap_or("").to_string(),
        company_url: node.attribute("companyURL").map(String::from),
        product_name: node.attribute("productName").unwrap_or("").to_string(),
        product_version: node.attribute("productVersion").unwrap_or("").to_string(),
        product_release: node.attribute("productRelease").map(String::from),
        creation_date_time: node.attribute("creationDateTime").unwrap_or("").to_string(),
        content_description: node.attribute("contentDescription").map(String::from),
    })
}

fn parse_content_header(node: roxmltree::Node) -> Result<ContentHeader, String> {
    let mut header = ContentHeader {
        name: node.attribute("name").unwrap_or("").to_string(),
        version: node.attribute("version").map(String::from),
        modification_date_time: node.attribute("modificationDateTime").map(String::from),
        organization: node.attribute("organization").map(String::from),
        author: node.attribute("author").map(String::from),
        language: node.attribute("language").map(String::from),
        ..Default::default()
    };

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "Comment" => header.comment = get_text_content(child),
            "coordinateInfo" => header.coordinate_info = parse_coordinate_info(child)?,
            _ => {}
        }
    }

    Ok(header)
}

fn parse_coordinate_info(node: roxmltree::Node) -> Result<CoordinateInfo, String> {
    let mut info = CoordinateInfo::default();

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "fbd" => info.fbd = parse_scaling(child)?,
            "ld" => info.ld = parse_scaling(child)?,
            "sfc" => info.sfc = parse_scaling(child)?,
            _ => {}
        }
    }

    Ok(info)
}

fn parse_scaling(node: roxmltree::Node) -> Result<Scaling, String> {
    let mut scaling = Scaling::default();

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "scaling" {
            scaling.scaling = ScalingValue {
                x: child.attribute("x").unwrap_or("1").to_string(),
                y: child.attribute("y").unwrap_or("1").to_string(),
            };
        }
    }

    Ok(scaling)
}

fn parse_types(doc: &roxmltree::Document, node: roxmltree::Node) -> Result<Types, String> {
    let mut types = Types::default();

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "dataTypes" => types.data_types = parse_data_types(child)?,
            "pous" => types.pous = parse_pous(doc, child)?,
            _ => {}
        }
    }

    Ok(types)
}

fn parse_data_types(node: roxmltree::Node) -> Result<DataTypes, String> {
    let mut data_types = DataTypes::default();

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "dataType" {
            data_types.data_type.push(parse_data_type_decl(child)?);
        }
    }

    Ok(data_types)
}

fn parse_data_type_decl(node: roxmltree::Node) -> Result<DataTypeDecl, String> {
    let name = node.attribute("name").unwrap_or("").to_string();
    let mut base_type = DataType::Bool;
    let mut initial_value = None;

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "baseType" => base_type = parse_type_element(child)?,
            "initialValue" => initial_value = Some(parse_value(child)?),
            _ => {}
        }
    }

    Ok(DataTypeDecl {
        name,
        base_type,
        initial_value,
    })
}

fn parse_type_element(node: roxmltree::Node) -> Result<DataType, String> {
    // The type element contains exactly one child that indicates the type
    if let Some(child) = node.children().find(|n| n.is_element()) {
        return parse_data_type_node(child);
    }
    Ok(DataType::Bool) // Default if empty
}

fn parse_data_type_node(node: roxmltree::Node) -> Result<DataType, String> {
    let tag = node.tag_name().name();
    match tag {
        // Elementary types
        "BOOL" => Ok(DataType::Bool),
        "BYTE" => Ok(DataType::Byte),
        "WORD" => Ok(DataType::Word),
        "DWORD" => Ok(DataType::DWord),
        "LWORD" => Ok(DataType::LWord),
        "SINT" => Ok(DataType::SInt),
        "INT" => Ok(DataType::Int),
        "DINT" => Ok(DataType::DInt),
        "LINT" => Ok(DataType::LInt),
        "USINT" => Ok(DataType::USInt),
        "UINT" => Ok(DataType::UInt),
        "UDINT" => Ok(DataType::UDInt),
        "ULINT" => Ok(DataType::ULInt),
        "REAL" => Ok(DataType::Real),
        "LREAL" => Ok(DataType::LReal),
        "TIME" => Ok(DataType::Time),
        "DATE" => Ok(DataType::Date),
        "DT" => Ok(DataType::DateAndTime),
        "TOD" => Ok(DataType::TimeOfDay),

        // String types
        "string" => Ok(DataType::String {
            length: node.attribute("length").map(String::from),
        }),
        "wstring" => Ok(DataType::WString {
            length: node.attribute("length").map(String::from),
        }),

        // Generic types
        "ANY" => Ok(DataType::Any),
        "ANY_DERIVED" => Ok(DataType::AnyDerived),
        "ANY_ELEMENTARY" => Ok(DataType::AnyElementary),
        "ANY_MAGNITUDE" => Ok(DataType::AnyMagnitude),
        "ANY_NUM" => Ok(DataType::AnyNum),
        "ANY_REAL" => Ok(DataType::AnyReal),
        "ANY_INT" => Ok(DataType::AnyInt),
        "ANY_BIT" => Ok(DataType::AnyBit),
        "ANY_STRING" => Ok(DataType::AnyString),
        "ANY_DATE" => Ok(DataType::AnyDate),

        // Derived type reference
        "derived" => Ok(DataType::Derived(DerivedType {
            name: node.attribute("name").unwrap_or("").to_string(),
        })),

        // Complex types
        "array" => Ok(DataType::Array(Box::new(parse_array_type(node)?))),
        "enum" => Ok(DataType::Enum(parse_enum_type(node)?)),
        "struct" => Ok(DataType::Struct(parse_struct_type(node)?)),
        "subrangeSigned" => Ok(DataType::SubrangeSigned(Box::new(parse_subrange_signed(
            node,
        )?))),
        "subrangeUnsigned" => Ok(DataType::SubrangeUnsigned(Box::new(
            parse_subrange_unsigned(node)?,
        ))),
        "pointer" => Ok(DataType::Pointer(Box::new(parse_pointer_type(node)?))),

        _ => Err(format!("Unknown data type: {}", tag)),
    }
}

fn parse_array_type(node: roxmltree::Node) -> Result<ArrayType, String> {
    let mut dimensions = Vec::new();
    let mut base_type = DataType::Bool;

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "dimension" => dimensions.push(Dimension {
                lower: child.attribute("lower").unwrap_or("0").to_string(),
                upper: child.attribute("upper").unwrap_or("0").to_string(),
            }),
            "baseType" => base_type = parse_type_element(child)?,
            _ => {}
        }
    }

    Ok(ArrayType {
        dimension: dimensions,
        base_type,
    })
}

fn parse_enum_type(node: roxmltree::Node) -> Result<EnumType, String> {
    let mut values = EnumValues { value: Vec::new() };
    let mut base_type = None;

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "values" => {
                for value_node in child.children().filter(|n| n.is_element()) {
                    if value_node.tag_name().name() == "value" {
                        values.value.push(EnumValue {
                            name: value_node.attribute("name").unwrap_or("").to_string(),
                            value: value_node.attribute("value").map(String::from),
                        });
                    }
                }
            }
            "baseType" => base_type = Some(Box::new(parse_type_element(child)?)),
            _ => {}
        }
    }

    Ok(EnumType { values, base_type })
}

fn parse_struct_type(node: roxmltree::Node) -> Result<StructType, String> {
    let mut members = Vec::new();

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "variable" {
            members.push(parse_struct_member(child)?);
        }
    }

    Ok(StructType { variable: members })
}

fn parse_struct_member(node: roxmltree::Node) -> Result<StructMember, String> {
    let name = node.attribute("name").unwrap_or("").to_string();
    let mut member_type = DataType::Bool;
    let mut initial_value = None;

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "type" => member_type = parse_type_element(child)?,
            "initialValue" => initial_value = Some(parse_value(child)?),
            _ => {}
        }
    }

    Ok(StructMember {
        name,
        member_type,
        initial_value,
    })
}

fn parse_subrange_signed(node: roxmltree::Node) -> Result<SubrangeSigned, String> {
    let lower = node.attribute("lower").unwrap_or("0").to_string();
    let upper = node.attribute("upper").unwrap_or("0").to_string();
    let mut base_type = DataType::Int;

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "baseType" {
            base_type = parse_type_element(child)?;
        }
    }

    Ok(SubrangeSigned {
        lower,
        upper,
        base_type,
    })
}

fn parse_subrange_unsigned(node: roxmltree::Node) -> Result<SubrangeUnsigned, String> {
    let lower = node.attribute("lower").unwrap_or("0").to_string();
    let upper = node.attribute("upper").unwrap_or("0").to_string();
    let mut base_type = DataType::UInt;

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "baseType" {
            base_type = parse_type_element(child)?;
        }
    }

    Ok(SubrangeUnsigned {
        lower,
        upper,
        base_type,
    })
}

fn parse_pointer_type(node: roxmltree::Node) -> Result<PointerType, String> {
    let mut base_type = DataType::Bool;

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "baseType" {
            base_type = parse_type_element(child)?;
        }
    }

    Ok(PointerType { base_type })
}

fn parse_pous(doc: &roxmltree::Document, node: roxmltree::Node) -> Result<Pous, String> {
    let mut pous = Pous::default();

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "pou" {
            pous.pou.push(parse_pou(doc, child)?);
        }
    }

    Ok(pous)
}

fn parse_pou(doc: &roxmltree::Document, node: roxmltree::Node) -> Result<Pou, String> {
    let name = node.attribute("name").unwrap_or("").to_string();
    let pou_type = match node.attribute("pouType") {
        Some("function") => PouType::Function,
        Some("functionBlock") => PouType::FunctionBlock,
        Some("program") => PouType::Program,
        _ => PouType::Program,
    };
    let global_id = node.attribute("globalId").map(String::from);

    let mut interface = None;
    let mut body = None;
    let mut actions = None;
    let mut transitions = None;

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "interface" => interface = Some(parse_interface(child)?),
            "body" => body = Some(parse_body(doc, child)?),
            "actions" => actions = Some(parse_actions(doc, child)?),
            "transitions" => transitions = Some(parse_transitions(doc, child)?),
            _ => {}
        }
    }

    Ok(Pou {
        name,
        pou_type,
        global_id,
        interface,
        body,
        actions,
        transitions,
    })
}

fn parse_interface(node: roxmltree::Node) -> Result<Interface, String> {
    let mut interface = Interface::default();

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "returnType" => interface.return_type = Some(parse_type_element(child)?),
            "localVars" => interface.local_vars.push(parse_var_list(child)?),
            "tempVars" => interface.temp_vars.push(parse_var_list(child)?),
            "inputVars" => interface.input_vars.push(parse_var_list(child)?),
            "outputVars" => interface.output_vars.push(parse_var_list(child)?),
            "inOutVars" => interface.in_out_vars.push(parse_var_list(child)?),
            "externalVars" => interface.external_vars.push(parse_var_list(child)?),
            "globalVars" => interface.global_vars.push(parse_var_list(child)?),
            _ => {}
        }
    }

    Ok(interface)
}

fn parse_var_list(node: roxmltree::Node) -> Result<VarList, String> {
    let mut var_list = VarList {
        name: node.attribute("name").map(String::from),
        constant: node.attribute("constant") == Some("true"),
        retain: node.attribute("retain") == Some("true"),
        nonretain: node.attribute("nonretain") == Some("true"),
        persistent: node.attribute("persistent") == Some("true"),
        variable: Vec::new(),
    };

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "variable" {
            var_list.variable.push(parse_variable(child)?);
        }
    }

    Ok(var_list)
}

fn parse_variable(node: roxmltree::Node) -> Result<Variable, String> {
    let name = node.attribute("name").unwrap_or("").to_string();
    let address = node.attribute("address").map(String::from);
    let global_id = node.attribute("globalId").map(String::from);
    let mut var_type = DataType::Bool;
    let mut initial_value = None;

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "type" => var_type = parse_type_element(child)?,
            "initialValue" => initial_value = Some(parse_value(child)?),
            _ => {}
        }
    }

    Ok(Variable {
        name,
        address,
        global_id,
        var_type,
        initial_value,
    })
}

fn parse_body(doc: &roxmltree::Document, node: roxmltree::Node) -> Result<Body, String> {
    let mut body = Body {
        worksheet_name: node.attribute("WorksheetName").map(String::from),
        global_id: node.attribute("globalId").map(String::from),
        ..Default::default()
    };

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "ST" => body.st = Some(parse_st_body(doc, child)?),
            "IL" => body.il = get_text_content(child),
            "FBD" => body.fbd = true,
            "LD" => body.ld = true,
            "SFC" => body.sfc = true,
            _ => {}
        }
    }

    Ok(body)
}

fn parse_st_body(doc: &roxmltree::Document, node: roxmltree::Node) -> Result<StBody, String> {
    // Look for xhtml element first, then fall back to direct text content
    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "xhtml" {
            // Get text from xhtml element
            if let Some(text_node) = child.children().find(|n| n.is_text()) {
                let text = text_node.text().unwrap_or("").to_string();
                let byte_pos = text_node.range().start;
                let text_pos = doc.text_pos_at(byte_pos);

                // Pass raw text with position - the ST parser handles leading whitespace
                return Ok(StBody {
                    text,
                    // roxmltree uses 1-based positions, convert to 0-based
                    line_offset: text_pos.row.saturating_sub(1) as usize,
                    col_offset: text_pos.col.saturating_sub(1) as usize,
                });
            }
        }
    }

    // Fall back to direct text content in ST element
    if let Some(text_node) = node.children().find(|n| n.is_text()) {
        let text = text_node.text().unwrap_or("").to_string();
        let byte_pos = text_node.range().start;
        let text_pos = doc.text_pos_at(byte_pos);

        return Ok(StBody {
            text,
            line_offset: text_pos.row.saturating_sub(1) as usize,
            col_offset: text_pos.col.saturating_sub(1) as usize,
        });
    }

    Ok(StBody {
        text: String::new(),
        line_offset: 0,
        col_offset: 0,
    })
}

fn parse_actions(doc: &roxmltree::Document, node: roxmltree::Node) -> Result<Actions, String> {
    let mut actions = Actions::default();

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "action" {
            actions.action.push(parse_action(doc, child)?);
        }
    }

    Ok(actions)
}

fn parse_action(doc: &roxmltree::Document, node: roxmltree::Node) -> Result<Action, String> {
    let name = node.attribute("name").unwrap_or("").to_string();
    let global_id = node.attribute("globalId").map(String::from);
    let mut body = Body::default();

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "body" {
            body = parse_body(doc, child)?;
        }
    }

    Ok(Action {
        name,
        global_id,
        body,
    })
}

fn parse_transitions(
    doc: &roxmltree::Document,
    node: roxmltree::Node,
) -> Result<Transitions, String> {
    let mut transitions = Transitions::default();

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "transition" {
            transitions.transition.push(parse_transition(doc, child)?);
        }
    }

    Ok(transitions)
}

fn parse_transition(
    doc: &roxmltree::Document,
    node: roxmltree::Node,
) -> Result<Transition, String> {
    let name = node.attribute("name").unwrap_or("").to_string();
    let global_id = node.attribute("globalId").map(String::from);
    let mut body = Body::default();

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "body" {
            body = parse_body(doc, child)?;
        }
    }

    Ok(Transition {
        name,
        global_id,
        body,
    })
}

fn parse_value(node: roxmltree::Node) -> Result<Value, String> {
    let mut value = Value::default();

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "simpleValue" => {
                value.simple_value = Some(super::schema::SimpleValue {
                    value: child.attribute("value").map(String::from),
                });
            }
            "arrayValue" => {
                value.array_value = Some(parse_array_value(child)?);
            }
            "structValue" => {
                value.struct_value = Some(parse_struct_value(child)?);
            }
            _ => {}
        }
    }

    Ok(value)
}

fn parse_array_value(node: roxmltree::Node) -> Result<super::schema::ArrayValue, String> {
    let mut elements = Vec::new();

    for child in node.children().filter(|n| n.is_element() && n.tag_name().name() == "value") {
        let repetition_value = child.attribute("repetitionValue").map(String::from);
        let inner_value = parse_value(child)?;
        elements.push(super::schema::ArrayValueElement {
            repetition_value,
            value: inner_value,
        });
    }

    Ok(super::schema::ArrayValue { value: elements })
}

fn parse_struct_value(node: roxmltree::Node) -> Result<super::schema::StructValue, String> {
    let mut elements = Vec::new();

    for child in node.children().filter(|n| n.is_element() && n.tag_name().name() == "value") {
        let member = child.attribute("member").unwrap_or("").to_string();
        let inner_value = parse_value(child)?;
        elements.push(super::schema::StructValueElement {
            member,
            value: inner_value,
        });
    }

    Ok(super::schema::StructValue { value: elements })
}

fn parse_instances(node: roxmltree::Node) -> Result<Instances, String> {
    let mut instances = Instances::default();

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "configurations" {
            instances.configurations = parse_configurations(child)?;
        }
    }

    Ok(instances)
}

fn parse_configurations(node: roxmltree::Node) -> Result<Configurations, String> {
    let mut configs = Configurations::default();

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "configuration" {
            configs.configuration.push(parse_configuration(child)?);
        }
    }

    Ok(configs)
}

fn parse_configuration(node: roxmltree::Node) -> Result<Configuration, String> {
    let name = node.attribute("name").unwrap_or("").to_string();
    let global_id = node.attribute("globalId").map(String::from);
    let mut resources = Vec::new();
    let mut global_vars = Vec::new();

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "resource" => resources.push(parse_resource(child)?),
            "globalVars" => global_vars.push(parse_var_list(child)?),
            _ => {}
        }
    }

    Ok(Configuration {
        name,
        global_id,
        resource: resources,
        global_vars,
    })
}

fn parse_resource(node: roxmltree::Node) -> Result<Resource, String> {
    let name = node.attribute("name").unwrap_or("").to_string();
    let global_id = node.attribute("globalId").map(String::from);
    let mut tasks = Vec::new();
    let mut global_vars = Vec::new();
    let mut pou_instances = Vec::new();

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "task" => tasks.push(parse_task(child)?),
            "globalVars" => global_vars.push(parse_var_list(child)?),
            "pouInstance" => pou_instances.push(parse_pou_instance(child)?),
            _ => {}
        }
    }

    Ok(Resource {
        name,
        global_id,
        task: tasks,
        global_vars,
        pou_instance: pou_instances,
    })
}

fn parse_task(node: roxmltree::Node) -> Result<Task, String> {
    let name = node.attribute("name").unwrap_or("").to_string();
    let priority = node.attribute("priority").unwrap_or("0").to_string();
    let interval = node.attribute("interval").map(String::from);
    let single = node.attribute("single").map(String::from);
    let global_id = node.attribute("globalId").map(String::from);
    let mut pou_instances = Vec::new();

    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "pouInstance" {
            pou_instances.push(parse_pou_instance(child)?);
        }
    }

    Ok(Task {
        name,
        priority,
        interval,
        single,
        global_id,
        pou_instance: pou_instances,
    })
}

fn parse_pou_instance(node: roxmltree::Node) -> Result<PouInstance, String> {
    Ok(PouInstance {
        name: node.attribute("name").unwrap_or("").to_string(),
        type_name: node.attribute("typeName").unwrap_or("").to_string(),
        global_id: node.attribute("globalId").map(String::from),
    })
}

/// Get text content from an element
fn get_text_content(node: roxmltree::Node) -> Option<String> {
    node.text().map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_file_id() -> FileId {
        FileId::from_string("test.xml")
    }

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

        let project = parse_plcopen_xml(xml, &test_file_id()).unwrap();

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

        let project = parse_plcopen_xml(xml, &test_file_id()).unwrap();

        assert_eq!(project.types.pous.pou.len(), 1);
        let pou = &project.types.pous.pou[0];
        assert_eq!(pou.name, "Counter");
        assert_eq!(pou.pou_type, PouType::FunctionBlock);

        let interface = pou.interface.as_ref().unwrap();
        assert_eq!(interface.input_vars.len(), 1);
        assert_eq!(interface.input_vars[0].variable.len(), 1);
        assert_eq!(interface.input_vars[0].variable[0].name, "Reset");
        assert!(matches!(
            interface.input_vars[0].variable[0].var_type,
            DataType::Bool
        ));

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

        let project = parse_plcopen_xml(xml, &test_file_id()).unwrap();

        assert_eq!(project.types.data_types.data_type.len(), 1);
        let dt = &project.types.data_types.data_type[0];
        assert_eq!(dt.name, "TrafficLight");

        let DataType::Enum(enum_type) = &dt.base_type else {
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

        let project = parse_plcopen_xml(xml, &test_file_id()).unwrap();

        let dt = &project.types.data_types.data_type[0];
        assert_eq!(dt.name, "IntArray");

        let DataType::Array(array) = &dt.base_type else {
            panic!("Expected array type");
        };
        assert_eq!(array.dimension.len(), 1);
        assert_eq!(array.dimension[0].lower, "0");
        assert_eq!(array.dimension[0].upper, "9");
        assert!(matches!(array.base_type, DataType::Int));
    }

    #[test]
    fn parse_when_st_body_then_captures_position() {
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
        <interface/>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">IF Reset THEN
  Count := 0;
END_IF;</xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;

        let project = parse_plcopen_xml(xml, &test_file_id()).unwrap();
        let pou = &project.types.pous.pou[0];
        let body = pou.body.as_ref().unwrap();
        let st_body = body.st_body().unwrap();

        // The ST body text should start at a valid line position
        assert!(st_body.line_offset > 0, "Expected line > 0");
        assert!(st_body.text.contains("IF Reset THEN"));
    }

    #[test]
    fn parse_when_multiple_pous_then_returns_all() {
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
      <pou name="POU1" pouType="functionBlock">
        <interface/>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">x := 1;</xhtml>
          </ST>
        </body>
      </pou>
      <pou name="POU2" pouType="functionBlock">
        <interface/>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">y := 2;</xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;

        let project = parse_plcopen_xml(xml, &test_file_id()).unwrap();

        assert_eq!(project.types.pous.pou.len(), 2);
        assert_eq!(project.types.pous.pou[0].name, "POU1");
        assert_eq!(project.types.pous.pou[1].name, "POU2");

        // POU2 should be on a later line than POU1
        let pos1 = project.types.pous.pou[0]
            .body
            .as_ref()
            .unwrap()
            .st_body()
            .unwrap()
            .line_offset;
        let pos2 = project.types.pous.pou[1]
            .body
            .as_ref()
            .unwrap()
            .st_body()
            .unwrap()
            .line_offset;
        assert!(pos2 > pos1);
    }

    #[test]
    fn parse_when_comprehensive_project_then_extracts_all_components() {
        // This test verifies that all major components of a PLCopen XML document
        // are parsed correctly
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="TestCorp" companyURL="http://test.com" productName="TestProduct" productVersion="2.0" productRelease="beta" creationDateTime="2024-06-15T10:30:00" contentDescription="Test project"/>
  <contentHeader name="ComprehensiveTest" version="1.0" author="Tester" organization="TestOrg" language="en">
    <Comment>Project comment</Comment>
    <coordinateInfo>
      <fbd><scaling x="1" y="1"/></fbd>
      <ld><scaling x="1" y="1"/></ld>
      <sfc><scaling x="1" y="1"/></sfc>
    </coordinateInfo>
  </contentHeader>
  <types>
    <dataTypes>
      <dataType name="MyEnum">
        <baseType>
          <enum>
            <values>
              <value name="Val1"/>
              <value name="Val2"/>
            </values>
          </enum>
        </baseType>
      </dataType>
      <dataType name="MyArray">
        <baseType>
          <array>
            <dimension lower="1" upper="10"/>
            <baseType><INT/></baseType>
          </array>
        </baseType>
      </dataType>
      <dataType name="MyStruct">
        <baseType>
          <struct>
            <variable name="Field1">
              <type><REAL/></type>
            </variable>
            <variable name="Field2">
              <type><BOOL/></type>
            </variable>
          </struct>
        </baseType>
      </dataType>
    </dataTypes>
    <pous>
      <pou name="AddNumbers" pouType="function">
        <interface>
          <returnType><INT/></returnType>
          <inputVars>
            <variable name="A">
              <type><INT/></type>
            </variable>
            <variable name="B">
              <type><INT/></type>
            </variable>
          </inputVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">AddNumbers := A + B;</xhtml>
          </ST>
        </body>
      </pou>
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
          <localVars>
            <variable name="Internal">
              <type><INT/></type>
            </variable>
          </localVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">IF Reset THEN Count := 0; END_IF;</xhtml>
          </ST>
        </body>
      </pou>
      <pou name="Main" pouType="program">
        <interface>
          <localVars>
            <variable name="MyCounter">
              <type><derived name="Counter"/></type>
            </variable>
          </localVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">MyCounter(Reset := FALSE);</xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;

        let project = parse_plcopen_xml(xml, &test_file_id()).unwrap();

        // Verify file header
        assert_eq!(project.file_header.company_name, "TestCorp");
        assert_eq!(
            project.file_header.company_url,
            Some("http://test.com".to_string())
        );
        assert_eq!(project.file_header.product_name, "TestProduct");
        assert_eq!(project.file_header.product_version, "2.0");
        assert_eq!(
            project.file_header.product_release,
            Some("beta".to_string())
        );
        assert_eq!(
            project.file_header.content_description,
            Some("Test project".to_string())
        );

        // Verify content header
        assert_eq!(project.content_header.name, "ComprehensiveTest");
        assert_eq!(project.content_header.version, Some("1.0".to_string()));
        assert_eq!(project.content_header.author, Some("Tester".to_string()));
        assert_eq!(
            project.content_header.organization,
            Some("TestOrg".to_string())
        );

        // Verify data types
        assert_eq!(project.types.data_types.data_type.len(), 3);

        // Enum type
        let enum_type = &project.types.data_types.data_type[0];
        assert_eq!(enum_type.name, "MyEnum");
        let DataType::Enum(e) = &enum_type.base_type else {
            panic!("Expected enum")
        };
        assert_eq!(e.values.value.len(), 2);
        assert_eq!(e.values.value[0].name, "Val1");
        assert_eq!(e.values.value[1].name, "Val2");

        // Array type
        let array_type = &project.types.data_types.data_type[1];
        assert_eq!(array_type.name, "MyArray");
        let DataType::Array(a) = &array_type.base_type else {
            panic!("Expected array")
        };
        assert_eq!(a.dimension.len(), 1);
        assert_eq!(a.dimension[0].lower, "1");
        assert_eq!(a.dimension[0].upper, "10");
        assert!(matches!(a.base_type, DataType::Int));

        // Struct type
        let struct_type = &project.types.data_types.data_type[2];
        assert_eq!(struct_type.name, "MyStruct");
        let DataType::Struct(s) = &struct_type.base_type else {
            panic!("Expected struct")
        };
        assert_eq!(s.variable.len(), 2);
        assert_eq!(s.variable[0].name, "Field1");
        assert!(matches!(s.variable[0].member_type, DataType::Real));
        assert_eq!(s.variable[1].name, "Field2");
        assert!(matches!(s.variable[1].member_type, DataType::Bool));

        // Verify POUs
        assert_eq!(project.types.pous.pou.len(), 3);

        // Function
        let func = &project.types.pous.pou[0];
        assert_eq!(func.name, "AddNumbers");
        assert_eq!(func.pou_type, PouType::Function);
        let interface = func.interface.as_ref().unwrap();
        assert!(interface.return_type.is_some());
        assert!(matches!(
            interface.return_type.as_ref().unwrap(),
            DataType::Int
        ));
        assert_eq!(interface.input_vars.len(), 1);
        assert_eq!(interface.input_vars[0].variable.len(), 2);
        assert_eq!(interface.input_vars[0].variable[0].name, "A");
        assert_eq!(interface.input_vars[0].variable[1].name, "B");
        assert!(func.body.as_ref().unwrap().is_st());
        assert!(func
            .body
            .as_ref()
            .unwrap()
            .st_text()
            .unwrap()
            .contains("AddNumbers := A + B"));

        // Function block
        let fb = &project.types.pous.pou[1];
        assert_eq!(fb.name, "Counter");
        assert_eq!(fb.pou_type, PouType::FunctionBlock);
        let fb_interface = fb.interface.as_ref().unwrap();
        assert_eq!(fb_interface.input_vars.len(), 1);
        assert_eq!(fb_interface.output_vars.len(), 1);
        assert_eq!(fb_interface.local_vars.len(), 1);
        assert_eq!(fb_interface.input_vars[0].variable[0].name, "Reset");
        assert_eq!(fb_interface.output_vars[0].variable[0].name, "Count");
        assert_eq!(fb_interface.local_vars[0].variable[0].name, "Internal");

        // Program
        let prog = &project.types.pous.pou[2];
        assert_eq!(prog.name, "Main");
        assert_eq!(prog.pou_type, PouType::Program);
        let prog_interface = prog.interface.as_ref().unwrap();
        assert_eq!(prog_interface.local_vars.len(), 1);
        let local_var = &prog_interface.local_vars[0].variable[0];
        assert_eq!(local_var.name, "MyCounter");
        let DataType::Derived(d) = &local_var.var_type else {
            panic!("Expected derived type")
        };
        assert_eq!(d.name, "Counter");
    }

    #[test]
    fn parse_when_variable_with_array_initial_value_then_extracts_elements() {
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
      <pou name="TestPou" pouType="program">
        <interface>
          <localVars>
            <variable name="MyArray">
              <type>
                <array>
                  <dimension lower="0" upper="2"/>
                  <baseType><INT/></baseType>
                </array>
              </type>
              <initialValue>
                <arrayValue>
                  <value><simpleValue value="1"/></value>
                  <value><simpleValue value="2"/></value>
                  <value repetitionValue="3"><simpleValue value="0"/></value>
                </arrayValue>
              </initialValue>
            </variable>
          </localVars>
        </interface>
      </pou>
    </pous>
  </types>
</project>"#;

        let project = parse_plcopen_xml(xml, &test_file_id()).unwrap();
        let pou = &project.types.pous.pou[0];
        let interface = pou.interface.as_ref().unwrap();
        let var = &interface.local_vars[0].variable[0];

        assert_eq!(var.name, "MyArray");
        let init = var.initial_value.as_ref().unwrap();
        let arr = init.array_value.as_ref().unwrap();
        assert_eq!(arr.value.len(), 3);

        // First element: simpleValue "1"
        assert_eq!(
            arr.value[0].value.simple_value.as_ref().unwrap().value,
            Some("1".to_string())
        );
        assert!(arr.value[0].repetition_value.is_none());

        // Second element: simpleValue "2"
        assert_eq!(
            arr.value[1].value.simple_value.as_ref().unwrap().value,
            Some("2".to_string())
        );

        // Third element: repeated 3 times, simpleValue "0"
        assert_eq!(arr.value[2].repetition_value, Some("3".to_string()));
        assert_eq!(
            arr.value[2].value.simple_value.as_ref().unwrap().value,
            Some("0".to_string())
        );
    }

    #[test]
    fn parse_when_variable_with_struct_initial_value_then_extracts_members() {
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
      <dataType name="Point">
        <baseType>
          <struct>
            <variable name="X"><type><INT/></type></variable>
            <variable name="Y"><type><INT/></type></variable>
          </struct>
        </baseType>
      </dataType>
    </dataTypes>
    <pous>
      <pou name="TestPou" pouType="program">
        <interface>
          <localVars>
            <variable name="MyPoint">
              <type><derived name="Point"/></type>
              <initialValue>
                <structValue>
                  <value member="X"><simpleValue value="10"/></value>
                  <value member="Y"><simpleValue value="20"/></value>
                </structValue>
              </initialValue>
            </variable>
          </localVars>
        </interface>
      </pou>
    </pous>
  </types>
</project>"#;

        let project = parse_plcopen_xml(xml, &test_file_id()).unwrap();
        let pou = &project.types.pous.pou[0];
        let interface = pou.interface.as_ref().unwrap();
        let var = &interface.local_vars[0].variable[0];

        assert_eq!(var.name, "MyPoint");
        let init = var.initial_value.as_ref().unwrap();
        let sv = init.struct_value.as_ref().unwrap();
        assert_eq!(sv.value.len(), 2);

        // First member: X = 10
        assert_eq!(sv.value[0].member, "X");
        assert_eq!(
            sv.value[0].value.simple_value.as_ref().unwrap().value,
            Some("10".to_string())
        );

        // Second member: Y = 20
        assert_eq!(sv.value[1].member, "Y");
        assert_eq!(
            sv.value[1].value.simple_value.as_ref().unwrap().value,
            Some("20".to_string())
        );
    }
}
