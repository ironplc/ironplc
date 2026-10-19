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
use crate::Lowering;

/// A lowered type, `None` when lowering cannot express it yet, or a P9998
/// when the analyzer's type breaks an invariant of the type table.
pub(crate) type LoweredType = Result<Option<TypeId>, Diagnostic>;

impl Lowering<'_> {
    /// The lowered type of the analyzer type `id`, added to the table the
    /// first time it is asked for.
    pub(crate) fn lowered_type(&mut self, id: AnalyzerTypeId) -> LoweredType {
        if let Some(known) = self.type_ids.get(&id) {
            return Ok(*known);
        }
        // A type that reaches itself other than as a function block a
        // reference refers to (`reference_target_type`), such as a structure
        // holding a reference to its own type, is not lowered yet, so the
        // walk ends.
        if self.types_in_progress.contains_key(&id) {
            return Ok(None);
        }
        self.types_in_progress.insert(id, None);
        let lowered = self.lower_type(id);
        self.types_in_progress.remove(&id);
        let lowered = lowered?;
        self.type_ids.insert(id, lowered);
        Ok(lowered)
    }

    /// The lowered type a reference refers to, of the analyzer type `target`.
    /// A function block being lowered that a reference reaches again, as its
    /// own field `next : REF_TO Node` does, is declared then, and defined at
    /// that id once its fields are lowered (REQ-LOW-lowering-195,
    /// REQ-LOW-ir-194).
    fn reference_target_type(&mut self, target: AnalyzerTypeId) -> LoweredType {
        let Some(declared) = self.types_in_progress.get(&target).copied() else {
            return self.lowered_type(target);
        };
        if let Some(declared) = declared {
            return Ok(Some(declared));
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
            return Ok(None);
        }
        let declared = self.program.declare_type();
        self.types_in_progress.insert(target, Some(declared));
        Ok(Some(declared))
    }

    fn lower_type(&mut self, id: AnalyzerTypeId) -> LoweredType {
        let types = self.analysis.context().types();
        let Some(attributes) = types.get_by_id(id) else {
            return Ok(None);
        };
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
            SemanticType::Real { size } => match wide_width(size) {
                Some(width) => TypeKind::Real { width },
                None => return Ok(None),
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
                    Some(len) => match u32::try_from(*len) {
                        Ok(capacity) => capacity,
                        Err(_) => return Ok(None),
                    },
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
                    let Some(underlying) =
                        self.elementary(underlying_type.as_deref().unwrap_or(&int))?
                    else {
                        return Ok(None);
                    };
                    underlying
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
            } => {
                let Some(base) = self.elementary(base_type)? else {
                    return Ok(None);
                };
                TypeKind::Subrange {
                    base,
                    min: *min_value,
                    max: *max_value,
                }
            }
            SemanticType::Array { dimensions, .. } => {
                let Some(element) = self.analysis.context().types().element_type(id) else {
                    return Ok(None);
                };
                let Some(element) = self.lowered_type(element)? else {
                    return Ok(None);
                };
                TypeKind::Array {
                    element,
                    dimensions: dimensions.iter().map(|d| (d.lower, d.upper)).collect(),
                }
            }
            SemanticType::Structure { fields } => {
                let Some(fields) = self.fields(id, fields.iter().map(|f| (&f.name, None)))? else {
                    return Ok(None);
                };
                TypeKind::Structure { fields }
            }
            SemanticType::FunctionBlock { name } => {
                let Some(fields) = self.block_fields(id)? else {
                    return Ok(None);
                };
                let base = self.base(id, &fields)?;
                TypeKind::FunctionBlock {
                    fields,
                    body: self.pou(name, &span),
                    base,
                }
            }
            // An interface value has no run-time representation yet
            // (ADR-0041 Phase 2), so there is nothing to lower.
            SemanticType::Interface { .. } => return Ok(None),
            SemanticType::Reference { .. } => {
                let Some(target) = self.analysis.context().types().referenced_type(id) else {
                    return Ok(None);
                };
                let Some(target) = self.reference_target_type(target)? else {
                    return Ok(None);
                };
                TypeKind::Reference {
                    target,
                    nullability: Nullability::Nullable,
                }
            }
        };
        let entry = TypeEntry { name, kind };
        match self.types_in_progress.get(&id).copied().flatten() {
            Some(declared) => self
                .program
                .define_type(declared, entry, &span)
                .map(|()| Some(declared)),
            None => self.program.push_type(entry, &span).map(Some),
        }
    }

    fn temporal(
        &mut self,
        kind: TemporalKind,
        size: &ByteSized,
        name: Option<DebugName>,
        span: &SourceSpan,
    ) -> LoweredType {
        let Some(width) = wide_width(size) else {
            return Ok(None);
        };
        self.program
            .push_type(
                TypeEntry {
                    name,
                    kind: TypeKind::Temporal { kind, width },
                },
                span,
            )
            .map(Some)
    }

    /// The fields of the record type `record`, each with its lowered type;
    /// `None` when lowering cannot express one of them yet.
    fn fields<'n>(
        &mut self,
        record: AnalyzerTypeId,
        fields: impl Iterator<Item = (&'n ironplc_dsl::core::Id, Option<Section>)>,
    ) -> Result<Option<Vec<Field>>, Diagnostic> {
        let fields: Vec<_> = fields.collect();
        let mut lowered = Vec::with_capacity(fields.len());
        for (name, section) in fields {
            let Some(field_type) = self.analysis.context().types().field_type(record, name) else {
                return Ok(None);
            };
            let Some(ty) = self.lowered_type(field_type)? else {
                return Ok(None);
            };
            lowered.push(Field::new(&name.to_string(), name.span(), section, ty));
        }
        Ok(Some(lowered))
    }

    /// The fields of the function block `block`, a block the library
    /// declares and a standard block alike: one for each variable the type
    /// environment records for it, in the order the block declares them, as
    /// a program's are (REQ-LOW-lowering-170). `None` when lowering cannot
    /// express the type of one of them yet.
    fn block_fields(&mut self, block: AnalyzerTypeId) -> Result<Option<Vec<Field>>, Diagnostic> {
        let Some(recorded) = self.analysis.context().types().block_fields(block) else {
            return Ok(None);
        };
        let mut fields = Vec::with_capacity(recorded.len());
        for field in recorded {
            let Some(field_type) = field.field_type else {
                return Ok(None);
            };
            let Some(ty) = self.lowered_type(field_type)? else {
                return Ok(None);
            };
            let declared = Field::new(
                &field.name.to_string(),
                field.name.span(),
                Some(section(&field.var_type)),
                ty,
            );
            let location = field.address.as_ref().map(direct_address);
            fields.push(declared.declared(location, retention(&field.qualifier)));
        }
        Ok(Some(fields))
    }

    /// The lowered type of the nearest block of the `EXTENDS` chain above the
    /// function block `block` whose fields `fields`, the lowered block's,
    /// begin with (REQ-LOW-lowering-177, REQ-LOW-ir-176). A block's lowered
    /// type has only the fields it declares itself, so that is the nearest
    /// block above it that declares none. The blocks in between, which
    /// declare fields, are not its base in the lowered program, and a call of
    /// a method one of them declares is not lowered. `None` when no block
    /// above it is one.
    fn base(&mut self, block: AnalyzerTypeId, fields: &[Field]) -> LoweredType {
        let ancestors: Vec<AnalyzerTypeId> = self
            .analysis
            .context()
            .types()
            .inheritance_chain(block)
            .skip(1)
            .collect();
        for ancestor in ancestors {
            let Some(base) = self.lowered_type(ancestor)? else {
                continue;
            };
            if self.program.types().can_extend(fields, base) {
                return Ok(Some(base));
            }
        }
        Ok(None)
    }

    /// The lowered type of an elementary type, such as a subrange's base.
    fn elementary(&mut self, representation: &SemanticType) -> LoweredType {
        let types = self.analysis.context().types();
        let Some(name): Option<TypeName> = types.elementary_type_name_for(representation) else {
            return Ok(None);
        };
        let Some(id) = types.id_of(&name) else {
            return Ok(None);
        };
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
