//! The debug section's entry of each variable (`VAR_NAME`) and of each
//! string a global or the program declares (`STRING_LAYOUT`), from the
//! lowered declarations.
//!
//! A variable gets its debug type two ways:
//!
//! - a global's or the program's from the type it resolves to: an alias of
//!   an elementary type is known by the elementary type, an array by its
//!   elements;
//! - a function's or a function block's, and a function's result, from the
//!   type the declaration names: a string alias by its own name, with no
//!   elementary tag, and so a function's result of any alias; an alias of
//!   an elementary type of a variable by the elementary type, which the
//!   analyzer resolves the declaration to.

use ironplc_container::debug_section::{
    function_id, iec_type_tag, var_section, EnumDefEntry, StringLayoutEntry, VarNameEntry,
};
use ironplc_container::{FunctionId, VarIndex};
use ironplc_ir::lowered::{
    Encoding, IntWidth, Section, TemporalKind, TypeId, TypeKind, TypeTable, WideWidth,
};
use ironplc_ir::Id;

use crate::compile::StringVarInfo;

/// The debug entries the layout records, in the walk's order.
#[derive(Default)]
pub(crate) struct DebugVars {
    pub(super) var_names: Vec<VarNameEntry>,
    pub(super) string_layouts: Vec<StringLayoutEntry>,
}

/// The `ENUM_DEF` entries: one for each enumeration type of the lowered
/// program, its values' names in the order the type lists them, sorted by
/// type name so one source always gives the same bytes (REQ-EN-codegen-064).
pub(super) fn enum_definitions(types: &TypeTable) -> Vec<EnumDefEntry> {
    let mut entries: Vec<EnumDefEntry> = types
        .iter()
        .filter_map(|(ty, entry)| {
            let TypeKind::Enumeration { values, .. } = &entry.kind else {
                return None;
            };
            Some(EnumDefEntry {
                type_name: enumeration_name(types, ty),
                values: values
                    .iter()
                    .map(|value| value.name.to_string().to_uppercase())
                    .collect(),
            })
        })
        .collect();
    entries.sort_by(|a, b| a.type_name.cmp(&b.type_name));
    entries
}

impl DebugVars {
    /// Records the global or the program's variable `name` of the type
    /// `ty`, of the section `section`, in the slot `var_index`, which holds
    /// the string `string` when it is one.
    pub(super) fn declared(
        &mut self,
        types: &TypeTable,
        var_index: VarIndex,
        section: Option<Section>,
        name: &Id,
        ty: TypeId,
        string: Option<&StringVarInfo>,
    ) {
        let (iec_type_tag, type_name) = resolved(types, ty);
        self.push(
            var_index,
            function_id::GLOBAL_SCOPE,
            section,
            name,
            iec_type_tag,
            type_name,
        );
        if let Some(string) = string {
            self.string_layouts.push(StringLayoutEntry {
                var_index,
                data_offset: string.data_offset,
                max_length: string.max_length,
            });
        }
    }

    /// Records the compiler-provided global `name` of the type `ty`, in the
    /// slot `var_index`.
    pub(super) fn system(&mut self, types: &TypeTable, var_index: VarIndex, name: &Id, ty: TypeId) {
        let (iec_type_tag, type_name) = named(types, ty);
        self.push(
            var_index,
            function_id::GLOBAL_SCOPE,
            Some(Section::Global),
            name,
            iec_type_tag,
            type_name,
        );
    }

    /// Records the variable `name` of the function or function block
    /// `function_id`, of the section `section`, whose slot `var_index`
    /// holds a value of the type `ty`, or refers to one for a `VAR_IN_OUT`.
    pub(super) fn local(
        &mut self,
        types: &TypeTable,
        var_index: VarIndex,
        function_id: FunctionId,
        section: Option<Section>,
        name: &Id,
        ty: TypeId,
    ) {
        let (iec_type_tag, type_name) = match section {
            // The slot holds a reference to the caller's variable.
            Some(Section::InOut) => {
                let (_, type_name) = as_declared(types, ty);
                (iec_type_tag::OTHER, format!("REF_TO {type_name}"))
            }
            Some(
                Section::Global
                | Section::Input
                | Section::Output
                | Section::Local
                | Section::Temporary
                | Section::Instance
                | Section::Internal,
            )
            | None => as_declared(types, ty),
        };
        self.push(
            var_index,
            function_id,
            section,
            name,
            iec_type_tag,
            type_name,
        );
    }

    /// Records the result `name` of the function `function_id`, of the
    /// type `ty`, in the slot `var_index`. A result has no section of its
    /// own; it is shown with the outputs.
    pub(super) fn result(
        &mut self,
        types: &TypeTable,
        var_index: VarIndex,
        function_id: FunctionId,
        name: &Id,
        ty: TypeId,
    ) {
        let (iec_type_tag, type_name) = match types.kind(ty) {
            Some(TypeKind::String(shape)) if !is_alias(types, ty) => string(*shape),
            _ => named(types, ty),
        };
        self.push(
            var_index,
            function_id,
            Some(Section::Output),
            name,
            iec_type_tag,
            type_name,
        );
    }

    fn push(
        &mut self,
        var_index: VarIndex,
        function_id: FunctionId,
        section: Option<Section>,
        name: &Id,
        iec_type_tag: u8,
        type_name: String,
    ) {
        self.var_names.push(VarNameEntry {
            var_index,
            function_id,
            var_section: section_of(section),
            iec_type_tag,
            name: name.to_string(),
            type_name,
        });
    }
}

/// The debug section's code of `section`.
fn section_of(section: Option<Section>) -> u8 {
    match section {
        Some(Section::Global) => var_section::VAR_GLOBAL,
        Some(Section::Input) => var_section::VAR_INPUT,
        Some(Section::Output) => var_section::VAR_OUTPUT,
        Some(Section::InOut) => var_section::VAR_IN_OUT,
        Some(Section::Temporary) => var_section::VAR_TEMP,
        Some(Section::Local | Section::Instance | Section::Internal) | None => var_section::VAR,
    }
}

/// The debug type of a global or the program's variable of the type `ty`:
/// by the type it resolves to.
fn resolved(types: &TypeTable, ty: TypeId) -> (u8, String) {
    let Some(kind) = types.kind(ty) else {
        return (iec_type_tag::OTHER, String::new());
    };
    match kind {
        TypeKind::Structure { .. } => (iec_type_tag::STRUCT, upper_name(types, ty)),
        TypeKind::Subrange { base, .. } => {
            let name = match types.get(ty).and_then(|entry| entry.name.as_ref()) {
                Some(name) => name.to_string().to_uppercase(),
                None => elementary(types.kind(*base))
                    .map(|(_, name)| name.to_string())
                    .unwrap_or_default(),
            };
            (base_tag(types, *base), name)
        }
        TypeKind::String(shape) => string(*shape),
        TypeKind::FunctionBlock { .. } => (iec_type_tag::FB_INSTANCE, upper_name(types, ty)),
        TypeKind::Array { element, .. } => (iec_type_tag::ARRAY, array_name(types, ty, *element)),
        TypeKind::Reference { .. } => (iec_type_tag::OTHER, "REF_TO".into()),
        TypeKind::Enumeration { .. } => (iec_type_tag::DINT, enumeration_name(types, ty)),
        TypeKind::Bool
        | TypeKind::Integer { .. }
        | TypeKind::BitString { .. }
        | TypeKind::Real { .. }
        | TypeKind::Temporal { .. }
        | TypeKind::Program { .. } => match elementary(Some(kind)) {
            Some((tag, name)) => (tag, name.to_string()),
            None => (iec_type_tag::OTHER, upper_name(types, ty)),
        },
    }
}

/// The debug type of a function's or a function block's variable of the
/// type `ty`: by the type its declaration names.
fn as_declared(types: &TypeTable, ty: TypeId) -> (u8, String) {
    let Some(kind) = types.kind(ty) else {
        return (iec_type_tag::OTHER, String::new());
    };
    match kind {
        // A string declared through an alias is known by the alias.
        TypeKind::String(_) if is_alias(types, ty) => named(types, ty),
        TypeKind::String(shape) => string(*shape),
        TypeKind::Reference { .. } => (iec_type_tag::OTHER, "REF_TO".into()),
        TypeKind::Structure { .. } => (iec_type_tag::STRUCT, upper_name(types, ty)),
        TypeKind::FunctionBlock { .. } => (iec_type_tag::FB_INSTANCE, upper_name(types, ty)),
        TypeKind::Enumeration { .. } => (iec_type_tag::DINT, enumeration_name(types, ty)),
        TypeKind::Subrange { base, .. } => {
            let name = match types.get(ty).and_then(|entry| entry.name.as_ref()) {
                Some(name) => name.to_string().to_uppercase(),
                None => elementary(types.kind(*base))
                    .map(|(_, name)| name.to_string())
                    .unwrap_or_default(),
            };
            (base_tag(types, *base), name)
        }
        TypeKind::Array { .. } | TypeKind::Program { .. } => (iec_type_tag::OTHER, String::new()),
        // An alias of an elementary type is known by the elementary type,
        // as the analyzer resolves the declaration.
        TypeKind::Bool
        | TypeKind::Integer { .. }
        | TypeKind::BitString { .. }
        | TypeKind::Real { .. }
        | TypeKind::Temporal { .. } => match elementary(Some(kind)) {
            Some((tag, name)) => (tag, name.to_string()),
            None => named(types, ty),
        },
    }
}

/// Whether the string type `ty` is an alias: a type declared with a name of
/// its own, rather than `STRING` or `WSTRING`.
fn is_alias(types: &TypeTable, ty: TypeId) -> bool {
    let name = upper_name(types, ty);
    !(name.is_empty() || name.starts_with("STRING") || name.starts_with("WSTRING"))
}

/// The debug type of a variable whose declaration names the type `ty`:
/// that name, and the tag of the elementary type it is, or of a subrange's
/// base; any other type, an alias of an elementary one included, has none.
fn named(types: &TypeTable, ty: TypeId) -> (u8, String) {
    let name = upper_name(types, ty);
    let tag = match types.kind(ty) {
        Some(TypeKind::Subrange { base, .. }) => base_tag(types, *base),
        kind => match elementary(kind) {
            Some((tag, elementary)) if elementary == name => tag,
            Some(_) | None => iec_type_tag::OTHER,
        },
    };
    (tag, name)
}

/// The debug type of a string of the shape `shape`.
fn string(shape: ironplc_ir::lowered::StringShape) -> (u8, String) {
    match shape.encoding {
        Encoding::Narrow => (iec_type_tag::STRING, "STRING".into()),
        Encoding::Wide => (iec_type_tag::WSTRING, "WSTRING".into()),
    }
}

/// The tag of a subrange whose base type is `base`: the elementary type's.
fn base_tag(types: &TypeTable, base: TypeId) -> u8 {
    match types.kind(base) {
        Some(TypeKind::Subrange { base, .. }) => base_tag(types, *base),
        kind => elementary(kind).map_or(iec_type_tag::OTHER, |(tag, _)| tag),
    }
}

/// The debug name of an array of the type `ty`, whose elements are of the
/// type `element`: an array of structures by its own name, or else by its
/// elements' (`compile_array.rs`, `compile_array_struct.rs`).
fn array_name(types: &TypeTable, ty: TypeId, element: TypeId) -> String {
    match types.kind(element) {
        Some(TypeKind::Structure { .. }) => {
            match types.get(ty).and_then(|entry| entry.name.as_ref()) {
                Some(name) => name.to_string().to_uppercase(),
                None => format!("ARRAY OF {}", upper_name(types, element)),
            }
        }
        Some(TypeKind::Reference { target, .. }) => {
            let target_name = match types.get(*target).and_then(|entry| entry.name.as_ref()) {
                Some(name) => name.to_string().to_uppercase(),
                None => element_name(types, *target),
            };
            format!("ARRAY OF REF_TO {target_name}")
        }
        _ => format!("ARRAY OF {}", element_name(types, element)),
    }
}

/// The name of an array's element type `element`, as the elementary type
/// it resolves to.
fn element_name(types: &TypeTable, element: TypeId) -> String {
    match types.kind(element) {
        Some(TypeKind::String(shape)) => string(*shape).1,
        kind => elementary(kind)
            .map(|(_, name)| name.to_string())
            .unwrap_or_default(),
    }
}

/// The debug name of the enumeration `ty`: its declared name, or for an
/// anonymous enumeration (`e : (A, B)`) a name made from its id
/// (REQ-EN-codegen-092).
pub(super) fn enumeration_name(types: &TypeTable, ty: TypeId) -> String {
    match types.get(ty).and_then(|entry| entry.name.as_ref()) {
        Some(name) => name.to_string().to_uppercase(),
        None => crate::compile_enum::anonymous_name(ty.index()),
    }
}

/// The upper-case declared name of `ty`; empty when it has none.
fn upper_name(types: &TypeTable, ty: TypeId) -> String {
    types
        .get(ty)
        .and_then(|entry| entry.name.as_ref())
        .map(|name| name.to_string().to_uppercase())
        .unwrap_or_default()
}

/// The tag and the name of the elementary type of the kind `kind`; `None`
/// when it is not one.
fn elementary(kind: Option<&TypeKind>) -> Option<(u8, &'static str)> {
    use IntWidth::{B16, B32, B64, B8};
    Some(match kind? {
        TypeKind::Bool => (iec_type_tag::BOOL, "BOOL"),
        TypeKind::Integer { width, signed } => match (width, signed) {
            (B8, true) => (iec_type_tag::SINT, "SINT"),
            (B16, true) => (iec_type_tag::INT, "INT"),
            (B32, true) => (iec_type_tag::DINT, "DINT"),
            (B64, true) => (iec_type_tag::LINT, "LINT"),
            (B8, false) => (iec_type_tag::USINT, "USINT"),
            (B16, false) => (iec_type_tag::UINT, "UINT"),
            (B32, false) => (iec_type_tag::UDINT, "UDINT"),
            (B64, false) => (iec_type_tag::ULINT, "ULINT"),
        },
        TypeKind::BitString { width } => match width {
            B8 => (iec_type_tag::BYTE, "BYTE"),
            B16 => (iec_type_tag::WORD, "WORD"),
            B32 => (iec_type_tag::DWORD, "DWORD"),
            B64 => (iec_type_tag::LWORD, "LWORD"),
        },
        TypeKind::Real { width } => match width {
            WideWidth::B32 => (iec_type_tag::REAL, "REAL"),
            WideWidth::B64 => (iec_type_tag::LREAL, "LREAL"),
        },
        TypeKind::Temporal { kind, width } => match (kind, width) {
            (TemporalKind::Time, WideWidth::B32) => (iec_type_tag::TIME, "TIME"),
            (TemporalKind::Time, WideWidth::B64) => (iec_type_tag::LTIME, "LTIME"),
            (TemporalKind::Date, WideWidth::B32) => (iec_type_tag::DATE, "DATE"),
            (TemporalKind::Date, WideWidth::B64) => (iec_type_tag::LDATE, "LDATE"),
            (TemporalKind::TimeOfDay, WideWidth::B32) => (iec_type_tag::TIME_OF_DAY, "TIME_OF_DAY"),
            (TemporalKind::TimeOfDay, WideWidth::B64) => (iec_type_tag::LTOD, "LTIME_OF_DAY"),
            (TemporalKind::DateAndTime, WideWidth::B32) => {
                (iec_type_tag::DATE_AND_TIME, "DATE_AND_TIME")
            }
            (TemporalKind::DateAndTime, WideWidth::B64) => (iec_type_tag::LDT, "LDATE_AND_TIME"),
        },
        TypeKind::Enumeration { .. }
        | TypeKind::Subrange { .. }
        | TypeKind::String(_)
        | TypeKind::Array { .. }
        | TypeKind::Structure { .. }
        | TypeKind::FunctionBlock { .. }
        | TypeKind::Program { .. }
        | TypeKind::Reference { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elementary_when_long_temporal_then_long_tag_and_name() {
        let kind = TypeKind::Temporal {
            kind: TemporalKind::TimeOfDay,
            width: WideWidth::B64,
        };
        assert_eq!(
            elementary(Some(&kind)),
            Some((iec_type_tag::LTOD, "LTIME_OF_DAY"))
        );
        let kind = TypeKind::Temporal {
            kind: TemporalKind::DateAndTime,
            width: WideWidth::B32,
        };
        assert_eq!(
            elementary(Some(&kind)),
            Some((iec_type_tag::DATE_AND_TIME, "DATE_AND_TIME"))
        );
    }

    #[test]
    fn section_of_when_each_section_then_debug_code() {
        assert_eq!(section_of(Some(Section::Temporary)), var_section::VAR_TEMP);
        assert_eq!(section_of(Some(Section::InOut)), var_section::VAR_IN_OUT);
        assert_eq!(section_of(None), var_section::VAR);
    }
}
