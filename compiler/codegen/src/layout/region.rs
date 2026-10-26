//! The data region the lowered declarations reserve
//! (`specs/design/lowered-program.md`, "`CompileContext`").
//!
//! A variable that does not fit in its slot takes a run of the data region,
//! in the order the layout visits the declarations: a string, a structure,
//! an array, or the fields of a function block instance, then those of each
//! instance its fields hold, to any depth ([`crate::held_instances`]).

use std::collections::HashMap;

use ironplc_container::CharWidth;
use ironplc_ir::lowered::Program;
use ironplc_ir::lowered::StandardFunctionBlock;
use ironplc_ir::lowered::{Encoding, PouId, StringShape, TypeId, TypeKind, TypeTable};

use super::declarations::copy_slots;
use crate::compile::{string_region_size, FbFieldIndex, StringVarInfo};
use crate::compile_call::standard_fb_layout;
use crate::held_instances::HeldInstance;

/// The run of the data region a variable takes.
pub(super) struct Run {
    /// Where it starts.
    pub(super) start: u32,
    /// The instances the fields of a function block instance hold, and
    /// theirs, in the order they are reserved.
    pub(super) held: Vec<HeldInstance>,
}

/// The runs of the data region reserved so far.
pub(super) struct Region<'a> {
    lowering: &'a Program,
    /// The reachable user function blocks, which an instance of one is laid
    /// out for.
    blocks: &'a [PouId],
    /// The standard function block each lowered POU without a body is.
    standard_blocks: &'a HashMap<PouId, StandardFunctionBlock>,
    /// Where the next run starts.
    next: u32,
}

impl<'a> Region<'a> {
    /// An empty data region, for the declarations of `lowering`, whose
    /// reachable user function blocks are `blocks`.
    pub(super) fn new(
        lowering: &'a Program,
        blocks: &'a [PouId],
        standard_blocks: &'a HashMap<PouId, StandardFunctionBlock>,
    ) -> Self {
        Self {
            lowering,
            blocks,
            standard_blocks,
            next: 0,
        }
    }

    /// Where the run reserved next would start: the size of the region so
    /// far.
    pub(super) fn end(&self) -> u32 {
        self.next
    }

    /// Reserves the run of a global's or the program's variable of the type
    /// `ty`, and returns it, when it has one; `None` when the region
    /// overflows.
    pub(super) fn variable(&mut self, ty: TypeId) -> Option<Option<Run>> {
        let types = self.types();
        let start = self.next;
        let mut held = Vec::new();
        let reserved = match types.kind(ty)? {
            TypeKind::String(shape) => {
                self.string(*shape)?;
                true
            }
            TypeKind::Structure { .. } => {
                self.structure(ty)?;
                true
            }
            TypeKind::Array { element, .. } => {
                let elements = elements(types, ty)?;
                let element_bytes = match types.kind(*element)? {
                    TypeKind::Structure { .. } => slots(types, *element)?.checked_mul(8)?,
                    TypeKind::String(shape) => {
                        let (max_length, char_width) = string_shape(*shape)?;
                        string_region_size(max_length, char_width)
                    }
                    TypeKind::Bool
                    | TypeKind::Integer { .. }
                    | TypeKind::BitString { .. }
                    | TypeKind::Real { .. }
                    | TypeKind::Temporal { .. }
                    | TypeKind::Enumeration { .. }
                    | TypeKind::Subrange { .. }
                    | TypeKind::Array { .. }
                    | TypeKind::FunctionBlock { .. }
                    | TypeKind::Program { .. }
                    | TypeKind::Reference { .. } => 8,
                };
                self.reserve(elements.checked_mul(element_bytes)?)?;
                true
            }
            TypeKind::FunctionBlock { .. } => self.instance(ty, &mut held)?.is_some(),
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::Program { .. }
            | TypeKind::Reference { .. } => false,
        };
        Some(reserved.then_some(Run { start, held }))
    }

    /// Reserves the run of a string of the shape `shape`, and returns where
    /// it is.
    pub(super) fn string(&mut self, shape: StringShape) -> Option<StringVarInfo> {
        let (max_length, char_width) = string_shape(shape)?;
        let data_offset = self.reserve(string_region_size(max_length, char_width))?;
        string_at(data_offset, shape)
    }

    /// Reserves the run of a structure of the type `ty`, and returns where
    /// it starts.
    pub(super) fn structure(&mut self, ty: TypeId) -> Option<u32> {
        let bytes = slots(self.types(), ty)?.checked_mul(8)?;
        self.reserve(bytes)
    }

    /// Reserves the fields of an instance of the function block type `ty`,
    /// then those of each instance its fields hold, which it records in
    /// `held`, and returns where its fields start. A block that is neither
    /// reachable nor standard has no layout, and reserves nothing.
    fn instance(&mut self, ty: TypeId, held: &mut Vec<HeldInstance>) -> Option<Option<u32>> {
        let TypeKind::FunctionBlock { body, .. } = self.types().kind(ty)? else {
            return Some(None);
        };
        let body = *body;
        if self.blocks.contains(&body) {
            let fields: Vec<_> = copy_slots(self.lowering, body)?
                .into_iter()
                .map(|(_, field)| field)
                .collect();
            let start = self.reserve(u32::try_from(fields.len()).ok()?.checked_mul(8)?)?;
            let holders: Vec<(FbFieldIndex, TypeId)> = fields
                .iter()
                .enumerate()
                .filter(|(_, field)| {
                    matches!(
                        self.types().kind(field.ty),
                        Some(TypeKind::FunctionBlock { .. })
                    )
                })
                .map(|(index, field)| Some((FbFieldIndex::try_from(index).ok()?, field.ty)))
                .collect::<Option<_>>()?;
            for (index, field) in holders {
                let next = held.len();
                if let Some(data_offset) = self.instance(field, held)? {
                    // An instance precedes those its own fields hold.
                    held.insert(next, HeldInstance::new(start, index, data_offset));
                }
            }
            return Some(Some(start));
        }
        let Some(block) = self.standard_blocks.get(&body) else {
            return Some(None);
        };
        let (_, fields, _) = standard_fb_layout(*block);
        self.reserve(u32::try_from(fields).ok()?.checked_mul(8)?)
            .map(Some)
    }

    /// Reserves `bytes` and returns where they start; `None` when the
    /// region overflows.
    fn reserve(&mut self, bytes: u32) -> Option<u32> {
        let start = self.next;
        self.next = self.next.checked_add(bytes)?;
        Some(start)
    }

    /// The declarations the region is reserved for.
    pub(super) fn lowering(&self) -> &'a Program {
        self.lowering
    }

    fn types(&self) -> &'a TypeTable {
        self.lowering.types()
    }
}

/// Where a string of the shape `shape` whose run starts at `data_offset`
/// is.
pub(super) fn string_at(data_offset: u32, shape: StringShape) -> Option<StringVarInfo> {
    let (max_length, char_width) = string_shape(shape)?;
    Some(StringVarInfo {
        data_offset,
        max_length,
        char_width,
    })
}

/// The capacity and width of the strings of the shape `shape`.
pub(crate) fn string_shape(shape: StringShape) -> Option<(u16, CharWidth)> {
    let char_width = match shape.encoding {
        Encoding::Narrow => CharWidth::Narrow,
        Encoding::Wide => CharWidth::Wide,
    };
    Some((u16::try_from(shape.capacity).ok()?, char_width))
}

/// How many elements an array of the type `ty` has, across its dimensions.
pub(crate) fn elements(types: &TypeTable, ty: TypeId) -> Option<u32> {
    let TypeKind::Array { dimensions, .. } = types.kind(ty)? else {
        return None;
    };
    dimensions.iter().try_fold(1_u32, |total, (lower, upper)| {
        let size = (i64::from(*upper) - i64::from(*lower) + 1).max(0);
        total.checked_mul(u32::try_from(size).ok()?)
    })
}

/// How many 8-byte slots a value of the type `ty` takes in the data region,
/// as a structure's field or an array's element (`SemanticType::slot_count`):
/// one for a scalar or a reference, the sum of its fields' for a structure,
/// its elements' for an array, and enough for the header and the
/// characters of a string. A function block or a program has none.
pub(crate) fn slots(types: &TypeTable, ty: TypeId) -> Option<u32> {
    match types.kind(ty)? {
        TypeKind::Bool
        | TypeKind::Integer { .. }
        | TypeKind::BitString { .. }
        | TypeKind::Real { .. }
        | TypeKind::Temporal { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Subrange { .. }
        | TypeKind::Reference { .. } => Some(1),
        TypeKind::Structure { fields } => fields.iter().try_fold(0_u32, |total, field| {
            total.checked_add(slots(types, field.ty)?)
        }),
        TypeKind::Array { element, .. } => {
            elements(types, ty)?.checked_mul(slots(types, *element)?)
        }
        TypeKind::String(shape) => {
            let (max_length, char_width) = string_shape(*shape)?;
            Some(string_region_size(max_length, char_width).div_ceil(8))
        }
        TypeKind::FunctionBlock { .. } | TypeKind::Program { .. } => None,
    }
}
