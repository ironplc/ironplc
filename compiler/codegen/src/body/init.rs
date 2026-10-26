//! `init` from the lowered program (REQ-LOW-codegen-200): for each global,
//! then each field of the program instance, in declaration order, the
//! setup of the storage this backend's layout gives it, then the statements
//! of the lowered `init` that store its starting value.
//!
//! The variable table and the data region start cleared, so a store that
//! leaves its storage cleared is left out: a cleared element of an array,
//! member of an instance or empty string anywhere, and a cleared scalar
//! variable of an elementary type. A store left out is not one this backend
//! need reach. A field of a structure, reached through structures
//! alone, is stored whatever its value, and so is an enumeration, a
//! subrange or a reference variable.
//!
//! A starting value lowering does not express yet, or a store this backend
//! does not emit yet, is not supported yet (P9999), at the declaration or
//! the store.

use ironplc_ir::lowered::{
    Const, Expr, ExprKind, Owner, Place, PouId, Projection, Stmt, StmtKind, StrExprKind, TypeId,
    TypeKind,
};
use ironplc_ir::lowered::{Declaration, Program};
use ironplc_ir::{Diagnostic, Label};

use crate::access::{Named, Stored};
use crate::compile::CompileContext;
use crate::emit::Emitter;
use crate::layout::declarations::{globals, instance_type, named, named_field};

/// Emits `init`, the declared globals and then the program's own
/// declarations, from the lowered program, in the program's scope: the one
/// current outside the other bodies.
pub(crate) fn emit_init(emitter: &mut Emitter, ctx: &mut CompileContext) -> Result<(), Diagnostic> {
    let lowering = ctx.lowering()?;
    match super::current_pou(ctx) {
        Some(program) => emit(emitter, ctx, lowering, program),
        None => Err(Diagnostic::internal_error()),
    }
}

/// The declaration of a global or of a field of the program instance, and
/// the statements of `init` that store its starting value.
struct Declared<'l> {
    stored: Stored,
    stores: Vec<&'l Stmt>,
}

fn emit(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Program,
    program: PouId,
) -> Result<(), Diagnostic> {
    // A program's `VAR_TEMP` is not in `init`: the program's prologue gives
    // it its starting value, at the head of every scan.
    let declared = declarations(lowering, ctx, lowering.init())?;
    let mut ordered = Vec::with_capacity(declared.len());
    for name in visited(lowering, program).ok_or_else(Diagnostic::internal_error)? {
        let stores = declared
            .iter()
            .find(|declared| declared.stored == name.stored);
        ordered.push((name, stores));
    }
    // Every store belongs to one of the declarations `init` visits.
    let unvisited = declared.iter().find(|declared| {
        !ordered
            .iter()
            .any(|(_, stores)| stores.is_some_and(|stores| std::ptr::eq(stores, *declared)))
    });
    if let Some(unvisited) = unvisited {
        return Err(Diagnostic::internal_error_at(Label::span(
            unvisited
                .stores
                .first()
                .map(|stmt| stmt.span().clone())
                .unwrap_or_default(),
            "A store of init is into no declaration init visits",
        )));
    }
    for (name, stores) in ordered {
        crate::compile_setup::set_up_storage(emitter, ctx, lowering.types(), &name)?;
        let Some(declared) = stores else {
            continue;
        };
        let stores: Vec<Stmt> = declared.stores.iter().map(|stmt| (*stmt).clone()).collect();
        super::stmt::emit_stmts(emitter, ctx, lowering, &stores)?;
    }
    Ok(())
}

/// The declarations `init` visits, in order: each declared global, then
/// each of the own variables and fields of `program`. A global the program
/// declares in its own interface is visited in both, and set up in both.
fn visited(lowering: &Program, program: PouId) -> Option<Vec<Named>> {
    let types = lowering.types();
    let program_type = instance_type(types, program)?;
    let mut visited = Vec::new();
    for (var, system) in globals(lowering) {
        if !system {
            visited.push(named(lowering, var)?);
        }
    }
    for declaration in lowering.declarations(program)? {
        match declaration {
            Declaration::Field(idx) => {
                let field = types.fields(program_type)?.get(idx.index() as usize)?;
                visited.push(named_field(program_type, *idx, field));
            }
            Declaration::Variable(var) => visited.push(named(lowering, *var)?),
            Declaration::External(_) => {}
        }
    }
    Some(visited)
}

/// The statements of `init` that are emitted ([`kept`]), by the declaration
/// each stores into, in the order they come; P9999 at a store this backend
/// does not emit yet. A store left out is not one this backend need reach.
fn declarations<'l>(
    lowering: &Program,
    ctx: &CompileContext,
    statements: &'l [Stmt],
) -> Result<Vec<Declared<'l>>, Diagnostic> {
    let mut declared: Vec<Declared> = Vec::new();
    for stmt in statements {
        let place = match stmt.kind() {
            StmtKind::Assign { place, .. } | StmtKind::AssignStr { place, .. } => place,
            StmtKind::AssignBits { .. }
            | StmtKind::Copy { .. }
            | StmtKind::FbCall { .. }
            | StmtKind::Call { .. }
            | StmtKind::If { .. }
            | StmtKind::Case { .. }
            | StmtKind::For { .. }
            | StmtKind::Loop { .. }
            | StmtKind::Exit(_)
            | StmtKind::Continue(_)
            | StmtKind::Return => return Err(super::unsupported_store(stmt)),
        };
        let Some((stored, ty)) = declaration(lowering, ctx, place) else {
            // A resource's global has no storage (REQ-EM-codegen-007), so
            // neither does what it starts with.
            if is_unstored_global(lowering, ctx, place) {
                continue;
            }
            return Err(Diagnostic::internal_error_at(Label::span(
                stmt.span().clone(),
                "A store of init is into no declaration the layout knows",
            )));
        };
        if !kept(lowering, ty, stmt) {
            continue;
        }
        if !super::can_emit(lowering, ctx, stmt) {
            return Err(super::unsupported_store(stmt));
        }
        match declared
            .iter_mut()
            .find(|declared| declared.stored == stored)
        {
            Some(declared) => declared.stores.push(stmt),
            None => declared.push(Declared {
                stored,
                stores: vec![stmt],
            }),
        }
    }
    Ok(declared)
}

/// Whether `place` is in a global the layout keeps no storage for.
fn is_unstored_global(lowering: &Program, ctx: &CompileContext, place: &Place) -> bool {
    lowering
        .variables()
        .get(place.root())
        .is_some_and(|root| root.owner() == Owner::Global)
        && !ctx.stores(&Stored::Var(place.root()))
}

/// The declaration `place` stores into, as the layout keeps it, and the
/// type of what it declares.
fn declaration(
    lowering: &Program,
    ctx: &CompileContext,
    place: &Place,
) -> Option<(Stored, TypeId)> {
    let root = lowering.variables().get(place.root())?;
    if let Some((instance, field, _)) = super::instance_field(lowering, root.ty(), place.path()) {
        let stored = Stored::Field(instance, field);
        let ty = lowering
            .types()
            .fields(instance)?
            .get(field.index() as usize)?
            .ty;
        return ctx.stores(&stored).then_some((stored, ty));
    }
    let stored = Stored::Var(place.root());
    ctx.stores(&stored).then_some((stored, root.ty()))
}

/// Whether the store `stmt`, into a declaration of the type `declared`, is
/// emitted: one that leaves its storage cleared is not.
fn kept(lowering: &Program, declared: TypeId, stmt: &Stmt) -> bool {
    let (place, clears) = match stmt.kind() {
        StmtKind::Assign { place, value } => (place, is_cleared(value)),
        StmtKind::AssignStr { place, value } => (
            place,
            matches!(value.kind(), StrExprKind::Literal(chars) if chars.is_empty()),
        ),
        StmtKind::AssignBits { .. }
        | StmtKind::Copy { .. }
        | StmtKind::FbCall { .. }
        | StmtKind::Call { .. }
        | StmtKind::If { .. }
        | StmtKind::Case { .. }
        | StmtKind::For { .. }
        | StmtKind::Loop { .. }
        | StmtKind::Exit(_)
        | StmtKind::Continue(_)
        | StmtKind::Return => return true,
    };
    if !clears {
        return true;
    }
    let types = lowering.types();
    let Some(root) = lowering.variables().get(place.root()) else {
        return true;
    };
    let path = match super::instance_field(lowering, root.ty(), place.path()) {
        Some((_, _, rest)) => rest,
        None => place.path(),
    };
    if path.is_empty() {
        // A whole variable, of a type that is always stored.
        return matches!(
            types.kind(declared),
            Some(TypeKind::Enumeration { .. } | TypeKind::Subrange { .. })
        );
    }
    // A field of a structure, reached through structures alone, is always
    // stored; a string field's header is part of the setup.
    let mut ty = declared;
    for projection in path {
        let Projection::Field(field) = projection else {
            return false;
        };
        if !matches!(types.kind(ty), Some(TypeKind::Structure { .. })) {
            return false;
        }
        let Some(field) = types
            .fields(ty)
            .and_then(|fields| fields.get(field.index() as usize))
        else {
            return false;
        };
        ty = field.ty;
    }
    !matches!(types.kind(ty), Some(TypeKind::String(_)))
}

/// Whether `value` is the constant that leaves storage cleared.
fn is_cleared(value: &Expr) -> bool {
    let ExprKind::Const(constant) = value.kind() else {
        return false;
    };
    match constant {
        Const::Bool(value) => !*value,
        Const::I32(value) => *value == 0,
        Const::I64(value) => *value == 0,
        // `-0.0` sets the sign bit, so its store is not left out.
        Const::F32(value) => value.to_bits() == 0,
        Const::F64(value) => value.to_bits() == 0,
    }
}
