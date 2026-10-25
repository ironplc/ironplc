//! The lowered program's types, built from the analyzer's.
//!
//! The analyzer's type environment describes each type with its layout
//! (field offsets, slot counts); the lowered type table keeps only what the
//! language defines (REQ-LOW-lowering-027) and numbers its types itself.

use ironplc_analyzer::semantic_type::{ByteSized, FunctionBlockVarType, SemanticType};
use ironplc_container::{CharWidth, DEFAULT_STRING_MAX_LENGTH};
use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::type_id::TypeId as AnalyzerTypeId;
use ironplc_ir::execution::DebugName;
use ironplc_ir::lowered::{
    Encoding, EnumerationValue, Field, IntWidth, Nullability, Section, StringShape, TemporalKind,
    TypeEntry, TypeId, TypeKind, WideWidth,
};

use crate::variables::{direct_address, retention};
use crate::{Lowered, Lowering, Unlowered};

impl Lowering<'_> {
    /// The lowered type of the analyzer type `id`, added to the table the
    /// first time it is asked for. A type the analyzer's environment breaks
    /// an invariant of the type table with is a P9998.
    pub(crate) fn lowered_type(&mut self, id: AnalyzerTypeId) -> Lowered<TypeId> {
        if let Some(known) = self.type_ids.get(&id) {
            return known.clone();
        }
        // A type that reaches itself other than as a function block a
        // reference refers to (`reference_target_type`), such as a structure
        // holding a reference to its own type, is not lowered yet, so the
        // walk ends.
        if self.types_in_progress.contains_key(&id) {
            return Err(Unlowered::NotYet);
        }
        self.types_in_progress.insert(id, None);
        let lowered = self.lower_type(id);
        self.types_in_progress.remove(&id);
        self.type_ids.insert(id, lowered.clone());
        lowered
    }

    /// The lowered type a reference refers to, of the analyzer type `target`.
    /// A function block being lowered that a reference reaches again, as its
    /// own field `next : REF_TO Node` does, is declared then, and defined at
    /// that id once its fields are lowered (REQ-LOW-lowering-195,
    /// REQ-LOW-ir-194). Any other type a reference reaches while it is being
    /// lowered is not lowered yet.
    fn reference_target_type(&mut self, target: AnalyzerTypeId) -> Lowered<TypeId> {
        let Some(declared) = self.types_in_progress.get(&target).copied() else {
            return self.lowered_type(target);
        };
        if let Some(declared) = declared {
            return Ok(declared);
        }
        let is_block = matches!(
            self.analysis
                .context()
                .types()
                .get_by_id(target)
                .map(|attributes| &attributes.representation),
            Some(SemanticType::FunctionBlock { .. })
        );
        if !is_block {
            return Err(Unlowered::NotYet);
        }
        let declared = self.program.declare_type();
        self.types_in_progress.insert(target, Some(declared));
        Ok(declared)
    }

    fn lower_type(&mut self, id: AnalyzerTypeId) -> Lowered<TypeId> {
        let types = self.analysis.context().types();
        let attributes = types.get_by_id(id).ok_or(Unlowered::NotYet)?;
        let representation = attributes.representation.clone();
        let (name, span) = match types.name_of(id) {
            Some(name) => (
                Some(DebugName::new(name.to_string(), name.span())),
                name.span(),
            ),
            None => (None, SourceSpan::default()),
        };
        let kind = match &representation {
            SemanticType::Bool => TypeKind::Bool,
            SemanticType::Int { size } => TypeKind::Integer {
                width: int_width(size),
                signed: true,
            },
            SemanticType::UInt { size } => TypeKind::Integer {
                width: int_width(size),
                signed: false,
            },
            SemanticType::Bytes { size } => TypeKind::BitString {
                width: int_width(size),
            },
            SemanticType::Real { size } => TypeKind::Real {
                width: wide_width(size).ok_or(Unlowered::NotYet)?,
            },
            SemanticType::Time { size } => {
                return self.temporal(TemporalKind::Time, size, name, &span)
            }
            SemanticType::Date { size } => {
                return self.temporal(TemporalKind::Date, size, name, &span)
            }
            SemanticType::TimeOfDay { size } => {
                return self.temporal(TemporalKind::TimeOfDay, size, name, &span)
            }
            SemanticType::DateAndTime { size } => {
                return self.temporal(TemporalKind::DateAndTime, size, name, &span)
            }
            SemanticType::String {
                max_len,
                char_width,
            } => {
                let capacity = match max_len {
                    Some(len) => u32::try_from(*len).map_err(|_| Unlowered::NotYet)?,
                    None => u32::from(DEFAULT_STRING_MAX_LENGTH),
                };
                TypeKind::String(StringShape {
                    encoding: match char_width {
                        CharWidth::Narrow => Encoding::Narrow,
                        CharWidth::Wide => Encoding::Wide,
                    },
                    capacity,
                })
            }
            SemanticType::Enumeration {
                members,
                underlying_type,
            } => TypeKind::Enumeration {
                // The type the declaration names, and `INT` when it names
                // none, as in CODESYS and TwinCAT (REQ-LOW-lowering-190).
                underlying: {
                    let int = SemanticType::Int {
                        size: ByteSized::B16,
                    };
                    self.elementary(underlying_type.as_deref().unwrap_or(&int))?
                },
                values: members
                    .iter()
                    .map(|member| {
                        // Analysis reports an ordinal outside `DINT`
                        // (P2044), so each one fits.
                        let ordinal = i32::try_from(member.ordinal).map_err(|_| {
                            Diagnostic::internal_error_at(Label::span(
                                member.name.span(),
                                format!(
                                    "The ordinal {} of {} does not fit a DINT",
                                    member.ordinal, member.name
                                ),
                            ))
                        })?;
                        Ok(EnumerationValue {
                            name: DebugName::new(member.name.to_string(), member.name.span()),
                            ordinal,
                        })
                    })
                    .collect::<Result<_, Diagnostic>>()?,
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
                let element = self
                    .analysis
                    .context()
                    .types()
                    .element_type(id)
                    .ok_or(Unlowered::NotYet)?;
                TypeKind::Array {
                    element: self.lowered_type(element)?,
                    dimensions: dimensions.iter().map(|d| (d.lower, d.upper)).collect(),
                }
            }
            SemanticType::Structure { fields } => TypeKind::Structure {
                fields: self.fields(id, fields.iter().map(|f| (&f.name, None)))?,
            },
            SemanticType::FunctionBlock { name } => {
                let fields = self.block_fields(id)?;
                let base = self.base(id, &fields)?;
                TypeKind::FunctionBlock {
                    fields,
                    body: self.pou(name, &span),
                    base,
                }
            }
            // An interface value has no run-time representation yet
            // (ADR-0041 Phase 2), so there is nothing to lower.
            SemanticType::Interface { .. } => return Err(Unlowered::NotYet),
            SemanticType::Reference { .. } => {
                let target = self
                    .analysis
                    .context()
                    .types()
                    .referenced_type(id)
                    .ok_or(Unlowered::NotYet)?;
                TypeKind::Reference {
                    target: self.reference_target_type(target)?,
                    nullability: Nullability::Nullable,
                }
            }
        };
        let entry = TypeEntry { name, kind };
        match self.types_in_progress.get(&id).copied().flatten() {
            Some(declared) => {
                self.program.define_type(declared, entry, &span)?;
                Ok(declared)
            }
            None => Ok(self.program.push_type(entry, &span)?),
        }
    }

    fn temporal(
        &mut self,
        kind: TemporalKind,
        size: &ByteSized,
        name: Option<DebugName>,
        span: &SourceSpan,
    ) -> Lowered<TypeId> {
        let width = wide_width(size).ok_or(Unlowered::NotYet)?;
        Ok(self.program.push_type(
            TypeEntry {
                name,
                kind: TypeKind::Temporal { kind, width },
            },
            span,
        )?)
    }

    /// The fields of the record type `record`, each with its lowered type.
    fn fields<'n>(
        &mut self,
        record: AnalyzerTypeId,
        fields: impl Iterator<Item = (&'n ironplc_dsl::core::Id, Option<Section>)>,
    ) -> Lowered<Vec<Field>> {
        let fields: Vec<_> = fields.collect();
        let mut lowered = Vec::with_capacity(fields.len());
        for (name, section) in fields {
            let field_type = self
                .analysis
                .context()
                .types()
                .field_type(record, name)
                .ok_or(Unlowered::NotYet)?;
            let ty = self.lowered_type(field_type)?;
            lowered.push(Field::new(&name.to_string(), name.span(), section, ty));
        }
        Ok(lowered)
    }

    /// The fields of the function block `block`, a block the library
    /// declares and a standard block alike: one for each variable the type
    /// environment records for it, in the order the block declares them, as
    /// a program's are (REQ-LOW-lowering-170).
    fn block_fields(&mut self, block: AnalyzerTypeId) -> Lowered<Vec<Field>> {
        let recorded = self
            .analysis
            .context()
            .types()
            .block_fields(block)
            .ok_or(Unlowered::NotYet)?;
        let mut fields = Vec::with_capacity(recorded.len());
        for field in recorded {
            let field_type = field.field_type.ok_or(Unlowered::NotYet)?;
            let ty = self.lowered_type(field_type)?;
            let declared = Field::new(
                &field.name.to_string(),
                field.name.span(),
                Some(section(&field.var_type)),
                ty,
            );
            let location = field.address.as_ref().map(direct_address);
            fields.push(declared.declared(location, retention(&field.qualifier)));
        }
        Ok(fields)
    }

    /// The lowered type of the nearest block of the `EXTENDS` chain above the
    /// function block `block` whose fields `fields`, the lowered block's,
    /// begin with (REQ-LOW-lowering-177, REQ-LOW-ir-176). A block's lowered
    /// type has only the fields it declares itself, so that is the nearest
    /// block above it that declares none. The blocks in between, which
    /// declare fields, are not its base in the lowered program, and a call of
    /// a method one of them declares is not lowered. `None` when no block
    /// above it is one.
    fn base(&mut self, block: AnalyzerTypeId, fields: &[Field]) -> Lowered<Option<TypeId>> {
        let ancestors: Vec<AnalyzerTypeId> = self
            .analysis
            .context()
            .types()
            .inheritance_chain(block)
            .skip(1)
            .collect();
        for ancestor in ancestors {
            // A block lowering does not express yet is no base.
            let base = match self.lowered_type(ancestor) {
                Ok(base) => base,
                Err(Unlowered::NotYet) => continue,
                Err(Unlowered::Broken(diagnostic)) => return Err(Unlowered::Broken(diagnostic)),
            };
            if self.program.types().can_extend(fields, base) {
                return Ok(Some(base));
            }
        }
        Ok(None)
    }

    /// The lowered type of an elementary type, such as a subrange's base.
    fn elementary(&mut self, representation: &SemanticType) -> Lowered<TypeId> {
        let types = self.analysis.context().types();
        let name: TypeName = types
            .elementary_type_name_for(representation)
            .ok_or(Unlowered::NotYet)?;
        let id = types.id_of(&name).ok_or(Unlowered::NotYet)?;
        self.lowered_type(id)
    }
}

fn int_width(size: &ByteSized) -> IntWidth {
    match size {
        ByteSized::B8 => IntWidth::B8,
        ByteSized::B16 => IntWidth::B16,
        ByteSized::B32 => IntWidth::B32,
        ByteSized::B64 => IntWidth::B64,
    }
}

/// The width of a real or a time: 32 or 64 bits, and `None` for any other.
fn wide_width(size: &ByteSized) -> Option<WideWidth> {
    match size {
        ByteSized::B32 => Some(WideWidth::B32),
        ByteSized::B64 => Some(WideWidth::B64),
        ByteSized::B8 | ByteSized::B16 => None,
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
