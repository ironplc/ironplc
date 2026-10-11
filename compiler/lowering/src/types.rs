//! The lowered program's types, built from the analyzer's.
//!
//! The analyzer's type environment describes each type with its layout
//! (field offsets, slot counts); the lowered type table keeps only what the
//! language defines (REQ-LOW-lowering-027) and numbers its types itself.

use ironplc_analyzer::semantic_type::{ByteSized, FunctionBlockVarType, SemanticType};
use ironplc_container::{CharWidth, DEFAULT_STRING_MAX_LENGTH};
use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::Located;
use ironplc_dsl::type_id::TypeId as AnalyzerTypeId;
use ironplc_ir::execution::DebugName;
use ironplc_ir::lowered::{
    Encoding, Field, Section, StringShape, TemporalKind, TypeEntry, TypeId, TypeKind,
};

use crate::Lowering;

impl Lowering<'_> {
    /// The lowered type of the analyzer type `id`, added to the table the
    /// first time it is asked for; `None` for a type lowering cannot express
    /// yet.
    pub(crate) fn lowered_type(&mut self, id: AnalyzerTypeId) -> Option<TypeId> {
        if let Some(known) = self.type_ids.get(&id) {
            return *known;
        }
        // A type that reaches itself, such as a structure holding a reference
        // to its own type, is not lowered yet: mark it unsupported while its
        // parts are lowered, so the walk ends.
        self.type_ids.insert(id, None);
        let lowered = self.lower_type(id);
        self.type_ids.insert(id, lowered);
        lowered
    }

    fn lower_type(&mut self, id: AnalyzerTypeId) -> Option<TypeId> {
        let types = self.analysis.context().types();
        let representation = types.get_by_id(id)?.representation.clone();
        let name = types
            .name_of(id)
            .map(|name| DebugName::new(name.to_string(), name.span()));
        let kind = match &representation {
            SemanticType::Bool => TypeKind::Bool,
            SemanticType::Int { size } => TypeKind::Integer {
                bits: bits(size),
                signed: true,
            },
            SemanticType::UInt { size } => TypeKind::Integer {
                bits: bits(size),
                signed: false,
            },
            SemanticType::Bytes { size } => TypeKind::BitString { bits: bits(size) },
            SemanticType::Real { size } => TypeKind::Real { bits: bits(size) },
            SemanticType::Time { size } => temporal(TemporalKind::Time, size),
            SemanticType::Date { size } => temporal(TemporalKind::Date, size),
            SemanticType::TimeOfDay { size } => temporal(TemporalKind::TimeOfDay, size),
            SemanticType::DateAndTime { size } => temporal(TemporalKind::DateAndTime, size),
            SemanticType::String {
                max_len,
                char_width,
            } => TypeKind::String(StringShape {
                encoding: match char_width {
                    CharWidth::Narrow => Encoding::Narrow,
                    CharWidth::Wide => Encoding::Wide,
                },
                capacity: match max_len {
                    Some(len) => u32::try_from(*len).ok()?,
                    None => u32::from(DEFAULT_STRING_MAX_LENGTH),
                },
            }),
            SemanticType::Enumeration { members, .. } => TypeKind::Enumeration {
                values: members
                    .iter()
                    .map(|member| DebugName::new(member.name.to_string(), member.name.span()))
                    .collect(),
            },
            SemanticType::Subrange {
                base_type,
                min_value,
                max_value,
            } => TypeKind::Subrange {
                base: self.elementary(base_type)?,
                min: *min_value,
                max: *max_value,
            },
            SemanticType::Array { dimensions, .. } => {
                let element = self.analysis.context().types().element_type(id)?;
                TypeKind::Array {
                    element: self.lowered_type(element)?,
                    dimensions: dimensions.iter().map(|d| (d.lower, d.upper)).collect(),
                }
            }
            SemanticType::Structure { fields } => TypeKind::Structure {
                fields: self.fields(id, fields.iter().map(|f| (&f.name, None)))?,
            },
            SemanticType::FunctionBlock { name, fields } => TypeKind::FunctionBlock {
                fields: self.fields(
                    id,
                    fields
                        .iter()
                        .map(|f| (&f.name, f.var_type.as_ref().map(section))),
                )?,
                body: self.pou(name),
            },
            SemanticType::Reference { .. } => TypeKind::Reference {
                target: self.lowered_type(self.analysis.context().types().referenced_type(id)?)?,
                nullability: ironplc_ir::lowered::Nullability::Nullable,
            },
            SemanticType::Function { .. } => return None,
        };
        Some(self.types.push(TypeEntry { name, kind }))
    }

    /// The fields of the record type `record`, each with its lowered type.
    fn fields<'n>(
        &mut self,
        record: AnalyzerTypeId,
        fields: impl Iterator<Item = (&'n ironplc_dsl::core::Id, Option<Section>)>,
    ) -> Option<Vec<Field>> {
        let fields: Vec<_> = fields.collect();
        let mut lowered = Vec::with_capacity(fields.len());
        for (name, section) in fields {
            let field_type = self.analysis.context().types().field_type(record, name)?;
            let ty = self.lowered_type(field_type)?;
            lowered.push(Field::new(&name.to_string(), name.span(), section, ty));
        }
        Some(lowered)
    }

    /// The lowered type of an elementary type, such as a subrange's base.
    fn elementary(&mut self, representation: &SemanticType) -> Option<TypeId> {
        let types = self.analysis.context().types();
        let name: TypeName = types.elementary_type_name_for(representation)?;
        let id = types.id_of(&name)?;
        self.lowered_type(id)
    }
}

fn bits(size: &ByteSized) -> u8 {
    match size {
        ByteSized::B8 => 8,
        ByteSized::B16 => 16,
        ByteSized::B32 => 32,
        ByteSized::B64 => 64,
    }
}

fn temporal(kind: TemporalKind, size: &ByteSized) -> TypeKind {
    TypeKind::Temporal {
        kind,
        bits: bits(size),
    }
}

pub(crate) fn section(var_type: &FunctionBlockVarType) -> Section {
    match var_type {
        FunctionBlockVarType::Input => Section::Input,
        FunctionBlockVarType::Output => Section::Output,
        FunctionBlockVarType::InOut => Section::InOut,
        FunctionBlockVarType::Internal => Section::Local,
    }
}
