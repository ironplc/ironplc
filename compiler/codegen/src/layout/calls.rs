//! How each function block instance is reached, and what a call of each user
//! function, function block and method needs, from the lowered declarations
//! (`declarations.rs`).

use std::collections::HashMap;

use ironplc_container::{FbTypeId, FunctionId, VarIndex};
use ironplc_ir::lowered::Program;
use ironplc_ir::lowered::StandardFunctionBlock;
use ironplc_ir::lowered::{FieldIdx, PouId, Section, TypeId, TypeKind, VarId};

use super::declarations::{copy_slots, instance_type};
use super::region::Run;
use crate::body::slot_type;
use crate::compile::{
    FbFieldIndex, FbInstanceInfo, ParamPassing, StringParamInfo, StringVarInfo, UserFbTypeInfo,
    UserMethodInfo, VarTypeInfo, DEFAULT_VAR_TYPE_INFO, FIRST_USER_FB_TYPE_ID,
};
use crate::compile_call::standard_fb_layout;
use crate::held_instances::HeldField;

/// The function blocks an instance can be of.
pub(super) struct Blocks<'a> {
    lowering: &'a Program,
    /// The reachable user function blocks, which take the type ids from
    /// [`FIRST_USER_FB_TYPE_ID`] in this order.
    user: &'a [PouId],
    /// The standard function block each lowered POU without a body is.
    standard: &'a HashMap<PouId, StandardFunctionBlock>,
}

/// The fields of a function block, by the fields of its lowered type.
struct Fields {
    type_id: FbTypeId,
    /// Each field's index.
    indices: HashMap<FieldIdx, FbFieldIndex>,
    /// The type each field is operated and stored at.
    types: HashMap<FieldIdx, VarTypeInfo>,
}

impl<'a> Blocks<'a> {
    /// The function blocks of `lowering`: its reachable user blocks `user`,
    /// and the standard ones `standard` holds.
    pub(super) fn new(
        lowering: &'a Program,
        user: &'a [PouId],
        standard: &'a HashMap<PouId, StandardFunctionBlock>,
    ) -> Self {
        Self {
            lowering,
            user,
            standard,
        }
    }

    /// How the instance of the function block type `ty` in the slot
    /// `var_index`, whose fields take `run`, is reached; `None` within when
    /// its block has no layout.
    pub(super) fn instance(
        &self,
        ty: TypeId,
        var_index: VarIndex,
        run: Run,
    ) -> Option<Option<FbInstanceInfo>> {
        let TypeKind::FunctionBlock { body, .. } = self.lowering.types().kind(ty)? else {
            return None;
        };
        let Some(fields) = self.fields(*body)? else {
            return Some(None);
        };
        Some(Some(FbInstanceInfo {
            var_index,
            type_id: fields.type_id,
            data_offset: run.start,
            field_indices: fields.indices,
            field_types: fields.types,
            held: run.held,
        }))
    }

    /// What a call of the user function block `block`, whose body is
    /// `function_id` and whose copy slots start at `var_offset`, needs, with
    /// its methods `methods`, by their lowered POUs.
    pub(super) fn block_type(
        &self,
        block: PouId,
        function_id: FunctionId,
        var_offset: VarIndex,
        methods: HashMap<PouId, UserMethodInfo>,
    ) -> Option<UserFbTypeInfo> {
        let fields = self.fields(block)??;
        let types = self.lowering.types();
        let copy_slots = copy_slots(self.lowering, block)?;
        let mut held_fields = Vec::new();
        for (index, (idx, field)) in copy_slots.iter().enumerate() {
            if let TypeKind::FunctionBlock { body, .. } = types.kind(field.ty)? {
                held_fields.push(HeldField {
                    field: *idx,
                    index: FbFieldIndex::try_from(index).ok()?,
                    block: *body,
                });
            }
        }
        Some(UserFbTypeInfo {
            type_id: fields.type_id,
            num_fields: copy_slots.len(),
            field_indices: fields.indices,
            function_id,
            var_offset: var_offset.raw(),
            field_types: fields.types,
            held_fields,
            methods,
        })
    }

    /// The fields of the function block whose body is `body`: a user
    /// block's copy slots, or a standard block's VM fields; `None` within
    /// when it is neither reachable nor standard.
    fn fields(&self, body: PouId) -> Option<Option<Fields>> {
        let types = self.lowering.types();
        if let Some(position) = self.user.iter().position(|user| *user == body) {
            let mut indices = HashMap::new();
            let mut field_types = HashMap::new();
            for (index, (idx, field)) in copy_slots(self.lowering, body)?.into_iter().enumerate() {
                indices.insert(idx, FbFieldIndex::try_from(index).ok()?);
                let held = slot_type(types, field.ty).unwrap_or(DEFAULT_VAR_TYPE_INFO);
                field_types.insert(idx, held);
            }
            let type_id = FIRST_USER_FB_TYPE_ID
                .raw()
                .checked_add(u16::try_from(position).ok()?)?;
            return Some(Some(Fields {
                type_id: FbTypeId::new(type_id),
                indices,
                types: field_types,
            }));
        }
        let Some(block) = self.standard.get(&body) else {
            return Some(None);
        };
        // The VM's block numbers its fields by name; the lowered type
        // declares the same fields.
        let (type_id, _, by_name) = standard_fb_layout(*block);
        let instance = instance_type(types, body)?;
        let mut indices = HashMap::new();
        let mut field_types = HashMap::new();
        for (position, field) in types.fields(instance)?.iter().enumerate() {
            let idx = types.field_idx(instance, position)?;
            if let Some(index) = by_name.get(&field.name.to_string().to_lowercase()) {
                indices.insert(idx, *index);
            }
            if let Some(held) = slot_type(types, field.ty) {
                field_types.insert(idx, held);
            }
        }
        Some(Some(Fields {
            type_id,
            indices,
            types: field_types,
        }))
    }
}

/// What a call of a function or a method needs to know of its variables,
/// gathered as they are laid out.
#[derive(Default)]
pub(super) struct Calling {
    /// How a call passes each `VAR_INPUT` and `VAR_IN_OUT`, in the order a
    /// call's normal form names them.
    pub(super) params: Vec<ParamPassing>,
    /// The slot of each `VAR_OUTPUT`.
    pub(super) outputs: HashMap<VarId, VarIndex>,
    /// Where in the data region each string `VAR_OUTPUT` is.
    pub(super) string_outputs: HashMap<VarId, StringVarInfo>,
}

impl Calling {
    /// Records the variable `var` of the section `section`, in the slot
    /// `index`, which holds the string `string` when it is one.
    pub(super) fn add(
        &mut self,
        section: Section,
        var: VarId,
        index: VarIndex,
        string: Option<StringVarInfo>,
    ) {
        match section {
            Section::InOut => self.params.push(ParamPassing::Reference),
            // A call copies a string argument into the parameter's run.
            Section::Input => self
                .params
                .push(string.map_or(ParamPassing::Value, |string| {
                    ParamPassing::String(StringParamInfo {
                        data_offset: string.data_offset,
                        max_length: string.max_length,
                        char_width: string.char_width,
                    })
                })),
            Section::Output => {
                self.outputs.insert(var, index);
                if let Some(string) = string {
                    self.string_outputs.insert(var, string);
                }
            }
            Section::Local
            | Section::Temporary
            | Section::Global
            | Section::Instance
            | Section::Internal => {}
        }
    }
}
