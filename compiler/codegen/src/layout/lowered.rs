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
//!    `VAR_IN_OUT`, then its other variables, then its result;
//! 4. each user function block, in library order: one copy slot for each of
//!    its own inputs, then outputs, then locals; then each method, laid out
//!    as a function is, with a result slot whether it returns one or not.
//!
//! A structure whose fields hold strings in arrays, and an array of
//! structures with a string field, take a scratch slot after their own.
//!
//! Until it is complete, the layout the bodies are compiled against is
//! still built from the AST, and this one is only checked equal to it.

use ironplc_container::{FunctionId, VarIndex};
use ironplc_dsl::core::Id;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::execution::{GlobalKind, GlobalScope};
use ironplc_ir::lowered::{Field, PouId, PouKind, Section, TypeId, TypeKind, TypeTable, VarId};
use ironplc_lowering::{Declaration, PartialProgram};

use super::names::{self, ScopeId, Scoped};
use crate::compile::CompileContext;

/// Checks the slots numbered from the declarations of `lowering` equal to
/// those `ctx` laid out from the AST, for the program `program`; an
/// internal error names each one that differs. A program a declaration of
/// which did not lower keeps the AST layout, and has nothing to check.
pub(crate) fn check(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    program: &Id,
) -> Result<(), Diagnostic> {
    let Some(lowered) = slots(lowering, program) else {
        return Ok(());
    };
    let differences = ctx.variables.differences(&lowered);
    if differences.is_empty() {
        return Ok(());
    }
    Err(Diagnostic::internal_error_at(Label::span(
        program.span.clone(),
        format!(
            "The layout built from the lowered declarations differs: {}",
            differences.join("; ")
        ),
    )))
}

/// The slot of each name the layout knows, numbered from the declarations
/// of `lowering`, whose program the container runs is `program`; `None`
/// when a declaration did not lower.
fn slots(lowering: &PartialProgram, program: &Id) -> Option<Scoped<VarIndex>> {
    if !lowering.declarations_complete() {
        return None;
    }
    let pous = Pous::of(lowering, program)?;
    let types = lowering.types();
    let mut slots = Scoped::default();

    let program_declarations = lowering.declarations(pous.program)?;
    for var in globals(lowering) {
        // A global the program declares is laid out with its variables.
        if !program_declarations.contains(&Declaration::Variable(var)) {
            insert_variable(&mut slots, ScopeId::Global, lowering, var)?;
        }
    }
    let program_type = instance_type(types, pous.program)?;
    for declaration in program_declarations {
        match declaration {
            Declaration::Field(field) => {
                let field = types.fields(program_type)?.get(field.index() as usize)?;
                insert_named(
                    &mut slots,
                    ScopeId::Program,
                    types,
                    name_of(field),
                    field.ty,
                );
            }
            Declaration::Variable(var) => {
                insert_variable(&mut slots, ScopeId::Program, lowering, *var)?;
            }
            Declaration::External(_) => {}
        }
    }

    let mut next = VarIndex::new(names::slot_count(&slots, ScopeId::Program) as u16);
    for (function_id, function) in pous.functions() {
        let scope = ScopeId::Function(function_id);
        let frame = frame(lowering, function)?;
        for var in frame.variables {
            slots.insert(scope, name(lowering, var)?, next);
            next = VarIndex::new(next.raw() + 1);
        }
        let result = frame.result?;
        let name = name(lowering, result)?;
        slots.insert(scope, name.clone(), next);
        next = VarIndex::new(next.raw() + 1);
        // A structure result is laid out as a global's or the program's
        // variable is, scratch slot and all, but the scratch slot is
        // counted from the names the function sees (`compile_struct.rs`).
        let ty = lowering.variables().get(result)?.ty();
        if matches!(types.kind(ty), Some(TypeKind::Structure { .. })) && needs_scratch(types, ty) {
            insert_counted(&mut slots, scope, scratch(&name));
        }
    }
    for (function_id, block) in pous.blocks() {
        let field_var_off = next;
        for field in copy_slots(lowering, block)? {
            slots.insert(ScopeId::Block(function_id), field, next);
            next = VarIndex::new(next.raw() + 1);
        }
        for (method_id, method) in pous.methods(block) {
            let scope = ScopeId::Method {
                block: function_id,
                method: method_id,
            };
            let frame = frame(lowering, method)?;
            let mut current = next;
            for var in frame.variables {
                slots.insert(scope, name(lowering, var)?, current);
                current = VarIndex::new(current.raw() + 1);
            }
            // A method has a result slot whether it returns a result or not.
            if let Some(result) = frame.result {
                slots.insert(scope, name(lowering, result)?, current);
            }
            current = VarIndex::new(current.raw() + 1);
            // A method's run spans its block's copy slots too, and the next
            // method's starts that many slots on (`block.rs`).
            next = VarIndex::new(next.raw() + current.raw() - field_var_off.raw());
        }
    }
    Some(slots)
}

/// The variables of the globals that have storage, in the execution
/// model's order, the compiler-provided ones first. A resource's
/// `VAR_GLOBAL` has none yet (`execution.rs`).
fn globals(lowering: &PartialProgram) -> Vec<VarId> {
    let bindings = lowering.bindings();
    lowering
        .execution()
        .globals()
        .filter(|(_, global)| match global.kind {
            GlobalKind::System(_)
            | GlobalKind::Declared(GlobalScope::TopLevel | GlobalScope::Configuration) => true,
            GlobalKind::Declared(GlobalScope::Resource) => false,
        })
        .filter_map(|(id, _)| bindings.global(id))
        .collect()
}

/// Records the variable `var` in `scope`, the globals' or the program's,
/// as [`insert_named`] does.
fn insert_variable(
    slots: &mut Scoped<VarIndex>,
    scope: ScopeId,
    lowering: &PartialProgram,
    var: VarId,
) -> Option<()> {
    let ty = lowering.variables().get(var)?.ty();
    insert_named(slots, scope, lowering.types(), name(lowering, var)?, ty);
    Some(())
}

/// Records `name`, of the type `ty`, in `scope`, the globals' or the
/// program's, at the next slot the scope counts, and its scratch slot after
/// it if it takes one.
fn insert_named(
    slots: &mut Scoped<VarIndex>,
    scope: ScopeId,
    types: &TypeTable,
    name: Id,
    ty: TypeId,
) {
    let scratch_slot = needs_scratch(types, ty).then(|| scratch(&name));
    insert_counted(slots, scope, name);
    if let Some(scratch_slot) = scratch_slot {
        insert_counted(slots, scope, scratch_slot);
    }
}

/// Records `name` in `scope` at the next slot the body of `scope` counts,
/// as the walk of the AST declarations numbers the globals, the program's
/// variables and the scratch slots.
fn insert_counted(slots: &mut Scoped<VarIndex>, scope: ScopeId, name: Id) {
    let index = VarIndex::new(names::slot_count(slots, scope) as u16);
    slots.insert(scope, name, index);
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
fn instance_type(types: &TypeTable, pou: PouId) -> Option<TypeId> {
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

/// The names of the copy slots of the function block `pou`: its own
/// inputs, then its outputs, then its locals, each in declaration order.
fn copy_slots(lowering: &PartialProgram, pou: PouId) -> Option<Vec<Id>> {
    let types = lowering.types();
    let fields = types.fields(instance_type(types, pou)?)?;
    let mut own = Vec::new();
    for declaration in lowering.declarations(pou)? {
        match declaration {
            Declaration::Field(field) => own.push(fields.get(field.index() as usize)?),
            Declaration::Variable(_) | Declaration::External(_) => {}
        }
    }
    let mut names = Vec::with_capacity(own.len());
    for section in [Section::Input, Section::Output, Section::Local] {
        names.extend(
            own.iter()
                .filter(|field| field.section == Some(section))
                .map(|field| name_of(field)),
        );
    }
    Some(names)
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
