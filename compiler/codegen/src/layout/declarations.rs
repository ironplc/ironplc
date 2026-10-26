//! The layout built from the lowered declarations
//! (`specs/design/lowered-program.md`, "`CompileContext`").
//!
//! It numbers the variable-table slots from the execution model's globals
//! and each body's declarations, in the order the body declares them:
//!
//! 1. the globals, the compiler-provided ones first, then those of the
//!    resource that runs the program;
//! 2. the program's fields, `VAR_TEMP` and the globals a PLCopen program
//!    declares in its interface;
//! 3. each user function, in library order: its `VAR_INPUT` and
//!    `VAR_IN_OUT`, then its other variables, then its result and the
//!    result's scratch slot, if it takes one;
//! 4. each user function block, in library order: one copy slot for each of
//!    its own inputs, then outputs, then locals; then each method, laid out
//!    as a function is, with a result slot whether it returns one or not.
//!
//! A structure whose fields hold strings in arrays, and an array of
//! structures with a string field, take a scratch slot after their own.
//!
//! Each slot that holds one value is operated at its type, the one a
//! `VAR_IN_OUT` or `VAR_OUTPUT` of a function or method refers to for one
//! of those; a compiler-provided global has none.
//!
//! Each string has a run of the data region of its own ([`super::region`]),
//! but a function's `VAR_IN_OUT`, which refers to its caller's. An array,
//! a structure, an array of structures and what a reference refers to are
//! reached through the container's array descriptors, which the
//! declarations add in the walk's order ([`super::descriptors`]). A
//! structure's and an array of structures' entry names its lowered type,
//! whose fields an access selects ([`super::entries`]). A function block
//! instance's fields, and what a call of each user function, function block
//! and method needs, come from the same declarations ([`super::calls`]), and
//! so do the debug entries of the variables and of the strings of the
//! globals and the program, and of each enumeration type
//! ([`super::debug_vars`]).
//!
//! The bodies are compiled against this layout ([`Layout::install`]), which
//! gives each body where its variables are ([`super::bodies`]). Each
//! declaration is checked within the limits of the bytecode format first
//! ([`super::limits`]).

use std::collections::HashMap;

use ironplc_container::debug_section::EnumDefEntry;
use ironplc_container::{ArrayDescriptor, ContainerBuilder, FunctionId, VarIndex};
use ironplc_ir::execution::{GlobalId, GlobalKind, GlobalScope};
use ironplc_ir::lowered::StandardFunctionBlock;
use ironplc_ir::lowered::{Declaration, Program};
use ironplc_ir::lowered::{
    Field, FieldIdx, Nullability, PouId, PouKind, Section, TypeId, TypeKind, TypeTable, VarId,
};
use ironplc_ir::{Diagnostic, Label};
use ironplc_ir::{Id, SourceSpan};

use super::calls::{Blocks, Calling};
use super::debug_vars::DebugVars;
use super::descriptors::Descriptors;
use super::entries;
use super::names::{ScopeId, Scoped};
use super::region::{string_at, Region};
use super::var_table::Counter;
use super::{BlockLayout, FunctionLayout, MethodLayout};
use crate::access::{Named, Stored};
use crate::body::slot_type;
use crate::compile::{
    CompileContext, FbInstanceInfo, OpType, StringReturnInfo, StringVarInfo, UserFbTypeInfo,
    UserFunctionInfo, UserMethodInfo, VarTypeInfo, DEFAULT_OP_TYPE,
};
use crate::compile_array::ArrayVarInfo;
use crate::compile_array_struct::StructArrayVarInfo;
use crate::compile_reference::StringRefInfo;
use crate::compile_struct::StructVarInfo;

/// What the layout built from the lowered declarations knows of each name:
/// its slot, the type the slot is operated at, a string's run of the data
/// region, how an array, a structure, an array of structures, what a
/// reference refers to and a function block instance are reached, and
/// whether it is a function's `VAR_IN_OUT`; where the region ends; the
/// array descriptors, by index; and what a call of each user function,
/// function block and method needs.
#[derive(Default)]
pub(crate) struct Layout {
    pub(super) slots: Scoped<VarIndex>,
    pub(super) var_types: Scoped<VarTypeInfo>,
    pub(super) string_vars: Scoped<StringVarInfo>,
    pub(super) array_vars: Scoped<ArrayVarInfo>,
    pub(super) struct_vars: Scoped<StructVarInfo>,
    pub(super) struct_array_vars: Scoped<StructArrayVarInfo>,
    pub(super) string_refs: Scoped<StringRefInfo>,
    pub(super) region_refs: Scoped<u16>,
    pub(super) fb_instances: Scoped<FbInstanceInfo>,
    pub(super) in_out_params: Scoped<()>,
    pub(super) data_end: u32,
    pub(super) descriptors: Vec<ArrayDescriptor>,
    /// What a call of each user function needs, by its lowered POU.
    pub(super) user_functions: HashMap<PouId, UserFunctionInfo>,
    /// What a call of each user function block and its methods needs, by
    /// the block's lowered POU.
    pub(super) user_fb_types: HashMap<PouId, UserFbTypeInfo>,
    /// The debug entries of the variables and the strings.
    pub(super) debug: DebugVars,
    /// The debug entry of each enumeration type.
    pub(super) enum_defs: Vec<EnumDefEntry>,
    /// Where each user function's variables are, in library order.
    pub(super) functions: Vec<FunctionLayout>,
    /// Where each user function block's copy slots and its methods'
    /// variables are, in library order.
    pub(super) blocks: Vec<BlockLayout>,
    /// How many slots the globals and the program's variables take.
    pub(super) program_slots: u16,
    /// The scope the program's body is compiled in.
    pub(super) program: ScopeId,
    /// How many slots the variable table holds.
    pub(super) slot_end: VarIndex,
    /// Whether every slot fits the variable table.
    counter: Counter,
    /// How many slots the globals and the program count so far.
    counted: usize,
}

/// What the bodies are compiled with, of a [`Layout`] once it is installed.
pub(crate) struct Bodies {
    /// Where each user function's variables are, in library order.
    pub(crate) functions: Vec<FunctionLayout>,
    /// Where each user function block's copy slots and its methods'
    /// variables are, in library order.
    pub(crate) blocks: Vec<BlockLayout>,
    /// How many slots the globals and the program's variables take.
    pub(crate) program_slots: u16,
    /// How many slots the variable table holds.
    pub(crate) slots: VarIndex,
}

/// What the declarations reserve as the walk visits them: the runs of the
/// data region, and the array descriptors; and the function blocks an
/// instance can be of.
struct Storage<'a> {
    region: Region<'a>,
    descriptors: Descriptors<'a>,
    blocks: Blocks<'a>,
}

impl Layout {
    /// The layout of the declarations of `lowering`, whose program the
    /// container runs is the POU `program`, and the standard function block
    /// each POU that stands for one is, `standard_blocks`. A declaration
    /// that needs more slots than the variable table holds is P9997.
    pub(crate) fn of(
        lowering: &Program,
        program: PouId,
        standard_blocks: &HashMap<PouId, StandardFunctionBlock>,
    ) -> Result<Self, Diagnostic> {
        let span = lowering
            .pous()
            .name(program)
            .map(|name| name.span())
            .unwrap_or_default();
        let no_layout = || {
            Diagnostic::internal_error_at(Label::span(
                span.clone(),
                "The lowered declarations give no layout",
            ))
        };
        let pous = Pous::of(lowering, program).ok_or_else(no_layout)?;
        super::limits::check(lowering, &pous).ok_or_else(no_layout)??;
        let layout = Self::of_complete(lowering, &pous, standard_blocks).ok_or_else(no_layout)?;
        layout.counter.check(&span)?;
        Ok(layout)
    }

    /// The layout of the declarations of `lowering`, every one of which
    /// lowered and is within the limits, of the POUs `pous`; `None` when
    /// they are not the program's.
    fn of_complete(
        lowering: &Program,
        pous: &Pous,
        standard_blocks: &HashMap<PouId, StandardFunctionBlock>,
    ) -> Option<Self> {
        let types = lowering.types();
        let mut storage = Storage {
            region: Region::new(lowering, &pous.blocks, standard_blocks),
            descriptors: Descriptors::new(types),
            blocks: Blocks::new(lowering, &pous.blocks, standard_blocks),
        };
        let mut layout = Self {
            program: ScopeId::Pou(pous.program),
            ..Self::default()
        };

        let program_declarations = lowering.declarations(pous.program)?;
        for (var, system) in globals(lowering) {
            if system {
                // The runtime writes a compiler-provided global, which no
                // body operates on through the layout's type.
                let name = named(lowering, var)?;
                let index = layout.insert_counted(ScopeId::Global, name.stored, None);
                let ty = lowering.variables().get(var)?.ty();
                layout.debug.system(types, index, &name.name, ty);
            } else if !program_declarations.contains(&Declaration::Variable(var)) {
                // A global the program declares is laid out with its
                // variables.
                layout.insert_variable(ScopeId::Global, &mut storage, var)?;
            }
        }
        let program_type = instance_type(types, pous.program)?;
        for declaration in program_declarations {
            match declaration {
                Declaration::Field(idx) => {
                    let field = types.fields(program_type)?.get(idx.index() as usize)?;
                    layout.insert_named(
                        layout.program,
                        &mut storage,
                        named_field(program_type, *idx, field),
                        field.ty,
                        field.section,
                    )?;
                }
                Declaration::Variable(var) => {
                    layout.insert_variable(layout.program, &mut storage, *var)?;
                }
                Declaration::External(_) => {}
            }
        }

        // Saturated: a count past the table has marked it full.
        let mut next = VarIndex::new(u16::try_from(layout.counted).unwrap_or(u16::MAX));
        layout.program_slots = next.raw();
        for (function_id, function) in pous.functions() {
            let scope = ScopeId::Pou(function);
            let frame = frame(lowering, function)?;
            let var_offset = next;
            let mut calling = Calling::default();
            for var in frame.variables {
                let name = named(lowering, var)?;
                let section = lowering.variables().get(var)?.section();
                // A `VAR_IN_OUT` refers to its caller's variable.
                let string = if section == Section::InOut {
                    layout.in_out_params.insert(scope, name.stored, ());
                    None
                } else {
                    let held = held(lowering, var)?;
                    layout.insert_reference(&mut storage, scope, name.stored, held, next)?;
                    layout.insert_string(&mut storage, scope, name.stored, held)?
                };
                calling.add(section, var, next, string);
                layout.debug.local(
                    types,
                    next,
                    function_id,
                    Some(section),
                    &name.name,
                    held(lowering, var)?,
                );
                layout.insert(scope, name.stored, next, held_type(lowering, var));
                next = layout.counter.after(next);
            }
            let result = frame.result?;
            let name = named(lowering, result)?;
            let result_index = next;
            layout.debug.result(
                types,
                next,
                function_id,
                &name.name,
                held(lowering, result)?,
            );
            layout.insert(scope, name.stored, next, held_type(lowering, result));
            next = layout.counter.after(next);
            let ty = held(lowering, result)?;
            let return_string = layout.insert_string(&mut storage, scope, name.stored, ty)?;
            let mut return_struct_desc_index = None;
            if matches!(types.kind(ty), Some(TypeKind::Structure { .. })) {
                let data_offset = storage.region.structure(ty)?;
                let slots = storage.descriptors.structure(ty)?;
                return_struct_desc_index = Some(slots.desc_index);
                // A structure result takes its scratch slot after it, as a
                // global's or the program's variable does, and the
                // function's run counts it. The result is a result
                // parameter, which refers to the caller's structure.
                let scratch_index = needs_scratch(types, ty).then_some(next);
                if scratch_index.is_some() {
                    layout.counter.slot(usize::from(next.raw()));
                    next = layout.counter.after(next);
                }
                let entry = entries::structure(ty, result_index, data_offset, scratch_index, slots);
                layout.struct_vars.insert(scope, name.stored, entry);
            }
            let info = UserFunctionInfo {
                function_id,
                var_offset,
                num_params: u16::try_from(calling.params.len()).ok()?,
                params: calling.params,
                return_string_info: return_string.map(|string| StringReturnInfo {
                    data_offset: string.data_offset,
                    max_length: string.max_length,
                    char_width: string.char_width,
                }),
                return_struct_desc_index,
                outputs: calling.outputs,
                string_outputs: calling.string_outputs,
            };
            layout.functions.push(FunctionLayout {
                function_id,
                scope,
                num_locals: next.raw() - var_offset.raw(),
                num_params: info.num_params,
                return_var_index: result_index,
                return_op_type: loaded_at(held_type(lowering, result)),
                return_string_info: info.return_string_info.clone(),
            });
            layout.user_functions.insert(function, info);
        }
        for (function_id, block) in pous.blocks() {
            let field_var_off = next;
            let scope = ScopeId::Pou(block);
            let block_type = instance_type(types, block)?;
            for (idx, field) in copy_slots(lowering, block)? {
                let name = named_field(block_type, idx, field);
                layout.debug.local(
                    types,
                    next,
                    function_id,
                    field.section,
                    &name.name,
                    field.ty,
                );
                layout.insert_string(&mut storage, scope, name.stored, field.ty)?;
                layout.insert_reference(&mut storage, scope, name.stored, field.ty, next)?;
                layout.insert(scope, name.stored, next, slot_type(types, field.ty));
                next = layout.counter.after(next);
            }
            let mut body = BlockLayout {
                function_id,
                scope,
                num_locals: next.raw() - field_var_off.raw(),
                methods: Vec::new(),
            };
            let mut methods = HashMap::new();
            for (method_id, method) in pous.methods(block) {
                let scope = ScopeId::Pou(method);
                let frame = frame(lowering, method)?;
                let param_var_off = next;
                let mut current = next;
                let mut calling = Calling::default();
                for var in frame.variables {
                    let name = named(lowering, var)?;
                    // A call copies a string argument into the run of the
                    // parameter, a `VAR_IN_OUT`'s too.
                    let held_ty = held(lowering, var)?;
                    let string = layout.insert_string(&mut storage, scope, name.stored, held_ty)?;
                    let section = lowering.variables().get(var)?.section();
                    calling.add(section, var, current, string);
                    layout.insert(scope, name.stored, current, held_type(lowering, var));
                    current = layout.counter.after(current);
                }
                let has_return_value = frame.result.is_some();
                // A method has a result slot whether it returns a result or
                // not.
                let return_var_index = current;
                let mut return_op_type = DEFAULT_OP_TYPE;
                if let Some(result) = frame.result {
                    let held = held_type(lowering, result);
                    return_op_type = loaded_at(held);
                    layout.insert(scope, Stored::Var(result), current, held);
                }
                current = layout.counter.after(current);
                // The next method's slots follow this method's own.
                next = current;
                let info = UserMethodInfo {
                    function_id: method_id,
                    param_var_off: param_var_off.raw(),
                    num_params: u16::try_from(calling.params.len()).ok()?,
                    params: calling.params,
                    has_return_value,
                    outputs: calling.outputs,
                    string_outputs: calling.string_outputs,
                };
                body.methods.push(MethodLayout {
                    function_id: method_id,
                    scope,
                    // The method's frame spans its block's copy slots, the
                    // earlier methods' slots and its own.
                    num_locals: current.raw() - field_var_off.raw(),
                    num_params: info.num_params,
                    return_var_index,
                    return_op_type,
                    has_return_value,
                });
                methods.insert(method, info);
            }
            layout.blocks.push(body);
            let info = storage
                .blocks
                .block_type(block, function_id, field_var_off, methods)?;
            layout.user_fb_types.insert(block, info);
        }
        layout.slot_end = next;
        layout.data_end = storage.region.end();
        layout.descriptors = storage.descriptors.into_added();
        layout.enum_defs = super::debug_vars::enum_definitions(types);
        Some(layout)
    }

    /// Makes this layout the one `ctx` compiles the bodies against
    /// (REQ-LOW-codegen-199), its array descriptors the ones `builder`
    /// holds, and its debug entries the ones the container holds; and gives
    /// what each body is compiled with.
    pub(crate) fn install(
        self,
        ctx: &mut CompileContext,
        builder: &mut ContainerBuilder,
    ) -> Result<Bodies, Diagnostic> {
        for descriptor in &self.descriptors {
            builder.add_strided_array_descriptor(
                descriptor.element_type,
                descriptor.total_elements,
                descriptor.element_extra,
                descriptor.element_stride,
            );
        }
        let (max_string_capacity, has_wide_string) =
            self.temporary_buffers(ctx.lowering()?.types());
        ctx.variables = self.slots;
        ctx.var_types = self.var_types;
        ctx.string_vars = self.string_vars;
        ctx.array_vars = self.array_vars;
        ctx.struct_vars = self.struct_vars;
        ctx.struct_array_vars = self.struct_array_vars;
        ctx.string_refs = self.string_refs;
        ctx.region_refs = self.region_refs;
        ctx.fb_instances = self.fb_instances;
        ctx.in_out_params = self.in_out_params;
        ctx.scope = self.program;
        ctx.user_functions = self.user_functions;
        ctx.user_fb_types = self.user_fb_types;
        ctx.data_region_offset = self.data_end;
        ctx.max_string_capacity = max_string_capacity;
        ctx.has_wide_string = has_wide_string;
        ctx.debug_var_names = self.debug.var_names;
        ctx.debug_string_layouts = self.debug.string_layouts;
        ctx.debug_enum_defs = self.enum_defs;
        Ok(Bodies {
            functions: self.functions,
            blocks: self.blocks,
            program_slots: self.program_slots,
            slots: self.slot_end,
        })
    }

    /// What the temporary buffers are sized for: the largest capacity of a
    /// string the layout holds, and whether a string of a global or the
    /// program, or an array of strings, is wide. A wide string of a function,
    /// a function block or a method does not size them (`compile_fn.rs`).
    /// Its structures' fields are those of their types in `types`.
    pub(super) fn temporary_buffers(&self, types: &TypeTable) -> (u16, bool) {
        let strings: Vec<_> = self.string_vars.entries().collect();
        let arrays: Vec<_> = self
            .array_vars
            .entries()
            .filter(|(_, array)| array.is_string_element)
            .collect();
        let capacity =
            strings
                .iter()
                .map(|(_, string)| string.max_length)
                .chain(arrays.iter().map(|(_, array)| array.string_max_len))
                .chain(self.struct_vars.entries().flat_map(|(_, entry)| {
                    entries::struct_capacities(types, entry, &self.descriptors)
                }))
                .chain(self.struct_array_vars.entries().flat_map(|(_, entry)| {
                    entries::struct_array_capacities(entry, &self.descriptors)
                }))
                .max()
                .unwrap_or(0);
        let wide = strings
            .iter()
            .filter(|(scope, _)| *scope == ScopeId::Global || *scope == self.program)
            .any(|(_, string)| string.char_width.is_wide())
            || arrays
                .iter()
                .any(|(_, array)| array.string_char_width.is_wide());
        (capacity, wide)
    }

    /// Records `stored` in `scope` at the slot `index`, operated at `held`;
    /// the table is marked full when it has no slot `index`.
    fn insert(
        &mut self,
        scope: ScopeId,
        stored: Stored,
        index: VarIndex,
        held: Option<VarTypeInfo>,
    ) {
        let index = self.counter.slot(usize::from(index.raw()));
        if let Some(held) = held {
            self.var_types.insert(scope, stored, held);
        }
        self.slots.insert(scope, stored, index);
    }

    /// Records the variable `var` in `scope`, the globals' or the
    /// program's, as [`Layout::insert_named`] does.
    fn insert_variable(&mut self, scope: ScopeId, storage: &mut Storage, var: VarId) -> Option<()> {
        let lowering = storage.region.lowering();
        let variable = lowering.variables().get(var)?;
        let (ty, section) = (variable.ty(), variable.section());
        self.insert_named(scope, storage, named(lowering, var)?, ty, Some(section))
    }

    /// Records `name`, of the type `ty`, in `scope`, the globals' or the
    /// program's, at the next slot the globals and the program count, and
    /// its scratch slot after it if it takes one; and reserves its run of
    /// the data region and its array descriptors, if it has any.
    fn insert_named(
        &mut self,
        scope: ScopeId,
        storage: &mut Storage,
        name: Named,
        ty: TypeId,
        section: Option<Section>,
    ) -> Option<()> {
        let types = storage.region.lowering().types();
        let run = storage.region.variable(ty)?;
        let start = run.as_ref().map(|run| run.start);
        let string = match (types.kind(ty), start) {
            (Some(TypeKind::String(shape)), Some(start)) => Some(string_at(start, *shape)?),
            _ => None,
        };
        if let Some(string) = &string {
            self.string_vars.insert(scope, name.stored, string.clone());
        }
        let index = self.insert_counted(scope, name.stored, slot_type(types, ty));
        self.debug
            .declared(types, index, section, &name.name, ty, string.as_ref());
        if let (Some(TypeKind::FunctionBlock { .. }), Some(run)) = (types.kind(ty), run) {
            if let Some(instance) = storage.blocks.instance(ty, index, run)? {
                self.fb_instances.insert(scope, name.stored, instance);
            }
        }
        let reached = storage.descriptors.variable(ty, index, start)?;
        let scratch_index = needs_scratch(types, ty).then(|| self.count());
        if let Some(slots) = reached.slots {
            let data_offset = start?;
            if matches!(types.kind(ty)?, TypeKind::Structure { .. }) {
                let entry = entries::structure(ty, index, data_offset, scratch_index, slots);
                self.struct_vars.insert(scope, name.stored, entry);
            } else {
                let entry =
                    entries::struct_array(types, ty, index, data_offset, scratch_index, slots)?;
                self.struct_array_vars.insert(scope, name.stored, entry);
            }
        }
        self.insert_reached(
            scope,
            name.stored,
            reached.array,
            reached.string_ref,
            reached.region_ref,
        );
        Some(())
    }

    /// Records how `stored` in `scope` is reached as an array, through the
    /// strings it refers to and through the descriptor that sizes what it
    /// refers to, of those it has.
    fn insert_reached(
        &mut self,
        scope: ScopeId,
        stored: Stored,
        array: Option<ArrayVarInfo>,
        string_ref: Option<StringRefInfo>,
        region_ref: Option<u16>,
    ) {
        if let Some(array) = array {
            self.array_vars.insert(scope, stored, array);
        }
        if let Some(string_ref) = string_ref {
            self.string_refs.insert(scope, stored, string_ref);
        }
        if let Some(region_ref) = region_ref {
            self.region_refs.insert(scope, stored, region_ref);
        }
    }

    /// Reserves the run of the data region that `stored`, of a function, a
    /// function block or a method, holds in `scope` when its type `ty` is a
    /// string, and returns where it is.
    fn insert_string(
        &mut self,
        storage: &mut Storage,
        scope: ScopeId,
        stored: Stored,
        ty: TypeId,
    ) -> Option<Option<StringVarInfo>> {
        let Some(TypeKind::String(shape)) = storage.region.lowering().types().kind(ty) else {
            return Some(None);
        };
        let string = storage.region.string(*shape)?;
        self.string_vars.insert(scope, stored, string.clone());
        Some(Some(string))
    }

    /// Adds the array descriptors that `stored`, of a function or a function
    /// block, whose slot is `index`, reaches its target through in `scope`
    /// when its type `ty` is a reference.
    fn insert_reference(
        &mut self,
        storage: &mut Storage,
        scope: ScopeId,
        stored: Stored,
        ty: TypeId,
        index: VarIndex,
    ) -> Option<()> {
        let types = storage.region.lowering().types();
        if let Some(TypeKind::Reference { target, .. }) = types.kind(ty) {
            let reached = storage.descriptors.reference(*target, index)?;
            self.insert_reached(
                scope,
                stored,
                reached.array,
                reached.string_ref,
                reached.region_ref,
            );
        }
        Some(())
    }

    /// Records `stored` in `scope` at the next slot the globals and the
    /// program count.
    fn insert_counted(
        &mut self,
        scope: ScopeId,
        stored: Stored,
        held: Option<VarTypeInfo>,
    ) -> VarIndex {
        let index = self.count();
        self.insert(scope, stored, index, held);
        index
    }

    /// The next slot the globals and the program count, which their
    /// variables and their scratch slots are numbered from.
    fn count(&mut self) -> VarIndex {
        let index = self.counter.slot(self.counted);
        self.counted += 1;
        index
    }
}

/// The type a result held at `held` is loaded at: a string or a structure
/// result lives in the data region, and its slot holds a data offset at the
/// default type.
fn loaded_at(held: Option<VarTypeInfo>) -> OpType {
    held.map(|info| (info.op_width, info.signedness))
        .unwrap_or(DEFAULT_OP_TYPE)
}

/// The type the slot of the variable `var` of a function or method is
/// operated at: that of what it [holds](held).
fn held_type(lowering: &Program, var: VarId) -> Option<VarTypeInfo> {
    slot_type(lowering.types(), held(lowering, var)?)
}

/// The type of what the variable `var` of a function or method holds: for
/// a `VAR_IN_OUT`, a `VAR_OUTPUT` or a result parameter, which lowering
/// makes a reference to the caller's place, the type of what it refers to.
fn held(lowering: &Program, var: VarId) -> Option<TypeId> {
    let types = lowering.types();
    let ty = lowering.variables().get(var)?.ty();
    Some(match types.reference(ty) {
        Some((target, Nullability::NonNull)) => target,
        Some((_, Nullability::Nullable)) | None => ty,
    })
}

/// The variables of the globals that have storage, in the execution
/// model's order, the compiler-provided ones first, each with whether the
/// compiler provides it.
///
/// A resource's `VAR_GLOBAL` is visible only to the programs of that
/// resource (IEC 61131-3, 2.7.1), so only the resource that runs the program
/// instance gives its globals storage; another resource's have none.
pub(crate) fn globals(lowering: &Program) -> Vec<(VarId, bool)> {
    let bindings = lowering.bindings();
    let execution = lowering.execution();
    let running: Vec<GlobalId> = execution
        .configurations()
        .iter()
        .flat_map(|configuration| &configuration.resources)
        .filter(|resource| resource.tasks.iter().any(|task| !task.instances.is_empty()))
        .flat_map(|resource| resource.globals.iter().copied())
        .collect();
    execution
        .globals()
        .filter_map(|(id, global)| {
            let system = match global.kind {
                GlobalKind::System(_) => true,
                GlobalKind::Declared(GlobalScope::TopLevel | GlobalScope::Configuration) => false,
                GlobalKind::Declared(GlobalScope::Resource) if running.contains(&id) => false,
                GlobalKind::Declared(GlobalScope::Resource) => return None,
            };
            Some((bindings.global(id)?, system))
        })
        .collect()
}

/// Whether a variable of the type `ty` takes a scratch slot: a structure
/// with a field that is an array of strings, or that holds an array of
/// structures with a string field; or an array of structures with a string
/// field.
fn needs_scratch(types: &TypeTable, ty: TypeId) -> bool {
    match types.kind(ty) {
        Some(TypeKind::Structure { fields }) => {
            fields
                .iter()
                .any(|field| is_array_of_strings(types, field.ty))
                || holds_element_strings(types, fields)
        }
        Some(TypeKind::Array { element, .. }) => has_string_field(types, *element),
        Some(
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::String(_)
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. }
            | TypeKind::Reference { .. },
        )
        | None => false,
    }
}

/// Whether `ty` is an array of strings.
fn is_array_of_strings(types: &TypeTable, ty: TypeId) -> bool {
    match types.kind(ty) {
        Some(TypeKind::Array { element, .. }) => {
            matches!(types.kind(*element), Some(TypeKind::String(_)))
        }
        _ => false,
    }
}

/// Whether the structure fields `fields`, or a structure among them, hold
/// an array of structures with a string field.
fn holds_element_strings(types: &TypeTable, fields: &[Field]) -> bool {
    fields.iter().any(|field| match types.kind(field.ty) {
        Some(TypeKind::Structure { fields }) => holds_element_strings(types, fields),
        Some(TypeKind::Array { element, .. }) => has_string_field(types, *element),
        _ => false,
    })
}

/// Whether `ty` is a structure with a field that is a string.
fn has_string_field(types: &TypeTable, ty: TypeId) -> bool {
    match types.kind(ty) {
        Some(TypeKind::Structure { fields }) => fields
            .iter()
            .any(|field| matches!(types.kind(field.ty), Some(TypeKind::String(_)))),
        _ => false,
    }
}

/// The span of the name of each POU whose source file the debug section's
/// `SOURCE_FILE_TABLE` lists, in the order it lists them: the program the
/// container runs, the POU `program`, then each function, then each function
/// block.
pub(crate) fn source_spans(
    lowering: &Program,
    program: PouId,
) -> Result<Vec<SourceSpan>, Diagnostic> {
    let spans = Pous::of(lowering, program).and_then(|pous| {
        let table = lowering.pous();
        std::iter::once(pous.program)
            .chain(pous.functions.iter().copied())
            .chain(pous.blocks.iter().copied())
            .map(|pou| Some(table.name(pou)?.span()))
            .collect()
    });
    spans.ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            SourceSpan::default(),
            "The lowered program does not run the program it binds",
        ))
    })
}

/// The reachable POUs of a lowered program and the container function id
/// each body takes: each function block's from 2, then each function's,
/// then each method's.
pub(super) struct Pous {
    pub(super) program: PouId,
    pub(super) functions: Vec<PouId>,
    pub(super) blocks: Vec<PouId>,
    /// Each method, with the block it belongs to, in block order.
    pub(super) methods: Vec<(PouId, PouId)>,
}

impl Pous {
    /// The POUs of `lowering`, whose program the container runs is the POU
    /// `program`.
    fn of(lowering: &Program, program: PouId) -> Option<Self> {
        let table = lowering.pous();
        let mut found = None;
        let mut functions = Vec::new();
        let mut blocks = Vec::new();
        let mut methods = Vec::new();
        for pou in table.ids() {
            // A standard function block has no declaration of its own.
            let Some(parts) = lowering.parts(pou) else {
                continue;
            };
            match parts.kind {
                PouKind::Program => {
                    if pou == program {
                        found = Some(pou);
                    }
                }
                PouKind::Function => functions.push(pou),
                PouKind::FunctionBlock => blocks.push(pou),
                PouKind::Method => methods.push((table.method_of(pou)?, pou)),
            }
        }
        // Each block's methods follow every block's body and every
        // function's, in the order of the blocks.
        let mut ordered = Vec::with_capacity(methods.len());
        for block in &blocks {
            ordered.extend(methods.iter().filter(|(of, _)| of == block));
        }
        Some(Self {
            program: found?,
            functions,
            blocks,
            methods: ordered,
        })
    }

    /// Each function, with its function id.
    fn functions(&self) -> impl Iterator<Item = (FunctionId, PouId)> + '_ {
        let base = 2 + self.blocks.len() as u16;
        (base..)
            .map(FunctionId::new)
            .zip(self.functions.iter().copied())
    }

    /// Each function block, with the function id of its body.
    fn blocks(&self) -> impl Iterator<Item = (FunctionId, PouId)> + '_ {
        (2_u16..)
            .map(FunctionId::new)
            .zip(self.blocks.iter().copied())
    }

    /// Each method of `block`, with its function id.
    fn methods(&self, block: PouId) -> impl Iterator<Item = (FunctionId, PouId)> + '_ {
        let base = 2 + (self.blocks.len() + self.functions.len()) as u16;
        (base..)
            .map(FunctionId::new)
            .zip(self.methods.iter())
            .filter(move |(_, (of, _))| *of == block)
            .map(|(id, (_, method))| (id, *method))
    }
}

/// The variable `var`, by its name.
pub(crate) fn named(lowering: &Program, var: VarId) -> Option<Named> {
    let name = lowering.variables().get(var)?.name();
    Some(Named {
        stored: Stored::Var(var),
        name: Id::from(&name.to_string()).with_position(name.span()),
    })
}

/// The field `idx` of an instance of `instance`, which is `field`, by its
/// name.
pub(crate) fn named_field(instance: TypeId, idx: FieldIdx, field: &Field) -> Named {
    Named {
        stored: Stored::Field(instance, idx),
        name: Id::from(&field.name.to_string()).with_position(field.name.span()),
    }
}

/// The type of the instance the body of `pou` works on, a program or a
/// function block.
pub(crate) fn instance_type(types: &TypeTable, pou: PouId) -> Option<TypeId> {
    types.iter().find_map(|(ty, entry)| match &entry.kind {
        TypeKind::Program { body, .. } | TypeKind::FunctionBlock { body, .. } => {
            (*body == pou).then_some(ty)
        }
        TypeKind::Bool
        | TypeKind::Integer { .. }
        | TypeKind::BitString { .. }
        | TypeKind::Real { .. }
        | TypeKind::Temporal { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Subrange { .. }
        | TypeKind::String(_)
        | TypeKind::Array { .. }
        | TypeKind::Structure { .. }
        | TypeKind::Reference { .. } => None,
    })
}

/// The fields of the function block `pou` that its copy slots hold, each
/// with its index: its own inputs, then its outputs, then its locals, each
/// in declaration order.
pub(super) fn copy_slots(lowering: &Program, pou: PouId) -> Option<Vec<(FieldIdx, &Field)>> {
    let types = lowering.types();
    let fields = types.fields(instance_type(types, pou)?)?;
    let mut own = Vec::new();
    for declaration in lowering.declarations(pou)? {
        match declaration {
            Declaration::Field(idx) => own.push((*idx, fields.get(idx.index() as usize)?)),
            Declaration::Variable(_) | Declaration::External(_) => {}
        }
    }
    let mut slots = Vec::with_capacity(own.len());
    for section in [Section::Input, Section::Output, Section::Local] {
        slots.extend(
            own.iter()
                .filter(|(_, field)| field.section == Some(section)),
        );
    }
    Some(slots)
}

/// The variables of a function or method, in the order their slots run.
pub(super) struct Frame {
    /// Its `VAR_INPUT` and `VAR_IN_OUT`, then its `VAR_OUTPUT`, `VAR` and
    /// `VAR_TEMP`, each in declaration order.
    pub(super) variables: Vec<VarId>,
    /// Its result; `None` for a method that returns none.
    pub(super) result: Option<VarId>,
}

/// The variables of the function or method `pou`.
pub(super) fn frame(lowering: &Program, pou: PouId) -> Option<Frame> {
    let variables = lowering.variables();
    let mut inputs = Vec::new();
    let mut rest = Vec::new();
    for declaration in lowering.declarations(pou)? {
        let var = match declaration {
            Declaration::Variable(var) => *var,
            Declaration::Field(_) | Declaration::External(_) => continue,
        };
        match variables.get(var)?.section() {
            Section::Input | Section::InOut => inputs.push(var),
            Section::Output | Section::Local | Section::Temporary => rest.push(var),
            Section::Global | Section::Instance | Section::Internal => {}
        }
    }
    inputs.extend(rest);
    Some(Frame {
        variables: inputs,
        result: lowering.result(pou),
    })
}
