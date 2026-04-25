//! The `types_all` MCP tool.
//!
//! Returns every user-defined type declared in the supplied sources with
//! kind-specific detail fields. Implements REQ-TOL-240.

use ironplc_analyzer::intermediate_type::{ByteSized, IntermediateType};
use ironplc_analyzer::SemanticContext;
use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::FileId;
use ironplc_project::project::{MemoryBackedProject, Project};
use serde::Serialize;

use super::common::{parse_options, serialize_diagnostics, validate_sources, SourceInput};

/// A single entry in the `types` array.
#[derive(Debug, Clone, Serialize)]
pub struct TypeEntry {
    pub name: String,
    pub kind: String,
    // Enum
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    // Struct
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<StructFieldEntry>>,
    // Array
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Vec<ArrayBound>>,
    // Subrange
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<i128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<i128>,
    // String
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<u128>,
    // Reference / alias
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StructFieldEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArrayBound {
    pub lower: i32,
    pub upper: i32,
}

/// Response returned by the `types_all` tool.
#[derive(Debug, Serialize)]
pub struct TypesAllResponse {
    pub ok: bool,
    pub types: Vec<TypeEntry>,
    pub diagnostics: Vec<serde_json::Value>,
}

impl TypesAllResponse {
    fn empty(ok: bool, diagnostics: Vec<serde_json::Value>) -> Self {
        Self {
            ok,
            types: vec![],
            diagnostics,
        }
    }
}

/// Builds the `types_all` response from raw inputs.
pub fn build_response(
    sources: &[SourceInput],
    options_value: &serde_json::Value,
) -> TypesAllResponse {
    let source_errors = validate_sources(sources);
    if !source_errors.is_empty() {
        return TypesAllResponse::empty(false, serialize_diagnostics(&source_errors));
    }

    let options = match parse_options(options_value) {
        Ok(opts) => opts,
        Err(errs) => {
            return TypesAllResponse::empty(false, serialize_diagnostics(&errs));
        }
    };

    let mut project = MemoryBackedProject::new(options);
    for src in sources {
        project.add_source(FileId::from_string(&src.name), src.content.clone());
    }

    let diagnostics_json = match project.semantic() {
        Ok(()) => vec![],
        Err(diags) => serialize_diagnostics(&diags),
    };

    let has_errors = diagnostics_json
        .iter()
        .any(|d| d["severity"].as_str() == Some("error"));

    let context = match project.semantic_context() {
        Some(ctx) => ctx,
        None => {
            return TypesAllResponse::empty(!has_errors, diagnostics_json);
        }
    };

    let mut types = collect_types(context);
    types.sort_by(|a, b| a.name.cmp(&b.name));

    TypesAllResponse {
        ok: !has_errors,
        types,
        diagnostics: diagnostics_json,
    }
}

fn collect_types(context: &SemanticContext) -> Vec<TypeEntry> {
    let mut entries = Vec::new();

    for (name, attrs) in context.types().iter_user_defined() {
        let entry = match &attrs.representation {
            IntermediateType::Enumeration { .. } => {
                let values: Vec<String> = context
                    .symbols()
                    .get_enumeration_values_for_type(name)
                    .into_iter()
                    .map(|v| v.to_string())
                    .collect();
                TypeEntry {
                    name: name.to_string(),
                    kind: "enum".into(),
                    values: Some(values),
                    ..empty_entry(name)
                }
            }
            IntermediateType::Structure { fields } => {
                let fs: Vec<StructFieldEntry> = fields
                    .iter()
                    .map(|f| StructFieldEntry {
                        name: f.name.to_string(),
                        type_name: render_type(&f.field_type),
                    })
                    .collect();
                TypeEntry {
                    name: name.to_string(),
                    kind: "struct".into(),
                    fields: Some(fs),
                    ..empty_entry(name)
                }
            }
            IntermediateType::Array {
                element_type,
                dimensions,
            } => {
                let bs: Vec<ArrayBound> = dimensions
                    .iter()
                    .map(|d| ArrayBound {
                        lower: d.lower,
                        upper: d.upper,
                    })
                    .collect();
                TypeEntry {
                    name: name.to_string(),
                    kind: "array".into(),
                    element_type: Some(render_type(element_type)),
                    bounds: Some(bs),
                    ..empty_entry(name)
                }
            }
            IntermediateType::Subrange {
                base_type,
                min_value,
                max_value,
            } => TypeEntry {
                name: name.to_string(),
                kind: "subrange".into(),
                base_type: Some(render_type(base_type)),
                low: Some(*min_value),
                high: Some(*max_value),
                ..empty_entry(name)
            },
            IntermediateType::String { max_len } => TypeEntry {
                name: name.to_string(),
                kind: "string".into(),
                length: *max_len,
                ..empty_entry(name)
            },
            IntermediateType::Reference { target_type } => TypeEntry {
                name: name.to_string(),
                kind: "reference".into(),
                target_type: Some(render_type(target_type)),
                ..empty_entry(name)
            },
            other => TypeEntry {
                name: name.to_string(),
                kind: "alias".into(),
                target_type: Some(render_type(other)),
                ..empty_entry(name)
            },
        };
        entries.push(entry);
    }

    entries
}

fn empty_entry(name: &TypeName) -> TypeEntry {
    TypeEntry {
        name: name.to_string(),
        kind: String::new(),
        values: None,
        fields: None,
        element_type: None,
        bounds: None,
        base_type: None,
        low: None,
        high: None,
        length: None,
        target_type: None,
    }
}

fn render_type(ty: &IntermediateType) -> String {
    match ty {
        IntermediateType::Bool => "BOOL".into(),
        IntermediateType::Int { size } => match size {
            ByteSized::B8 => "SINT".into(),
            ByteSized::B16 => "INT".into(),
            ByteSized::B32 => "DINT".into(),
            ByteSized::B64 => "LINT".into(),
        },
        IntermediateType::UInt { size } => match size {
            ByteSized::B8 => "USINT".into(),
            ByteSized::B16 => "UINT".into(),
            ByteSized::B32 => "UDINT".into(),
            ByteSized::B64 => "ULINT".into(),
        },
        IntermediateType::Real { size } => match size {
            ByteSized::B32 => "REAL".into(),
            ByteSized::B64 => "LREAL".into(),
            _ => "REAL".into(),
        },
        IntermediateType::Bytes { size } => match size {
            ByteSized::B8 => "BYTE".into(),
            ByteSized::B16 => "WORD".into(),
            ByteSized::B32 => "DWORD".into(),
            ByteSized::B64 => "LWORD".into(),
        },
        IntermediateType::Time { size } => match size {
            ByteSized::B64 => "LTIME".into(),
            _ => "TIME".into(),
        },
        IntermediateType::Date { size } => match size {
            ByteSized::B64 => "LDATE".into(),
            _ => "DATE".into(),
        },
        IntermediateType::TimeOfDay { size } => match size {
            ByteSized::B64 => "LTOD".into(),
            _ => "TIME_OF_DAY".into(),
        },
        IntermediateType::DateAndTime { size } => match size {
            ByteSized::B64 => "LDT".into(),
            _ => "DATE_AND_TIME".into(),
        },
        IntermediateType::String { .. } => "STRING".into(),
        IntermediateType::Enumeration { .. } => "ENUM".into(),
        IntermediateType::Structure { .. } => "STRUCT".into(),
        IntermediateType::Array { element_type, .. } => {
            format!("ARRAY OF {}", render_type(element_type))
        }
        IntermediateType::Subrange { base_type, .. } => render_type(base_type),
        IntermediateType::FunctionBlock { name, .. } => name.clone(),
        IntermediateType::Function { .. } => "FUNCTION".into(),
        IntermediateType::Reference { target_type } => {
            format!("REF_TO {}", render_type(target_type))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed2_options() -> serde_json::Value {
        serde_json::json!({"dialect": "iec61131-3-ed2"})
    }

    fn build(src: &str) -> TypesAllResponse {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: src.into(),
        }];
        build_response(&sources, &ed2_options())
    }

    #[test]
    fn build_response_when_enum_type_then_kind_enum_with_values() {
        let resp =
            build("TYPE MyEnum : (Stopped, Running, Fault); END_TYPE\nPROGRAM p\nEND_PROGRAM");
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let entry = resp
            .types
            .iter()
            .find(|t| t.name == "MyEnum")
            .expect("MyEnum not found");
        assert_eq!(entry.kind, "enum");
        let mut values = entry.values.clone().unwrap();
        values.sort();
        assert_eq!(
            values,
            vec![
                "Fault".to_string(),
                "Running".to_string(),
                "Stopped".to_string()
            ]
        );
    }

    #[test]
    fn build_response_when_struct_type_then_kind_struct_with_fields() {
        let resp = build(
            "TYPE PidParams : STRUCT Kp : REAL; Ki : REAL; END_STRUCT; END_TYPE\nPROGRAM p\nEND_PROGRAM",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let entry = resp
            .types
            .iter()
            .find(|t| t.name == "PidParams")
            .expect("PidParams not found");
        assert_eq!(entry.kind, "struct");
        let fields = entry.fields.clone().unwrap();
        let names: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
        assert!(names.contains(&"Kp".to_string()));
        assert!(names.contains(&"Ki".to_string()));
    }

    #[test]
    fn build_response_when_array_type_then_kind_array_with_bounds() {
        let resp = build("TYPE Buf : ARRAY[1..10] OF INT; END_TYPE\nPROGRAM p\nEND_PROGRAM");
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let entry = resp
            .types
            .iter()
            .find(|t| t.name == "Buf")
            .expect("Buf not found");
        assert_eq!(entry.kind, "array");
        let bounds = entry.bounds.clone().unwrap();
        assert_eq!(bounds.len(), 1);
        assert_eq!(bounds[0].lower, 1);
        assert_eq!(bounds[0].upper, 10);
        assert_eq!(entry.element_type.as_deref(), Some("INT"));
    }

    #[test]
    fn build_response_when_subrange_type_then_kind_subrange_with_low_high() {
        let resp = build("TYPE Percent : INT (0..100); END_TYPE\nPROGRAM p\nEND_PROGRAM");
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let entry = resp
            .types
            .iter()
            .find(|t| t.name == "Percent")
            .expect("Percent not found");
        assert_eq!(entry.kind, "subrange");
        assert_eq!(entry.low, Some(0));
        assert_eq!(entry.high, Some(100));
        assert_eq!(entry.base_type.as_deref(), Some("INT"));
    }

    #[test]
    fn build_response_when_semantic_error_then_ok_false() {
        let resp = build("PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM");
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_empty_source_name_then_p8001() {
        let sources = vec![SourceInput {
            name: String::new(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(!resp.ok);
        assert!(resp.diagnostics.iter().any(|d| d["code"] == "P8001"));
    }

    #[test]
    fn build_response_when_missing_dialect_then_p8001() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &serde_json::json!({}));
        assert!(!resp.ok);
        assert!(resp.diagnostics.iter().any(|d| d["code"] == "P8001"));
    }

    #[test]
    fn build_response_when_multiple_types_then_sorted_by_name() {
        let resp = build(
            "TYPE Zeta : (A, B); END_TYPE\nTYPE Alpha : (A, B); END_TYPE\nPROGRAM p\nEND_PROGRAM",
        );
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        let names: Vec<String> = resp.types.iter().map(|t| t.name.clone()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }
}
