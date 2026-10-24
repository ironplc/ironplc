//! `init` from the lowered program (REQ-LOW-codegen-200): for each global,
//! then each field of the program instance, in the order the AST's `init`
//! visits their declarations, the setup of the storage this backend's
//! layout gives it, then the statements of the lowered `init` that store
//! its starting value.
//!
//! The variable table and the data region start cleared, so a store that
//! leaves its storage cleared is left out where the AST's `init` leaves it
//! out (`compile_setup.rs`): a cleared element of an array, member of an
//! instance or empty string anywhere, and a cleared scalar variable of an
//! elementary type. A field of a structure, reached through structures
//! alone, is stored whatever its value, and so is an enumeration, a
//! subrange or a reference variable.
//!
//! An `init` lowering does not complete, or one with a statement this
//! backend does not emit yet, is left to the AST, whole, and so is the
//! `init` of a program whose `VAR_TEMP` the program's prologue does not
//! store from the lowered program (`prologue.rs`).

use ironplc_dsl::common::{VarDecl, VariableType};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_ir::lowered::{
    Const, Expr, ExprKind, Place, Projection, Stmt, StmtKind, StrExprKind, TypeId, TypeKind,
};
use ironplc_lowering::PartialProgram;

use crate::compile::CompileContext;
use crate::emit::Emitter;
use crate::layout::names::ScopeId;

/// Emits `init`, the globals `globals` declare and then the program's
/// variables `program` declares, from the lowered program, when it emits
/// every store; returns `false`, having emitted nothing, when `init` is left
/// to the AST.
pub(crate) fn try_emit_init(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    globals: &[VarDecl],
    program: &[VarDecl],
) -> Result<bool, Diagnostic> {
    let Some(route) = ctx.lowered_route.take() else {
        return Ok(false);
    };
    let caller = std::mem::replace(&mut ctx.scope, ScopeId::Program);
    let emitted = emit(emitter, ctx, route.lowering(), globals, program);
    ctx.scope = caller;
    ctx.lowered_route = Some(route);
    emitted
}

/// The declaration of a global or of a field of the program instance, and
/// the statements of `init` that store its starting value.
struct Declared<'l> {
    scope: ScopeId,
    name: Id,
    /// The type of what the declaration declares.
    ty: TypeId,
    stores: Vec<&'l Stmt>,
}

fn emit(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &PartialProgram,
    globals: &[VarDecl],
    program: &[VarDecl],
) -> Result<bool, Diagnostic> {
    // A program's `VAR_TEMP` is not in `init`: the program's prologue gives
    // it its starting value, at the head of every scan, or, when that is
    // left to the AST, the AST's `init` does, once.
    if program
        .iter()
        .any(|decl| decl.var_type == VariableType::VarTemp)
        && !super::prologue::emits(lowering, ctx, program)
    {
        return Ok(false);
    }
    let Some(statements) = lowering.init() else {
        return Ok(false);
    };
    let Some(declared) = declarations(lowering, ctx, statements) else {
        return Ok(false);
    };
    let order = globals
        .iter()
        .map(|decl| (ScopeId::Global, decl))
        .chain(program.iter().map(|decl| (ScopeId::Program, decl)));
    let mut ordered = Vec::with_capacity(declared.len());
    for (scope, decl) in order {
        let Some(id) = decl.identifier.symbolic_id() else {
            continue;
        };
        let stores = declared
            .iter()
            .find(|declared| declared.name == *id && same_scope(declared.scope, scope));
        ordered.push((decl, id, stores));
    }
    // Every store belongs to one of the declarations the AST's `init`
    // visits.
    let visited = ordered
        .iter()
        .filter(|(_, _, stores)| stores.is_some())
        .count();
    if visited != declared.len() {
        return Ok(false);
    }
    for (decl, id, stores) in ordered {
        crate::compile_setup::set_up_storage(emitter, ctx, id, &decl.identifier.span())?;
        let Some(declared) = stores else {
            continue;
        };
        let kept: Vec<Stmt> = declared
            .stores
            .iter()
            .filter(|stmt| kept(lowering, declared.ty, stmt))
            .map(|stmt| (*stmt).clone())
            .collect();
        super::stmt::emit_stmts(emitter, ctx, lowering, &kept)?;
    }
    Ok(true)
}

/// Whether a declaration recorded under `layout` is the one the AST's
/// `init` visits in `visited`: a global the program declares in its own
/// interface is laid out with the program's variables.
fn same_scope(layout: ScopeId, visited: ScopeId) -> bool {
    layout == visited || (layout == ScopeId::Program && visited == ScopeId::Global)
}

/// The statements of `init`, by the declaration each stores into, in the
/// order they come; `None` when one is not a store this backend emits, or
/// stores into no declaration the layout knows.
fn declarations<'l>(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    statements: &'l [Stmt],
) -> Option<Vec<Declared<'l>>> {
    let mut declared: Vec<Declared> = Vec::new();
    for stmt in statements {
        if !super::can_emit(lowering, ctx, stmt) {
            return None;
        }
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
            | StmtKind::Return => return None,
        };
        let (scope, name, ty) = declaration(lowering, ctx, place)?;
        match declared
            .iter_mut()
            .find(|declared| declared.scope == scope && declared.name == name)
        {
            Some(declared) => declared.stores.push(stmt),
            None => declared.push(Declared {
                scope,
                name,
                ty,
                stores: vec![stmt],
            }),
        }
    }
    Some(declared)
}

/// The declaration `place` stores into: its scope and name in the layout,
/// and the type of what it declares.
fn declaration(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    place: &Place,
) -> Option<(ScopeId, Id, TypeId)> {
    let root = lowering.variables().get(place.root())?;
    if let Some((instance, field, _)) = super::instance_field(lowering, root.ty(), place.path()) {
        let name = ctx.lowered_ids.field(instance, field)?.clone();
        let ty = lowering
            .types()
            .fields(instance)?
            .get(field.index() as usize)?
            .ty;
        return Some((ScopeId::Program, name, ty));
    }
    let (scope, name) = ctx.lowered_ids.variable(place.root())?;
    Some((*scope, name.clone(), root.ty()))
}

/// Whether the store `stmt`, into a declaration of the type `declared`, is
/// emitted: one that leaves its storage cleared is not, where the AST's
/// `init` leaves it out.
fn kept(lowering: &PartialProgram, declared: TypeId, stmt: &Stmt) -> bool {
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
