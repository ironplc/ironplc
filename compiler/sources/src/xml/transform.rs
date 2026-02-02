//! Transformation from PLCopen XML schema to IronPLC DSL
//!
//! This module transforms parsed PLCopen XML structures into
//! IronPLC's internal DSL representation.

use ironplc_dsl::{
    common::{
        ArrayDeclaration, ArraySubranges, Boolean, BooleanLiteral, ConstantKind,
        DataTypeDeclarationKind, DeclarationQualifier, ElementaryTypeName,
        EnumeratedSpecificationInit, EnumeratedSpecificationValues, EnumeratedValue,
        EnumerationDeclaration, FunctionBlockBodyKind, FunctionBlockDeclaration,
        FunctionDeclaration, InitialValueAssignmentKind, Integer, Library, LibraryElementKind,
        ProgramDeclaration, SignedInteger, SimpleDeclaration, SimpleInitializer, SpecificationKind,
        StructureDeclaration, StructureElementDeclaration, Subrange, SubrangeDeclaration,
        SubrangeSpecification, TypeName, VarDecl, VariableIdentifier, VariableType,
    },
    configuration::{
        ConfigurationDeclaration, ProgramConfiguration, ResourceDeclaration, TaskConfiguration,
    },
    core::{FileId, Id, SourceSpan},
    diagnostic::{Diagnostic, Label},
    sfc::{
        Action as SfcAction, ActionAssociation, ActionQualifier, ElementKind, Network, Sfc, Step,
        Transition,
    },
    textual::{ExprKind, Statements, StmtKind},
    time::DurationLiteral,
};
use ironplc_parser::options::ParseOptions;
use ironplc_problems::Problem;

use super::schema::{
    ArrayType, Configuration, DataType, DataTypeDecl, Dimension, EnumType, Instances, Interface,
    Pou, PouInstance, PouType, Project, Resource, SfcBody, StructType, SubrangeSigned,
    SubrangeUnsigned, Task, VarList, Variable,
};

/// Create a SourceSpan for the given file with no position info
fn file_span(file_id: &FileId) -> SourceSpan {
    SourceSpan::range(0, 0).with_file_id(file_id)
}

/// Transform a parsed PLCopen XML project into an IronPLC Library
///
/// This is the main entry point for XML -> DSL transformation.
pub fn transform_project(project: &Project, file_id: &FileId) -> Result<Library, Diagnostic> {
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
        let elem = transform_pou(pou, file_id)?;
        library.elements.push(elem);
    }

    // Transform instances (configurations)
    if let Some(ref instances) = project.instances {
        let configs = transform_instances(instances, file_id)?;
        for config in configs {
            library
                .elements
                .push(LibraryElementKind::ConfigurationDeclaration(config));
        }
    }

    Ok(library)
}

/// Transform a data type declaration
fn transform_data_type_decl(
    decl: &DataTypeDecl,
    file_id: &FileId,
) -> Result<DataTypeDeclarationKind, Diagnostic> {
    let type_name = TypeName::from(decl.name.as_str());

    match &decl.base_type {
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

    let spec = SpecificationKind::Inline(EnumeratedSpecificationValues { values });

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
        spec: SpecificationKind::Inline(ArraySubranges {
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
        spec: SpecificationKind::Inline(SubrangeSpecification {
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
        spec: SpecificationKind::Inline(SubrangeSpecification {
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
    data_type: &DataType,
    file_id: &FileId,
) -> Result<ElementaryTypeName, Diagnostic> {
    let span = file_span(file_id);
    match data_type {
        DataType::SInt => Ok(ElementaryTypeName::SINT),
        DataType::Int => Ok(ElementaryTypeName::INT),
        DataType::DInt => Ok(ElementaryTypeName::DINT),
        DataType::LInt => Ok(ElementaryTypeName::LINT),
        DataType::USInt => Ok(ElementaryTypeName::USINT),
        DataType::UInt => Ok(ElementaryTypeName::UINT),
        DataType::UDInt => Ok(ElementaryTypeName::UDINT),
        DataType::ULInt => Ok(ElementaryTypeName::ULINT),
        _ => Err(Diagnostic::problem(
            Problem::XmlSchemaViolation,
            Label::span(
                span,
                format!(
                    "Subrange base type must be an integer type, found: {}",
                    data_type.type_name()
                ),
            ),
        )),
    }
}

/// Transform a PLCopen DataType to a DSL TypeName
fn transform_data_type(data_type: &DataType, _file_id: &FileId) -> Result<TypeName, Diagnostic> {
    match data_type {
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
        DataType::Any => Ok(TypeName::from("ANY")),
        DataType::AnyDerived => Ok(TypeName::from("ANY_DERIVED")),
        DataType::AnyElementary => Ok(TypeName::from("ANY_ELEMENTARY")),
        DataType::AnyMagnitude => Ok(TypeName::from("ANY_MAGNITUDE")),
        DataType::AnyNum => Ok(TypeName::from("ANY_NUM")),
        DataType::AnyReal => Ok(TypeName::from("ANY_REAL")),
        DataType::AnyInt => Ok(TypeName::from("ANY_INT")),
        DataType::AnyBit => Ok(TypeName::from("ANY_BIT")),
        DataType::AnyString => Ok(TypeName::from("ANY_STRING")),
        DataType::AnyDate => Ok(TypeName::from("ANY_DATE")),

        // Subranges and pointers
        DataType::SubrangeSigned(_) | DataType::SubrangeUnsigned(_) => {
            Err(Diagnostic::todo(file!(), line!()))
        }
        DataType::Pointer(_) => Err(Diagnostic::todo(file!(), line!())),
    }
}

/// Transform a POU (Program Organization Unit)
fn transform_pou(pou: &Pou, file_id: &FileId) -> Result<LibraryElementKind, Diagnostic> {
    match pou.pou_type {
        PouType::Function => transform_function(pou, file_id),
        PouType::FunctionBlock => transform_function_block(pou, file_id),
        PouType::Program => transform_program(pou, file_id),
    }
}

/// Transform a function declaration
fn transform_function(pou: &Pou, file_id: &FileId) -> Result<LibraryElementKind, Diagnostic> {
    let name = Id::from(pou.name.as_str());
    let span = file_span(file_id);

    // Get return type (required for functions)
    let return_type = if let Some(ref interface) = pou.interface {
        if let Some(ref rt) = interface.return_type {
            transform_data_type(rt, file_id)?
        } else {
            // Functions must have a return type
            return Err(Diagnostic::problem(
                Problem::XmlSchemaViolation,
                Label::span(
                    span,
                    format!("Function '{}' is missing a return type", pou.name),
                ),
            ));
        }
    } else {
        return Err(Diagnostic::problem(
            Problem::XmlSchemaViolation,
            Label::span(
                span,
                format!("Function '{}' is missing interface", pou.name),
            ),
        ));
    };

    let variables = transform_interface(pou.interface.as_ref(), file_id)?;
    let body = transform_body_statements(pou, file_id)?;

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
fn transform_function_block(pou: &Pou, file_id: &FileId) -> Result<LibraryElementKind, Diagnostic> {
    let name = TypeName::from(pou.name.as_str());
    let span = file_span(file_id);

    let variables = transform_interface(pou.interface.as_ref(), file_id)?;
    let body = transform_body(pou, file_id)?;

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
fn transform_program(pou: &Pou, file_id: &FileId) -> Result<LibraryElementKind, Diagnostic> {
    let name = Id::from(pou.name.as_str());

    let variables = transform_interface(pou.interface.as_ref(), file_id)?;
    let body = transform_body(pou, file_id)?;

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
fn transform_body(pou: &Pou, file_id: &FileId) -> Result<FunctionBlockBodyKind, Diagnostic> {
    let Some(ref body) = pou.body else {
        return Ok(FunctionBlockBodyKind::Empty);
    };

    if let Some(st_body) = body.st_body() {
        let stmts = parse_st_body(
            &st_body.text,
            file_id,
            st_body.line_offset,
            st_body.col_offset,
        )?;
        Ok(FunctionBlockBodyKind::Statements(Statements {
            body: stmts,
        }))
    } else if let Some(ref sfc_body) = body.sfc {
        // Transform SFC body
        let sfc = transform_sfc_body(sfc_body, pou, file_id)?;
        Ok(FunctionBlockBodyKind::Sfc(sfc))
    } else {
        Ok(FunctionBlockBodyKind::Empty)
    }
}

/// Transform POU body to Vec<StmtKind> (for functions)
fn transform_body_statements(pou: &Pou, file_id: &FileId) -> Result<Vec<StmtKind>, Diagnostic> {
    let Some(ref body) = pou.body else {
        return Ok(vec![]);
    };

    if let Some(st_body) = body.st_body() {
        parse_st_body(
            &st_body.text,
            file_id,
            st_body.line_offset,
            st_body.col_offset,
        )
    } else {
        Ok(vec![])
    }
}

// ============================================================================
// Configuration transforms
// ============================================================================

/// Transform instances container to configuration declarations
fn transform_instances(
    instances: &Instances,
    file_id: &FileId,
) -> Result<Vec<ConfigurationDeclaration>, Diagnostic> {
    instances
        .configurations
        .configuration
        .iter()
        .map(|config| transform_configuration(config, file_id))
        .collect()
}

/// Transform a configuration declaration
fn transform_configuration(
    config: &Configuration,
    file_id: &FileId,
) -> Result<ConfigurationDeclaration, Diagnostic> {
    let name = Id::from(config.name.as_str());

    // Transform global variables
    let global_var = transform_global_vars(&config.global_vars, file_id)?;

    // Transform resources
    let resource_decl: Result<Vec<_>, _> = config
        .resource
        .iter()
        .map(|resource| transform_resource(resource, file_id))
        .collect();

    Ok(ConfigurationDeclaration {
        name,
        global_var,
        resource_decl: resource_decl?,
        fb_inits: vec![],
        located_var_inits: vec![],
    })
}

/// Transform a resource declaration
fn transform_resource(
    resource: &Resource,
    file_id: &FileId,
) -> Result<ResourceDeclaration, Diagnostic> {
    let name = Id::from(resource.name.as_str());

    // Transform global variables
    let global_vars = transform_global_vars(&resource.global_vars, file_id)?;

    // Transform tasks
    let tasks: Result<Vec<_>, _> = resource
        .task
        .iter()
        .map(|task| transform_task(task, file_id))
        .collect();

    // Transform program instances from the resource level
    let mut programs: Vec<ProgramConfiguration> = resource
        .pou_instance
        .iter()
        .map(|inst| transform_pou_instance(inst, None))
        .collect();

    // Also include program instances from tasks
    for task in &resource.task {
        for inst in &task.pou_instance {
            programs.push(transform_pou_instance(inst, Some(&task.name)));
        }
    }

    Ok(ResourceDeclaration {
        name: name.clone(),
        resource: name, // Use the same name for resource identifier
        global_vars,
        tasks: tasks?,
        programs,
    })
}

/// Transform a task configuration
fn transform_task(task: &Task, file_id: &FileId) -> Result<TaskConfiguration, Diagnostic> {
    let name = Id::from(task.name.as_str());

    // Parse priority
    let priority = task.priority.parse::<u32>().map_err(|_| {
        Diagnostic::problem(
            Problem::XmlSchemaViolation,
            Label::span(
                file_span(file_id),
                format!("Invalid task priority: '{}'", task.priority),
            ),
        )
    })?;

    // Parse interval if present
    let interval = if let Some(ref interval_str) = task.interval {
        Some(parse_duration(interval_str, file_id)?)
    } else {
        None
    };

    Ok(TaskConfiguration {
        name,
        priority,
        interval,
    })
}

/// Transform a POU instance (program configuration)
fn transform_pou_instance(instance: &PouInstance, task_name: Option<&str>) -> ProgramConfiguration {
    ProgramConfiguration {
        name: Id::from(instance.name.as_str()),
        storage: None,
        task_name: task_name.map(Id::from),
        type_name: Id::from(instance.type_name.as_str()),
        fb_tasks: vec![],
        sources: vec![],
        sinks: vec![],
    }
}

/// Transform global variables from VarList
fn transform_global_vars(
    var_lists: &[VarList],
    file_id: &FileId,
) -> Result<Vec<VarDecl>, Diagnostic> {
    let mut vars = vec![];
    for var_list in var_lists {
        vars.extend(transform_var_list(var_list, VariableType::Global, file_id)?);
    }
    Ok(vars)
}

/// Parse a duration string (e.g., "T#100ms", "T#1s") to DurationLiteral
fn parse_duration(duration_str: &str, file_id: &FileId) -> Result<DurationLiteral, Diagnostic> {
    use time::Duration;

    let span = file_span(file_id);

    // Strip the T# prefix if present
    let value_part = duration_str
        .strip_prefix("T#")
        .or_else(|| duration_str.strip_prefix("t#"))
        .unwrap_or(duration_str);

    // Try to parse common duration formats
    // Supports: Xms, Xs, Xm, Xh, Xd (and fractional values)
    if let Some(ms) = value_part.strip_suffix("ms") {
        let value: f64 = ms
            .parse()
            .map_err(|_| invalid_duration_error(duration_str, file_id))?;
        let micros = (value * 1000.0) as i64;
        return Ok(DurationLiteral {
            span,
            interval: Duration::microseconds(micros),
        });
    }
    if let Some(s) = value_part.strip_suffix('s') {
        let value: f64 = s
            .parse()
            .map_err(|_| invalid_duration_error(duration_str, file_id))?;
        let micros = (value * 1_000_000.0) as i64;
        return Ok(DurationLiteral {
            span,
            interval: Duration::microseconds(micros),
        });
    }
    if let Some(m) = value_part.strip_suffix('m') {
        let value: f64 = m
            .parse()
            .map_err(|_| invalid_duration_error(duration_str, file_id))?;
        let micros = (value * 60.0 * 1_000_000.0) as i64;
        return Ok(DurationLiteral {
            span,
            interval: Duration::microseconds(micros),
        });
    }
    if let Some(h) = value_part.strip_suffix('h') {
        let value: f64 = h
            .parse()
            .map_err(|_| invalid_duration_error(duration_str, file_id))?;
        let micros = (value * 3600.0 * 1_000_000.0) as i64;
        return Ok(DurationLiteral {
            span,
            interval: Duration::microseconds(micros),
        });
    }
    if let Some(d) = value_part.strip_suffix('d') {
        let value: f64 = d
            .parse()
            .map_err(|_| invalid_duration_error(duration_str, file_id))?;
        let micros = (value * 86400.0 * 1_000_000.0) as i64;
        return Ok(DurationLiteral {
            span,
            interval: Duration::microseconds(micros),
        });
    }

    Err(invalid_duration_error(duration_str, file_id))
}

/// Create an error diagnostic for invalid duration values
fn invalid_duration_error(value: &str, file_id: &FileId) -> Diagnostic {
    Diagnostic::problem(
        Problem::XmlSchemaViolation,
        Label::span(
            file_span(file_id),
            format!(
                "Invalid duration format: '{}'. Expected format like T#100ms, T#1s, T#1m",
                value
            ),
        ),
    )
}

// ============================================================================
// SFC transforms
// ============================================================================

/// Transform SFC body from XML to DSL Sfc
fn transform_sfc_body(sfc_body: &SfcBody, pou: &Pou, file_id: &FileId) -> Result<Sfc, Diagnostic> {
    // Find the initial step
    let initial_step = sfc_body
        .steps
        .iter()
        .find(|s| s.initial_step)
        .ok_or_else(|| {
            Diagnostic::problem(
                Problem::SfcMissingInitialStep,
                Label::span(file_span(file_id), "SFC body"),
            )
        })?;

    // Build action associations map from action blocks
    let action_associations = build_action_associations(sfc_body);

    // Transform initial step
    let initial_step_dsl = transform_sfc_step(initial_step, &action_associations);

    // Transform other elements (non-initial steps, transitions, actions)
    let mut elements = Vec::new();

    // Add non-initial steps
    for step in &sfc_body.steps {
        if !step.initial_step {
            elements.push(ElementKind::Step(transform_sfc_step(
                step,
                &action_associations,
            )));
        }
    }

    // Add transitions
    for transition in &sfc_body.transitions {
        elements.push(ElementKind::Transition(transform_sfc_transition(
            transition, file_id,
        )?));
    }

    // Add actions from POU-level actions container
    if let Some(ref actions) = pou.actions {
        for action in &actions.action {
            elements.push(ElementKind::Action(transform_sfc_action(action, file_id)?));
        }
    }

    Ok(Sfc {
        networks: vec![Network {
            initial_step: initial_step_dsl,
            elements,
        }],
    })
}

/// Build a map of step names to their action associations
fn build_action_associations(
    sfc_body: &SfcBody,
) -> std::collections::HashMap<String, Vec<ActionAssociation>> {
    let mut map = std::collections::HashMap::new();

    for action_block in &sfc_body.action_blocks {
        let associations: Vec<ActionAssociation> = action_block
            .actions
            .iter()
            .map(|a| ActionAssociation {
                name: Id::from(a.action_name.as_str()),
                qualifier: a.qualifier.as_ref().and_then(|q| parse_action_qualifier(q)),
                indicators: vec![],
            })
            .collect();

        map.insert(action_block.step_name.clone(), associations);
    }

    map
}

/// Transform SFC step
fn transform_sfc_step(
    step: &super::schema::SfcStep,
    action_associations: &std::collections::HashMap<String, Vec<ActionAssociation>>,
) -> Step {
    let associations = action_associations
        .get(&step.name)
        .cloned()
        .unwrap_or_default();

    Step {
        name: Id::from(step.name.as_str()),
        action_associations: associations,
    }
}

/// Transform SFC transition
fn transform_sfc_transition(
    transition: &super::schema::SfcTransition,
    file_id: &FileId,
) -> Result<Transition, Diagnostic> {
    // Parse the condition
    let condition = if let Some(ref st_body) = transition.condition_st {
        // Parse inline ST condition as an expression
        parse_st_condition(
            &st_body.text,
            file_id,
            st_body.line_offset,
            st_body.col_offset,
        )?
    } else if let Some(ref ref_name) = transition.condition_reference {
        // Reference to a named transition - use the name as a variable reference
        ExprKind::named_variable(ref_name)
    } else {
        // Default to TRUE if no condition specified
        ExprKind::Const(ConstantKind::Boolean(BooleanLiteral::new(Boolean::True)))
    };

    Ok(Transition {
        name: transition.name.as_ref().map(|n| Id::from(n.as_str())),
        priority: transition.priority,
        from: transition
            .from_steps
            .iter()
            .map(|s| Id::from(s.as_str()))
            .collect(),
        to: transition
            .to_steps
            .iter()
            .map(|s| Id::from(s.as_str()))
            .collect(),
        condition,
    })
}

/// Transform SFC action from POU-level action
fn transform_sfc_action(
    action: &super::schema::Action,
    file_id: &FileId,
) -> Result<SfcAction, Diagnostic> {
    let name = Id::from(action.name.as_str());

    // Parse the action body
    let body = if let Some(st_body) = action.body.st_body() {
        let stmts = parse_st_body(
            &st_body.text,
            file_id,
            st_body.line_offset,
            st_body.col_offset,
        )?;
        FunctionBlockBodyKind::Statements(Statements { body: stmts })
    } else {
        FunctionBlockBodyKind::Empty
    };

    Ok(SfcAction { name, body })
}

/// Parse an action qualifier string to ActionQualifier
fn parse_action_qualifier(qualifier: &str) -> Option<ActionQualifier> {
    match qualifier.to_uppercase().as_str() {
        "N" => Some(ActionQualifier::N),
        "R" => Some(ActionQualifier::R),
        "S" => Some(ActionQualifier::S),
        "L" => Some(ActionQualifier::L),
        "D" => Some(ActionQualifier::D),
        "P" => Some(ActionQualifier::P),
        // Timed qualifiers would need duration parsing
        _ => None,
    }
}

/// Parse ST condition expression
///
/// Note: Full expression parsing from embedded ST is not yet implemented.
/// This currently returns a simple TRUE literal as a placeholder.
fn parse_st_condition(
    _st_text: &str,
    _file_id: &FileId,
    _line_offset: usize,
    _col_offset: usize,
) -> Result<ExprKind, Diagnostic> {
    // TODO: Implement proper ST expression parsing for transition conditions
    // For now, return TRUE as placeholder
    Ok(ExprKind::Const(ConstantKind::Boolean(BooleanLiteral::new(
        Boolean::True,
    ))))
}

/// Parse ST body text using the ST parser
///
/// Uses the position information embedded in the StBody struct to provide
/// accurate line/column offsets for error reporting.
fn parse_st_body(
    st_text: &str,
    file_id: &FileId,
    line_offset: usize,
    col_offset: usize,
) -> Result<Vec<StmtKind>, Diagnostic> {
    let options = ParseOptions::default();
    ironplc_parser::parse_st_statements(st_text, file_id, &options, line_offset, col_offset)
}

/// Create an error diagnostic for invalid values
fn invalid_value_error(value: &str, context: &str, file_id: &FileId) -> Diagnostic {
    Diagnostic::problem(
        Problem::XmlSchemaViolation,
        Label::span(
            file_span(file_id),
            format!("Invalid {}: '{}'", context, value),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::position::parse_plcopen_xml;

    fn test_file_id() -> FileId {
        FileId::from_string("test.xml")
    }

    fn parse_project(xml: &str) -> Project {
        parse_plcopen_xml(xml, &test_file_id()).unwrap()
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
        let library = transform_project(&project, &test_file_id()).unwrap();

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
        let library = transform_project(&project, &test_file_id()).unwrap();

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
        let library = transform_project(&project, &test_file_id()).unwrap();

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
        let library = transform_project(&project, &test_file_id()).unwrap();

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
        let library = transform_project(&project, &test_file_id()).unwrap();

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
        let library = transform_project(&project, &test_file_id()).unwrap();

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
        let library = transform_project(&project, &test_file_id()).unwrap();

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

    // ========================================================================
    // Configuration transform tests
    // ========================================================================

    #[test]
    fn transform_when_configuration_then_creates_declaration() {
        let xml = format!(
            r#"{}
  <types>
    <dataTypes/>
    <pous>
      <pou name="Main" pouType="program">
        <interface/>
      </pou>
    </pous>
  </types>
  <instances>
    <configurations>
      <configuration name="Config1">
        <resource name="Resource1">
          <task name="MainTask" priority="1" interval="T#100ms">
            <pouInstance name="MainInstance" typeName="Main"/>
          </task>
        </resource>
      </configuration>
    </configurations>
  </instances>
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let library = transform_project(&project, &test_file_id()).unwrap();

        // Should have program declaration and configuration declaration
        assert_eq!(library.elements.len(), 2);

        let LibraryElementKind::ConfigurationDeclaration(config_decl) = &library.elements[1] else {
            panic!("Expected configuration declaration");
        };
        assert_eq!(config_decl.name.to_string(), "Config1");
        assert_eq!(config_decl.resource_decl.len(), 1);
    }

    #[test]
    fn transform_when_resource_then_creates_declaration() {
        let xml = format!(
            r#"{}
  <types>
    <dataTypes/>
    <pous>
      <pou name="Main" pouType="program">
        <interface/>
      </pou>
    </pous>
  </types>
  <instances>
    <configurations>
      <configuration name="Config1">
        <resource name="CPU1">
          <task name="FastTask" priority="1" interval="T#10ms"/>
          <task name="SlowTask" priority="10" interval="T#1s"/>
        </resource>
      </configuration>
    </configurations>
  </instances>
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let library = transform_project(&project, &test_file_id()).unwrap();

        let LibraryElementKind::ConfigurationDeclaration(config_decl) = &library.elements[1] else {
            panic!("Expected configuration declaration");
        };

        let resource = &config_decl.resource_decl[0];
        assert_eq!(resource.name.to_string(), "CPU1");
        assert_eq!(resource.tasks.len(), 2);
    }

    #[test]
    fn transform_when_task_with_interval_then_parses_duration() {
        let xml = format!(
            r#"{}
  <types>
    <dataTypes/>
    <pous/>
  </types>
  <instances>
    <configurations>
      <configuration name="Config1">
        <resource name="CPU1">
          <task name="PeriodicTask" priority="5" interval="T#100ms"/>
        </resource>
      </configuration>
    </configurations>
  </instances>
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let library = transform_project(&project, &test_file_id()).unwrap();

        let LibraryElementKind::ConfigurationDeclaration(config_decl) = &library.elements[0] else {
            panic!("Expected configuration declaration");
        };

        let task = &config_decl.resource_decl[0].tasks[0];
        assert_eq!(task.name.to_string(), "PeriodicTask");
        assert_eq!(task.priority, 5);
        assert!(task.interval.is_some());

        // 100ms = 100,000 microseconds
        let interval = task.interval.as_ref().unwrap();
        assert_eq!(interval.interval, time::Duration::microseconds(100_000));
    }

    #[test]
    fn transform_when_pou_instance_in_task_then_creates_program_config() {
        let xml = format!(
            r#"{}
  <types>
    <dataTypes/>
    <pous>
      <pou name="Main" pouType="program">
        <interface/>
      </pou>
    </pous>
  </types>
  <instances>
    <configurations>
      <configuration name="Config1">
        <resource name="CPU1">
          <task name="MainTask" priority="1" interval="T#100ms">
            <pouInstance name="MainProgram" typeName="Main"/>
          </task>
        </resource>
      </configuration>
    </configurations>
  </instances>
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let library = transform_project(&project, &test_file_id()).unwrap();

        let LibraryElementKind::ConfigurationDeclaration(config_decl) = &library.elements[1] else {
            panic!("Expected configuration declaration");
        };

        let resource = &config_decl.resource_decl[0];
        assert_eq!(resource.programs.len(), 1);

        let program = &resource.programs[0];
        assert_eq!(program.name.to_string(), "MainProgram");
        assert_eq!(program.type_name.to_string(), "Main");
        assert_eq!(program.task_name.as_ref().unwrap().to_string(), "MainTask");
    }

    #[test]
    fn transform_when_pou_instance_at_resource_level_then_creates_program_config() {
        let xml = format!(
            r#"{}
  <types>
    <dataTypes/>
    <pous>
      <pou name="Main" pouType="program">
        <interface/>
      </pou>
    </pous>
  </types>
  <instances>
    <configurations>
      <configuration name="Config1">
        <resource name="CPU1">
          <pouInstance name="FreeRunning" typeName="Main"/>
        </resource>
      </configuration>
    </configurations>
  </instances>
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let library = transform_project(&project, &test_file_id()).unwrap();

        let LibraryElementKind::ConfigurationDeclaration(config_decl) = &library.elements[1] else {
            panic!("Expected configuration declaration");
        };

        let resource = &config_decl.resource_decl[0];
        assert_eq!(resource.programs.len(), 1);

        let program = &resource.programs[0];
        assert_eq!(program.name.to_string(), "FreeRunning");
        assert!(program.task_name.is_none()); // No task association
    }

    #[test]
    fn parse_duration_when_milliseconds_then_correct() {
        let file_id = test_file_id();
        let duration = parse_duration("T#100ms", &file_id).unwrap();
        assert_eq!(duration.interval, time::Duration::microseconds(100_000));
    }

    #[test]
    fn parse_duration_when_seconds_then_correct() {
        let file_id = test_file_id();
        let duration = parse_duration("T#2s", &file_id).unwrap();
        assert_eq!(duration.interval, time::Duration::seconds(2));
    }

    #[test]
    fn parse_duration_when_minutes_then_correct() {
        let file_id = test_file_id();
        let duration = parse_duration("T#5m", &file_id).unwrap();
        assert_eq!(duration.interval, time::Duration::minutes(5));
    }

    #[test]
    fn parse_duration_when_lowercase_prefix_then_correct() {
        let file_id = test_file_id();
        let duration = parse_duration("t#500ms", &file_id).unwrap();
        assert_eq!(duration.interval, time::Duration::microseconds(500_000));
    }

    #[test]
    fn parse_duration_when_no_prefix_then_correct() {
        let file_id = test_file_id();
        let duration = parse_duration("1s", &file_id).unwrap();
        assert_eq!(duration.interval, time::Duration::seconds(1));
    }

    // ========================================================================
    // SFC transform tests
    // ========================================================================

    #[test]
    fn transform_when_sfc_body_with_steps_then_creates_sfc() {
        let xml = format!(
            r#"{}
  <types>
    <dataTypes/>
    <pous>
      <pou name="SfcProgram" pouType="program">
        <interface>
          <localVars>
            <variable name="x">
              <type><INT/></type>
            </variable>
          </localVars>
        </interface>
        <body>
          <SFC>
            <step localId="1" name="Init" initialStep="true"/>
            <step localId="2" name="Running"/>
          </SFC>
        </body>
        <actions>
          <action name="StartMotor">
            <body>
              <ST>
                <xhtml xmlns="http://www.w3.org/1999/xhtml">x := 1;</xhtml>
              </ST>
            </body>
          </action>
        </actions>
      </pou>
    </pous>
  </types>
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let library = transform_project(&project, &test_file_id()).unwrap();

        assert_eq!(library.elements.len(), 1);
        let LibraryElementKind::ProgramDeclaration(prog_decl) = &library.elements[0] else {
            panic!("Expected program declaration");
        };
        assert_eq!(prog_decl.name.to_string(), "SfcProgram");

        // Verify the body is an SFC
        let FunctionBlockBodyKind::Sfc(sfc) = &prog_decl.body else {
            panic!("Expected SFC body");
        };
        assert_eq!(sfc.networks.len(), 1);
        assert_eq!(sfc.networks[0].initial_step.name.to_string(), "Init");
    }

    #[test]
    fn transform_when_sfc_with_transition_then_creates_transition() {
        let xml = format!(
            r#"{}
  <types>
    <dataTypes/>
    <pous>
      <pou name="SfcProgram" pouType="program">
        <interface/>
        <body>
          <SFC>
            <step localId="1" name="Step1" initialStep="true"/>
            <step localId="2" name="Step2"/>
            <transition localId="3" name="T1" priority="1">
              <condition>
                <inline>
                  <ST>
                    <xhtml xmlns="http://www.w3.org/1999/xhtml">TRUE</xhtml>
                  </ST>
                </inline>
              </condition>
            </transition>
          </SFC>
        </body>
      </pou>
    </pous>
  </types>
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let library = transform_project(&project, &test_file_id()).unwrap();

        let LibraryElementKind::ProgramDeclaration(prog_decl) = &library.elements[0] else {
            panic!("Expected program declaration");
        };

        let FunctionBlockBodyKind::Sfc(sfc) = &prog_decl.body else {
            panic!("Expected SFC body");
        };

        // Should have initial step + 1 other step + 1 transition in elements
        let transition_count = sfc.networks[0]
            .elements
            .iter()
            .filter(|e| matches!(e, ElementKind::Transition(_)))
            .count();
        assert_eq!(transition_count, 1);
    }

    #[test]
    fn transform_when_sfc_with_action_then_creates_action() {
        let xml = format!(
            r#"{}
  <types>
    <dataTypes/>
    <pous>
      <pou name="SfcProgram" pouType="program">
        <interface/>
        <body>
          <SFC>
            <step localId="1" name="Step1" initialStep="true"/>
          </SFC>
        </body>
        <actions>
          <action name="DoSomething">
            <body>
              <ST>
                <xhtml xmlns="http://www.w3.org/1999/xhtml">;</xhtml>
              </ST>
            </body>
          </action>
        </actions>
      </pou>
    </pous>
  </types>
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let library = transform_project(&project, &test_file_id()).unwrap();

        let LibraryElementKind::ProgramDeclaration(prog_decl) = &library.elements[0] else {
            panic!("Expected program declaration");
        };

        let FunctionBlockBodyKind::Sfc(sfc) = &prog_decl.body else {
            panic!("Expected SFC body");
        };

        // Should have the action in elements
        let action_count = sfc.networks[0]
            .elements
            .iter()
            .filter(|e| matches!(e, ElementKind::Action(_)))
            .count();
        assert_eq!(action_count, 1);
    }

    #[test]
    fn transform_when_sfc_missing_initial_step_then_returns_error() {
        let xml = format!(
            r#"{}
  <types>
    <dataTypes/>
    <pous>
      <pou name="SfcProgram" pouType="program">
        <interface/>
        <body>
          <SFC>
            <step localId="1" name="Step1"/>
            <step localId="2" name="Step2"/>
          </SFC>
        </body>
      </pou>
    </pous>
  </types>
</project>"#,
            minimal_project_header()
        );

        let project = parse_project(&xml);
        let result = transform_project(&project, &test_file_id());

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, Problem::SfcMissingInitialStep.code());
    }
}
