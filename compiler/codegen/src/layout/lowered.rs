//! The layout built from the lowered declarations
//! (`specs/design/lowered-program.md`, "`CompileContext`").
//!
//! It numbers the variable-table slots as the walk of the AST declarations
//! does ([`super`]), from the execution model's globals and each body's
//! declarations, in the order the body declares them:
//!
//! 1. the globals, the compiler-provided ones first, but a resource's;
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
//! declarations add in the walk's order ([`super::descriptors`]). The
//! check compares a structure's and an array of structures' entry without
//! the analyzer types it holds ([`super::entries`]). A function block
//! instance's fields, and what a call of each user function, function block
//! and method needs, come from the same declarations ([`super::calls`]).
//!
//! When every declaration lowers, the bodies are compiled against this
//! layout ([`Layout::install`]); the walk of the AST declarations still
//! runs, for the entries this one does not hold yet and for a program a
//! declaration of which does not lower, and the end-to-end helpers check
//! the two equal ([`super::check`]).

use std::collections::HashMap;

use ironplc_analyzer::StandardFunctionBlock;
use ironplc_container::{ArrayDescriptor, FunctionId, VarIndex};
use ironplc_dsl::core::Id;
use ironplc_ir::execution::{GlobalKind, GlobalScope};
use ironplc_ir::lowered::{
    Field, Nullability, PouId, PouKind, Section, TypeId, TypeKind, TypeTable, VarId,
};
use ironplc_lowering::{Declaration, PartialProgram};

use super::calls::{Blocks, Calling};
use super::descriptors::Descriptors;
use super::entries::{StructArrayEntry, StructEntry};
use super::names::{self, ScopeId, Scoped};
use super::region::{string_at, Region};
use crate::compile::{
    CompileContext, FbInstanceInfo, StringReturnInfo, StringVarInfo, UserFbTypeInfo,
    UserFunctionInfo, UserMethodInfo, UserMethodParam, VarTypeInfo,
};
use crate::compile_array::ArrayVarInfo;
use crate::compile_reference::StringRefInfo;
use crate::lowered::type_info;

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
    pub(super) struct_vars: Scoped<StructEntry>,
    pub(super) struct_array_vars: Scoped<StructArrayEntry>,
    pub(super) string_refs: Scoped<StringRefInfo>,
    pub(super) region_refs: Scoped<u16>,
    pub(super) fb_instances: Scoped<FbInstanceInfo>,
    pub(super) in_out_params: Scoped<()>,
    pub(super) data_end: u32,
    pub(super) descriptors: Vec<ArrayDescriptor>,
    /// What a call of each user function needs, by its lower-case name.
    pub(super) user_functions: HashMap<String, UserFunctionInfo>,
    /// What a call of each user function block and its methods needs, by
    /// the block's upper-case name.
    pub(super) user_fb_types: HashMap<String, UserFbTypeInfo>,
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
    /// container runs is `program`, and the standard function block each
    /// type is `standard_blocks`, by its upper-case name; `None` when a
    /// declaration did not lower, and the program keeps the AST layout.
    pub(crate) fn of(
        lowering: &PartialProgram,
        program: &Id,
        standard_blocks: &HashMap<String, StandardFunctionBlock>,
    ) -> Option<Self> {
        if !lowering.declarations_complete() {
            return None;
        }
        let pous = Pous::of(lowering, program)?;
        let types = lowering.types();
        let mut storage = Storage {
            region: Region::new(lowering, &pous.blocks, standard_blocks),
            descriptors: Descriptors::new(types),
            blocks: Blocks::new(lowering, &pous.blocks, standard_blocks),
        };
        let mut layout = Self::default();

        let program_declarations = lowering.declarations(pous.program)?;
        for (var, system) in globals(lowering) {
            if system {
                // The runtime writes a compiler-provided global, which no
                // body operates on through the layout's type.
                layout.insert_counted(ScopeId::Global, name(lowering, var)?, None);
            } else if !program_declarations.contains(&Declaration::Variable(var)) {
                // A global the program declares is laid out with its
                // variables.
                layout.insert_variable(ScopeId::Global, &mut storage, var)?;
            }
        }
        let program_type = instance_type(types, pous.program)?;
        for declaration in program_declarations {
            match declaration {
                Declaration::Field(field) => {
                    let field = types.fields(program_type)?.get(field.index() as usize)?;
                    layout.insert_named(
                        ScopeId::Program,
                        &mut storage,
                        name_of(field),
                        field.ty,
                    )?;
                }
                Declaration::Variable(var) => {
                    layout.insert_variable(ScopeId::Program, &mut storage, *var)?;
                }
                Declaration::External(_) => {}
            }
        }

        let mut next = VarIndex::new(names::slot_count(&layout.slots, ScopeId::Program) as u16);
        for (function_id, function) in pous.functions() {
            let scope = ScopeId::Function(function_id);
            let frame = frame(lowering, function)?;
            let var_offset = next;
            let mut calling = Calling::default();
            for var in frame.variables {
                let name = name(lowering, var)?;
                let section = lowering.variables().get(var)?.section();
                // A `VAR_IN_OUT` refers to its caller's variable.
                let string = if section == Section::InOut {
                    layout.in_out_params.insert(scope, name.clone(), ());
                    None
                } else {
                    let held = held(lowering, var)?;
                    layout.insert_reference(&mut storage, scope, &name, held, next)?;
                    layout.insert_string(&mut storage, scope, &name, held)?
                };
                calling.add(section, &name, next, string);
                layout.insert(scope, name, next, held_type(lowering, var));
                next = VarIndex::new(next.raw() + 1);
            }
            let result = frame.result?;
            let name = name(lowering, result)?;
            let result_index = next;
            layout.insert(scope, name.clone(), next, held_type(lowering, result));
            next = VarIndex::new(next.raw() + 1);
            let ty = held(lowering, result)?;
            let return_string = layout.insert_string(&mut storage, scope, &name, ty)?;
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
                    layout.insert(scope, scratch(&name), next, None);
                    next = VarIndex::new(next.raw() + 1);
                }
                let entry = StructEntry::lowered(
                    types,
                    ty,
                    result_index,
                    data_offset,
                    scratch_index,
                    slots,
                )?;
                layout.struct_vars.insert(scope, name, entry);
            }
            let info = UserFunctionInfo {
                function_id,
                var_offset,
                num_params: u16::try_from(calling.params.len()).ok()?,
                params: calling
                    .params
                    .into_iter()
                    .map(|(_, passing)| passing)
                    .collect(),
                return_string_info: return_string.map(|string| StringReturnInfo {
                    data_offset: string.data_offset,
                    max_length: string.max_length,
                    char_width: string.char_width,
                }),
                return_struct_desc_index,
                outputs: calling.outputs,
                string_outputs: calling.string_outputs,
            };
            let key = lowering.pous().name(function)?.to_string().to_lowercase();
            layout.user_functions.insert(key, info);
        }
        for (function_id, block) in pous.blocks() {
            let field_var_off = next;
            let scope = ScopeId::Block(function_id);
            for field in copy_slots(lowering, block)? {
                let name = name_of(field);
                layout.insert_string(&mut storage, scope, &name, field.ty)?;
                layout.insert_reference(&mut storage, scope, &name, field.ty, next)?;
                layout.insert(scope, name, next, slot_type(types, field.ty));
                next = VarIndex::new(next.raw() + 1);
            }
            let mut methods = HashMap::new();
            for (method_id, method) in pous.methods(block) {
                let scope = ScopeId::Method {
                    block: function_id,
                    method: method_id,
                };
                let frame = frame(lowering, method)?;
                let param_var_off = next;
                let mut current = next;
                let mut calling = Calling::default();
                for var in frame.variables {
                    let name = name(lowering, var)?;
                    // A call copies a string argument into the run of the
                    // parameter, a `VAR_IN_OUT`'s too.
                    let string =
                        layout.insert_string(&mut storage, scope, &name, held(lowering, var)?)?;
                    let section = lowering.variables().get(var)?.section();
                    calling.add(section, &name, current, string);
                    layout.insert(scope, name, current, held_type(lowering, var));
                    current = VarIndex::new(current.raw() + 1);
                }
                let has_return_value = frame.result.is_some();
                // A method has a result slot whether it returns a result or
                // not.
                if let Some(result) = frame.result {
                    let held = held_type(lowering, result);
                    layout.insert(scope, name(lowering, result)?, current, held);
                }
                current = VarIndex::new(current.raw() + 1);
                // The next method's slots follow this method's own
                // (`block.rs`).
                next = current;
                let info = UserMethodInfo {
                    function_id: method_id,
                    param_var_off: param_var_off.raw(),
                    num_params: u16::try_from(calling.params.len()).ok()?,
                    params: calling
                        .params
                        .into_iter()
                        .map(|(name, passing)| UserMethodParam { name, passing })
                        .collect(),
                    has_return_value,
                    outputs: calling.outputs,
                    string_outputs: calling.string_outputs,
                };
                let key = lowering.pous().name(method)?.to_string().to_lowercase();
                methods.insert(key, info);
            }
            let info = storage
                .blocks
                .block_type(block, function_id, field_var_off, methods)?;
            let key = lowering.pous().name(block)?.to_string().to_uppercase();
            layout.user_fb_types.insert(key, info);
        }
        layout.data_end = storage.region.end();
        layout.descriptors = storage.descriptors.into_added();
        Some(layout)
    }

    /// Makes this layout the one `ctx` compiles the bodies against
    /// (REQ-LOW-codegen-199). A structure's and an array of structures'
    /// entries, which hold analyzer types, and the debug entries stay the
    /// AST layout's, as do the array descriptors, which the walk of the AST
    /// declarations has already added to the container.
    pub(crate) fn install(self, ctx: &mut CompileContext) {
        let (max_string_capacity, has_wide_string) = self.temporary_buffers();
        ctx.variables = self.slots;
        ctx.var_types = self.var_types;
        ctx.string_vars = self.string_vars;
        ctx.array_vars = self.array_vars;
        ctx.string_refs = self.string_refs;
        ctx.region_refs = self.region_refs;
        ctx.fb_instances = self.fb_instances;
        ctx.in_out_params = self.in_out_params;
        ctx.user_functions = self.user_functions;
        ctx.user_fb_types = self.user_fb_types;
        ctx.data_region_offset = self.data_end;
        ctx.max_string_capacity = max_string_capacity;
        ctx.has_wide_string = has_wide_string;
    }

    /// What the temporary buffers are sized for: the largest capacity of a
    /// string the layout holds, and whether a string of a global or the
    /// program, or an array of strings, is wide. A wide string of a function,
    /// a function block or a method does not size them (`compile_fn.rs`).
    pub(super) fn temporary_buffers(&self) -> (u16, bool) {
        let strings: Vec<_> = self.string_vars.entries().collect();
        let arrays: Vec<_> = self
            .array_vars
            .entries()
            .filter(|(_, array)| array.is_string_element)
            .collect();
        let capacity = strings
            .iter()
            .map(|(_, string)| string.max_length)
            .chain(arrays.iter().map(|(_, array)| array.string_max_len))
            .chain(
                self.struct_vars
                    .entries()
                    .flat_map(|(_, entry)| entry.capacities(&self.descriptors)),
            )
            .chain(
                self.struct_array_vars
                    .entries()
                    .flat_map(|(_, entry)| entry.capacities(&self.descriptors)),
            )
            .max()
            .unwrap_or(0);
        let wide = strings
            .iter()
            .filter(|(scope, _)| matches!(scope, ScopeId::Global | ScopeId::Program))
            .any(|(_, string)| string.char_width.is_wide())
            || arrays
                .iter()
                .any(|(_, array)| array.string_char_width.is_wide());
        (capacity, wide)
    }

    /// Records `name` in `scope` at the slot `index`, operated at `held`.
    fn insert(&mut self, scope: ScopeId, name: Id, index: VarIndex, held: Option<VarTypeInfo>) {
        if let Some(held) = held {
            self.var_types.insert(scope, name.clone(), held);
        }
        self.slots.insert(scope, name, index);
    }

    /// Records the variable `var` in `scope`, the globals' or the
    /// program's, as [`Layout::insert_named`] does.
    fn insert_variable(&mut self, scope: ScopeId, storage: &mut Storage, var: VarId) -> Option<()> {
        let lowering = storage.region.lowering();
        let ty = lowering.variables().get(var)?.ty();
        self.insert_named(scope, storage, name(lowering, var)?, ty)
    }

    /// Records `name`, of the type `ty`, in `scope`, the globals' or the
    /// program's, at the next slot the scope counts, and its scratch slot
    /// after it if it takes one; and reserves its run of the data region
    /// and its array descriptors, if it has any.
    fn insert_named(
        &mut self,
        scope: ScopeId,
        storage: &mut Storage,
        name: Id,
        ty: TypeId,
    ) -> Option<()> {
        let types = storage.region.lowering().types();
        let run = storage.region.variable(ty)?;
        let start = run.as_ref().map(|run| run.start);
        if let (Some(TypeKind::String(shape)), Some(start)) = (types.kind(ty), start) {
            self.string_vars
                .insert(scope, name.clone(), string_at(start, *shape)?);
        }
        let index = self.insert_counted(scope, name.clone(), slot_type(types, ty));
        if let (Some(TypeKind::FunctionBlock { .. }), Some(run)) = (types.kind(ty), run) {
            if let Some(instance) = storage.blocks.instance(ty, index, run)? {
                self.fb_instances.insert(scope, name.clone(), instance);
            }
        }
        let reached = storage.descriptors.variable(ty, index, start)?;
        let scratch_index =
            needs_scratch(types, ty).then(|| self.insert_counted(scope, scratch(&name), None));
        if let Some(slots) = reached.slots {
            let data_offset = start?;
            if matches!(types.kind(ty)?, TypeKind::Structure { .. }) {
                let entry =
                    StructEntry::lowered(types, ty, index, data_offset, scratch_index, slots)?;
                self.struct_vars.insert(scope, name.clone(), entry);
            } else {
                let entry =
                    StructArrayEntry::lowered(types, ty, index, data_offset, scratch_index, slots)?;
                self.struct_array_vars.insert(scope, name.clone(), entry);
            }
        }
        self.insert_reached(
            scope,
            &name,
            reached.array,
            reached.string_ref,
            reached.region_ref,
        );
        Some(())
    }

    /// Records how `name` in `scope` is reached as an array, through the
    /// strings it refers to and through the descriptor that sizes what it
    /// refers to, of those it has.
    fn insert_reached(
        &mut self,
        scope: ScopeId,
        name: &Id,
        array: Option<ArrayVarInfo>,
        string_ref: Option<StringRefInfo>,
        region_ref: Option<u16>,
    ) {
        if let Some(array) = array {
            self.array_vars.insert(scope, name.clone(), array);
        }
        if let Some(string_ref) = string_ref {
            self.string_refs.insert(scope, name.clone(), string_ref);
        }
        if let Some(region_ref) = region_ref {
            self.region_refs.insert(scope, name.clone(), region_ref);
        }
    }

    /// Reserves the run of the data region that `name`, of a function, a
    /// function block or a method, holds in `scope` when its type `ty` is a
    /// string, and returns where it is.
    fn insert_string(
        &mut self,
        storage: &mut Storage,
        scope: ScopeId,
        name: &Id,
        ty: TypeId,
    ) -> Option<Option<StringVarInfo>> {
        let Some(TypeKind::String(shape)) = storage.region.lowering().types().kind(ty) else {
            return Some(None);
        };
        let string = storage.region.string(*shape)?;
        self.string_vars.insert(scope, name.clone(), string.clone());
        Some(Some(string))
    }

    /// Adds the array descriptors that `name`, of a function or a function
    /// block, whose slot is `index`, reaches its target through in `scope`
    /// when its type `ty` is a reference.
    fn insert_reference(
        &mut self,
        storage: &mut Storage,
        scope: ScopeId,
        name: &Id,
        ty: TypeId,
        index: VarIndex,
    ) -> Option<()> {
        let types = storage.region.lowering().types();
        if let Some(TypeKind::Reference { target, .. }) = types.kind(ty) {
            let reached = storage.descriptors.reference(*target, index)?;
            self.insert_reached(
                scope,
                name,
                reached.array,
                reached.string_ref,
                reached.region_ref,
            );
        }
        Some(())
    }

    /// Records `name` in `scope` at the next slot the body of `scope`
    /// counts, as the walk of the AST declarations numbers the globals, the
    /// program's variables and the scratch slots.
    fn insert_counted(&mut self, scope: ScopeId, name: Id, held: Option<VarTypeInfo>) -> VarIndex {
        let index = VarIndex::new(names::slot_count(&self.slots, scope) as u16);
        self.insert(scope, name, index, held);
        index
    }
}

/// The type the slot of the variable `var` of a function or method is
/// operated at: that of what it [holds](held).
fn held_type(lowering: &PartialProgram, var: VarId) -> Option<VarTypeInfo> {
    slot_type(lowering.types(), held(lowering, var)?)
}

/// The type of what the variable `var` of a function or method holds: for
/// a `VAR_IN_OUT`, a `VAR_OUTPUT` or a result parameter, which lowering
/// makes a reference to the caller's place, the type of what it refers to.
fn held(lowering: &PartialProgram, var: VarId) -> Option<TypeId> {
    let types = lowering.types();
    let ty = lowering.variables().get(var)?.ty();
    Some(match types.reference(ty) {
        Some((target, Nullability::NonNull)) => target,
        Some((_, Nullability::Nullable)) | None => ty,
    })
}

/// The variables of the globals that have storage, in the execution
/// model's order, the compiler-provided ones first, each with whether the
/// compiler provides it. A resource's `VAR_GLOBAL` has none yet
/// (`execution.rs`).
fn globals(lowering: &PartialProgram) -> Vec<(VarId, bool)> {
    let bindings = lowering.bindings();
    lowering
        .execution()
        .globals()
        .filter_map(|(id, global)| {
            let system = match global.kind {
                GlobalKind::System(_) => true,
                GlobalKind::Declared(GlobalScope::TopLevel | GlobalScope::Configuration) => false,
                GlobalKind::Declared(GlobalScope::Resource) => return None,
            };
            Some((bindings.global(id)?, system))
        })
        .collect()
}

/// The type a slot holding a value of the lowered type `ty` is operated
/// at. A `BOOL` holds one bit of value, which also marks it for the
/// conversion path (`type_info.rs`).
pub(super) fn slot_type(types: &TypeTable, ty: TypeId) -> Option<VarTypeInfo> {
    match types.kind(ty) {
        Some(TypeKind::Bool) => Some(VarTypeInfo {
            storage_bits: 1,
            ..type_info(types, ty)?
        }),
        _ => type_info(types, ty),
    }
}

/// The name of the scratch slot of the variable `name`
/// (`CompileContext::allocate_scratch_variable`).
fn scratch(name: &Id) -> Id {
    Id::from(&format!("$scratch_{name}"))
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

/// The reachable POUs of a lowered program and the container function id
/// each body takes: each function block's from 2, then each function's,
/// then each method's.
struct Pous {
    program: PouId,
    functions: Vec<PouId>,
    blocks: Vec<PouId>,
    /// Each method, with the block it belongs to, in block order.
    methods: Vec<(PouId, PouId)>,
}

impl Pous {
    /// The POUs of `lowering`, whose program the container runs is
    /// `program`.
    fn of(lowering: &PartialProgram, program: &Id) -> Option<Self> {
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
                    if Id::from(&table.name(pou)?.to_string()) == *program {
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

/// The name of the variable `var`.
fn name(lowering: &PartialProgram, var: VarId) -> Option<Id> {
    Some(Id::from(&lowering.variables().get(var)?.name().to_string()))
}

/// The name of the field `field`.
fn name_of(field: &Field) -> Id {
    Id::from(&field.name.to_string())
}

/// The type of the instance the body of `pou` works on, a program or a
/// function block.
pub(super) fn instance_type(types: &TypeTable, pou: PouId) -> Option<TypeId> {
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

/// The fields of the function block `pou` that its copy slots hold: its
/// own inputs, then its outputs, then its locals, each in declaration order.
pub(super) fn copy_slots(lowering: &PartialProgram, pou: PouId) -> Option<Vec<&Field>> {
    let types = lowering.types();
    let fields = types.fields(instance_type(types, pou)?)?;
    let mut own = Vec::new();
    for declaration in lowering.declarations(pou)? {
        match declaration {
            Declaration::Field(field) => own.push(fields.get(field.index() as usize)?),
            Declaration::Variable(_) | Declaration::External(_) => {}
        }
    }
    let mut slots = Vec::with_capacity(own.len());
    for section in [Section::Input, Section::Output, Section::Local] {
        slots.extend(own.iter().filter(|field| field.section == Some(section)));
    }
    Some(slots)
}

/// The variables of a function or method, in the order their slots run.
struct Frame {
    /// Its `VAR_INPUT` and `VAR_IN_OUT`, then its `VAR_OUTPUT`, `VAR` and
    /// `VAR_TEMP`, each in declaration order.
    variables: Vec<VarId>,
    /// Its result; `None` for a method that returns none.
    result: Option<VarId>,
}

/// The variables of the function or method `pou`.
fn frame(lowering: &PartialProgram, pou: PouId) -> Option<Frame> {
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
