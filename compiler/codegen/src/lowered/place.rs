//! Addressing a lowered place in the storage codegen lays out.
//!
//! While both routes exist, a lowered place is stored where codegen's own
//! layout puts what the place names, and that layout is found by name,
//! through [`Place::resolve`] on a variable reference. So a lowered place is
//! addressed by rebuilding the variable reference it stands for, with a
//! numbered placeholder for each subscript, resolving that, and putting the
//! lowered subscripts in the placeholders' places. Both routes then address
//! an element through the same code, bounds check included, and what a
//! reference variable refers to through the same indirect load and store.
//! What a function's `VAR_IN_OUT` parameter refers to is rebuilt as the
//! parameter's own name, which codegen's layout reaches through the
//! reference the parameter's slot holds, and what its `VAR_OUTPUT` parameter
//! refers to as the parameter's own name too, whose slot codegen's layout
//! keeps the output in.

use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{
    ArrayVariable, DerefVariable, Expr as AstExpr, ExprKind as AstExprKind, NamedVariable,
    StructuredVariable, SymbolicVariableKind, Variable,
};
use ironplc_ir::lowered::{
    Const, Expr, Nullability, Owner, Place as LoweredPlace, Projection, TypeId, TypeKind,
    TypeTable, Variable as LoweredVariable,
};
use ironplc_lowering::PartialProgram;

use super::expr::emit_expr;
use super::{constant, instance_field, op_type, seen_name};
use crate::compile::{CompileContext, OpWidth, VarTypeInfo};
use crate::compile_place::Place;
use crate::compile_string_place::StringPlace;
use crate::compile_subscript::Subscript;
use crate::emit::Emitter;

/// A subscript of a lowered place.
pub(super) struct LoweredSubscript<'l> {
    lowering: &'l PartialProgram,
    expr: &'l Expr,
}

impl Subscript for LoweredSubscript<'_> {
    fn literal(&self) -> Option<i32> {
        match constant(self.expr)? {
            Const::I32(value) => Some(value),
            Const::Bool(_) | Const::I64(_) | Const::F32(_) | Const::F64(_) => None,
        }
    }

    fn emit(&self, emitter: &mut Emitter, ctx: &mut CompileContext) -> Result<(), Diagnostic> {
        emit_expr(emitter, ctx, self.lowering, self.expr)
    }
}

/// Where codegen's layout stores `place`, as one value in one slot.
pub(super) fn resolve<'l>(
    lowering: &'l PartialProgram,
    ctx: &CompileContext,
    place: &'l LoweredPlace,
    span: &SourceSpan,
) -> Result<Place<LoweredSubscript<'l>>, Diagnostic> {
    let mut subscripts = Vec::new();
    let reference =
        reference(lowering, ctx, place, &mut subscripts).ok_or_else(|| no_storage(span))?;
    storage(lowering, ctx, place, reference, &subscripts, span)
}

/// Where the value a read of `place` loads is.
pub(super) enum Load<'l> {
    /// In this storage.
    Stored(Place<LoweredSubscript<'l>>),
    /// In what the reference this storage holds refers to: `refs[0]^`, whose
    /// reference is not in a variable.
    Referenced(Place<LoweredSubscript<'l>>),
}

/// Where the value a read of `place` loads is. What a reference that is not
/// in a variable refers to is read by loading the reference and then what it
/// refers to, as the AST route reads `e^`. A store to it is not: the AST
/// route does not compile one.
pub(super) fn resolve_load<'l>(
    lowering: &'l PartialProgram,
    ctx: &CompileContext,
    place: &'l LoweredPlace,
    span: &SourceSpan,
) -> Result<Load<'l>, Diagnostic> {
    let mut subscripts = Vec::new();
    let reference =
        reference(lowering, ctx, place, &mut subscripts).ok_or_else(|| no_storage(span))?;
    let SymbolicVariableKind::Deref(deref) = reference else {
        return storage(lowering, ctx, place, reference, &subscripts, span).map(Load::Stored);
    };
    // A reference in a variable keeps the indirect address that
    // `resolve` gives it.
    if let SymbolicVariableKind::Named(_) = *deref.variable {
        let reference = SymbolicVariableKind::Deref(deref);
        return storage(lowering, ctx, place, reference, &subscripts, span).map(Load::Stored);
    }
    Place::resolve(ctx, &deref.variable)?
        .try_map_subscripts(|placeholder| {
            lowered_subscript(lowering, &subscripts, placeholder, span)
        })
        .map(Load::Referenced)
}

/// Where codegen's layout stores `place`, which `reference` stands for with
/// `subscripts` as its placeholders.
fn storage<'l>(
    lowering: &'l PartialProgram,
    ctx: &CompileContext,
    place: &'l LoweredPlace,
    reference: SymbolicVariableKind,
    subscripts: &[&'l Expr],
    span: &SourceSpan,
) -> Result<Place<LoweredSubscript<'l>>, Diagnostic> {
    // The whole of what a reference variable refers to: `p^`.
    if let SymbolicVariableKind::Deref(deref) = reference {
        let target = type_info(lowering.types(), place.ty());
        return Place::through_reference(ctx, &Variable::Symbolic(*deref.variable), target);
    }
    Place::resolve(ctx, &reference)?.try_map_subscripts(|placeholder| {
        lowered_subscript(lowering, subscripts, placeholder, span)
    })
}

/// The name of the variable `place` is the whole of, as this backend's
/// layout knows it: a variable, a field of the current instance, or what a
/// `VAR_IN_OUT` parameter refers to. `None` for any other place.
pub(super) fn variable_name(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> Option<Id> {
    let SymbolicVariableKind::Named(named) = reference(lowering, ctx, place, &mut Vec::new())?
    else {
        return None;
    };
    Some(named.name)
}

/// The name of the variable `place` is a field of, through fields only, as
/// this backend's layout knows it, and the names of those fields, outermost
/// first: `h.og.ob` is `h` with `og` and `ob`. A whole variable is its name
/// with no fields. `None` for any other place.
pub(super) fn field_path(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> Option<(Id, Vec<Id>)> {
    let mut reference = reference(lowering, ctx, place, &mut Vec::new())?;
    let mut fields = Vec::new();
    loop {
        match reference {
            SymbolicVariableKind::Named(named) => {
                fields.reverse();
                return Some((named.name, fields));
            }
            SymbolicVariableKind::Structured(structured) => {
                fields.push(structured.field);
                reference = *structured.record;
            }
            SymbolicVariableKind::Array(_)
            | SymbolicVariableKind::Deref(_)
            | SymbolicVariableKind::BitAccess(_)
            | SymbolicVariableKind::PartialAccess(_)
            | SymbolicVariableKind::SelfRef(_) => return None,
        }
    }
}

/// The name of the reference variable `place` is what it refers to, `p^`,
/// as this backend's layout knows it. `None` for any other place, and for a
/// reference in a `VAR_IN_OUT` parameter, whose slot holds a reference to
/// the reference.
pub(super) fn referenced_by(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> Option<Id> {
    let SymbolicVariableKind::Deref(deref) = reference(lowering, ctx, place, &mut Vec::new())?
    else {
        return None;
    };
    let SymbolicVariableKind::Named(named) = *deref.variable else {
        return None;
    };
    (!ctx.is_in_out(&named.name)).then_some(named.name)
}

/// Whether `expr` reads a field of an instance of a standard function block:
/// an instance this backend reaches by name, of no user-defined block's type.
///
/// Until lowering generates the standard blocks (issue 2288), the VM's own
/// block gives such a field its value, and may leave it outside the field's
/// type: an `_INT` counter counts its `CV` in 32 bits ("Standard function
/// blocks" in `specs/design/lowered-program.md`). So the lowered program's
/// proof that a read fits the type it reads (REQ-LOW-lowering-046) does not
/// hold for one, and a store of it is narrowed, as the route that reads the
/// AST narrows every store.
pub(super) fn reads_standard_block_field(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    expr: &Expr,
) -> bool {
    let Some(place) = super::read(expr) else {
        return false;
    };
    let Some(SymbolicVariableKind::Structured(field)) =
        reference(lowering, ctx, place, &mut Vec::new())
    else {
        return false;
    };
    let SymbolicVariableKind::Named(record) = field.record.as_ref() else {
        return false;
    };
    ctx.fb_instance(&record.name).is_some_and(|instance| {
        ctx.user_fb_types
            .values()
            .all(|block| block.type_id != instance.type_id)
    })
}

/// The slot of an instance that this backend reaches by name, other than the
/// current instance, that holds the field `place` is (`f.si`), whatever the
/// field's type. `None` for any other place.
///
/// This backend's layout keeps no data region for a string, structure or
/// array field of such an instance: its slot holds what the route that reads
/// the AST stores there, the index of a temporary buffer for a string
/// (issue 1753) and the region offset of the value stored for a structure or
/// an array.
pub(super) fn instance_slot<'l>(
    lowering: &'l PartialProgram,
    ctx: &CompileContext,
    place: &'l LoweredPlace,
) -> Option<Place<LoweredSubscript<'l>>> {
    let SymbolicVariableKind::Structured(field) = reference(lowering, ctx, place, &mut Vec::new())?
    else {
        return None;
    };
    // An instance's field has no subscripts.
    Place::fb_field(ctx, &field)
        .ok()??
        .try_map_subscripts(|_| Err(()))
        .ok()
}

/// Where codegen's layout stores the string `place`: a string variable, an
/// element of a string array or a string field of a structure, as the AST
/// route finds it, or the string a reference variable refers to, `rs^` or
/// `rsa^[i]`, reached through the reference.
pub(super) fn resolve_string<'l>(
    lowering: &'l PartialProgram,
    ctx: &CompileContext,
    place: &'l LoweredPlace,
    span: &SourceSpan,
) -> Result<StringPlace<LoweredSubscript<'l>>, Diagnostic> {
    let mut subscripts = Vec::new();
    let reference =
        reference(lowering, ctx, place, &mut subscripts).ok_or_else(|| no_storage(span))?;
    let reference = Variable::Symbolic(reference);
    let string = match StringPlace::through_reference(ctx, &reference) {
        Some(string) => string,
        None => StringPlace::resolve(ctx, &reference)?.ok_or_else(|| no_storage(span))?,
    };
    string.try_map_subscripts(|placeholder| {
        lowered_subscript(lowering, &subscripts, placeholder, span)
    })
}

/// The lowered subscript the literal `placeholder` stands for: the one at
/// its position in `subscripts`.
fn lowered_subscript<'l>(
    lowering: &'l PartialProgram,
    subscripts: &[&'l Expr],
    placeholder: &AstExpr,
    span: &SourceSpan,
) -> Result<LoweredSubscript<'l>, Diagnostic> {
    let expr = placeholder
        .literal()
        .and_then(|position| subscripts.get(usize::try_from(position).ok()?))
        .ok_or_else(|| no_storage(span))?;
    Ok(LoweredSubscript { lowering, expr })
}

/// The internal error for a place `can_emit` should have left to the AST
/// route.
fn no_storage(span: &SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(span.clone(), "A lowered place has no storage"))
}

/// The variable reference `place` stands for, with the subscript at
/// position `k` of `subscripts` as the literal `k`; `None` for a place this
/// backend does not reach yet.
fn reference<'l>(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    place: &'l LoweredPlace,
    subscripts: &mut Vec<&'l Expr>,
) -> Option<SymbolicVariableKind> {
    let types = lowering.types();
    let root = lowering.variables().get(place.root())?;
    let (mut reference, mut ty, path) = match instance_field(lowering, root.ty(), place.path()) {
        // A field of the current instance, which codegen copies into a slot
        // of the field's name.
        Some((instance, field, rest)) => {
            let declared = types.fields(instance)?.get(field.index() as usize)?;
            (
                named(ctx.lowered_ids.field(instance, field)?),
                declared.ty,
                rest,
            )
        }
        None => (
            named(seen_name(ctx, place.root())?),
            root.ty(),
            place.path(),
        ),
    };
    for projection in path {
        reference = match projection {
            Projection::Field(field) => {
                let declared = types.fields(ty)?.get(field.index() as usize)?;
                ty = declared.ty;
                SymbolicVariableKind::Structured(StructuredVariable {
                    record: Box::new(reference),
                    field: Id::from(&declared.name.to_string()),
                })
            }
            Projection::Index(exprs) => {
                let Some(TypeKind::Array { element, .. }) = types.kind(ty) else {
                    return None;
                };
                ty = *element;
                let placeholders = exprs
                    .iter()
                    .map(|expr| {
                        let position = subscripts.len();
                        subscripts.push(expr);
                        AstExpr::new(AstExprKind::integer_literal(&position.to_string()))
                    })
                    .collect();
                SymbolicVariableKind::Array(ArrayVariable {
                    subscripted_variable: Box::new(reference),
                    subscripts: placeholders,
                })
            }
            Projection::Deref => {
                let (target, nullability) = types.reference(ty)?;
                ty = target;
                match nullability {
                    Nullability::Nullable => SymbolicVariableKind::Deref(DerefVariable {
                        variable: Box::new(reference),
                    }),
                    Nullability::NonNull => by_reference(ctx, root, reference)?,
                }
            }
        };
    }
    Some(reference)
}

/// `reference`, when it names a parameter of the function being compiled,
/// `root`, that refers to a place of the caller: a `VAR_IN_OUT`, whose slot
/// holds a reference this backend follows, or a `VAR_OUTPUT`, whose value
/// this backend keeps in the parameter's own slot for the call to read once
/// the function returns (ADR-0046). They are the only non-null references
/// this backend follows outside a body's instance parameter.
fn by_reference(
    ctx: &CompileContext,
    root: &LoweredVariable,
    reference: SymbolicVariableKind,
) -> Option<SymbolicVariableKind> {
    let SymbolicVariableKind::Named(named) = &reference else {
        return None;
    };
    if ctx.in_out_ref_slot(&named.name).is_some() {
        return Some(reference);
    }
    // Lowering makes no other non-null reference a variable of a function
    // or method than its `VAR_IN_OUT`s and `VAR_OUTPUT`s
    // (REQ-LOW-lowering-035).
    let Owner::Pou(owner) = root.owner() else {
        return None;
    };
    let compiled = super::is_current_body(ctx, owner);
    (compiled && ctx.has_slot(&named.name)).then_some(reference)
}

/// What this backend knows a value of the lowered type `ty` as: the type it
/// is operated at, and the width a store narrows it to.
pub(crate) fn type_info(types: &TypeTable, ty: TypeId) -> Option<VarTypeInfo> {
    let (op_width, signedness) = op_type(types.operation_type(ty)?)?;
    let storage_bits = match types.narrowed_storage(ty) {
        Some(narrow) => narrow.bits(),
        None => match op_width {
            OpWidth::W32 | OpWidth::F32 => 32,
            OpWidth::W64 | OpWidth::F64 => 64,
        },
    };
    Some(VarTypeInfo {
        op_width,
        signedness,
        storage_bits,
    })
}

fn named(name: &Id) -> SymbolicVariableKind {
    SymbolicVariableKind::Named(NamedVariable { name: name.clone() })
}
