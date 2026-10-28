//! Addressing a lowered place in the storage codegen lays out.
//!
//! A lowered place is stored where codegen's own layout puts what the place
//! names, which the layout keeps by its lowered id. So a lowered place is
//! addressed as the [`Access`] that names it, with the lowered subscripts, which
//! [`Place::resolve`] resolves. An element is then addressed through the
//! layout's own code, bounds check included, and what a reference variable
//! refers to through its indirect load and store. What a function's
//! `VAR_IN_OUT` parameter refers to is named as the parameter itself, which
//! codegen's layout reaches through the reference the parameter's slot
//! holds, and what its `VAR_OUTPUT` parameter refers to as the parameter
//! too, whose slot codegen's layout keeps the output in.

use ironplc_ir::lowered::Program;
use ironplc_ir::lowered::{
    Const, Expr, FieldIdx, Nullability, Owner, Place as LoweredPlace, Projection, TypeId, TypeKind,
    TypeTable, Variable as LoweredVariable,
};
use ironplc_ir::{Diagnostic, Label};
use ironplc_ir::{Id, SourceSpan};

use super::expr::emit_expr;
use super::{constant, instance_field, op_type, stored_field, stored_variable};
use crate::access::{Access, Field, Index, Named};
use crate::compile::{CompileContext, OpWidth, Signedness, VarTypeInfo};
use crate::compile_place::Place;
use crate::compile_string_place::StringPlace;
use crate::compile_subscript::Subscript;
use crate::emit::Emitter;

/// A subscript of a lowered place.
#[derive(Clone, Copy)]
pub(super) struct LoweredSubscript<'l> {
    lowering: &'l Program,
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

/// A lowered place, as the layout addresses it.
type LoweredAccess<'l> = Access<LoweredSubscript<'l>>;

/// Where codegen's layout stores `place`, as one value in one slot.
pub(super) fn resolve<'l>(
    lowering: &'l Program,
    ctx: &CompileContext,
    place: &'l LoweredPlace,
    span: &SourceSpan,
) -> Result<Place<LoweredSubscript<'l>>, Diagnostic> {
    let access = access(lowering, ctx, place).ok_or_else(|| no_storage(span))?;
    storage(lowering, ctx, place, &access)
}

/// Where the value a read of `place` loads is, or a store to it goes.
pub(super) enum Load<'l> {
    /// In this storage.
    Stored(Place<LoweredSubscript<'l>>),
    /// In what the reference at this location refers to: `refs[0]^`, whose
    /// reference is in an array element, and `pp^^`, whose reference `pp^`
    /// is what a reference variable refers to. The reference is loaded
    /// from its location and then followed (REQ-LOW-codegen-203).
    Referenced(Box<Load<'l>>),
}

impl<'l> Load<'l> {
    /// Pushes the value at this location: the storage's, or the one the
    /// loaded reference refers to.
    pub(super) fn emit_load(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
    ) -> Result<(), Diagnostic> {
        match self {
            Load::Stored(storage) => storage.emit_load(emitter, ctx),
            Load::Referenced(reference) => {
                reference.emit_load(emitter, ctx)?;
                emitter.emit_load_indirect();
                Ok(())
            }
        }
    }

    /// Pops a value that already fits the place's type, as every value the
    /// lowered program stores does (REQ-LOW-lowering-046), and stores it
    /// at this location: in the storage, or in what the loaded reference
    /// refers to, which `STORE_INDIRECT` takes above the value.
    pub(super) fn emit_store_fitting(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
    ) -> Result<(), Diagnostic> {
        match self {
            Load::Stored(storage) => storage.emit_store_fitting(emitter, ctx),
            Load::Referenced(reference) => {
                reference.emit_load(emitter, ctx)?;
                emitter.emit_store_indirect();
                Ok(())
            }
        }
    }

    /// The storage the location is reached from: the one that holds the
    /// value, or the one the first reference followed to it is in.
    pub(super) fn storage(&self) -> &Place<LoweredSubscript<'l>> {
        match self {
            Load::Stored(storage) => storage,
            Load::Referenced(reference) => reference.storage(),
        }
    }
}

/// What this backend knows a reference as: a 64-bit slot, as a reference
/// variable is (`specs/design/ref-to.md`, "VarTypeInfo Integration").
const REFERENCE_INFO: VarTypeInfo = VarTypeInfo {
    op_width: OpWidth::W64,
    signedness: Signedness::Unsigned,
    storage_bits: 64,
};

/// Where the value a read of `place` loads is. What a reference that is not
/// in a variable refers to is read by loading the reference and then what it
/// refers to.
pub(super) fn resolve_load<'l>(
    lowering: &'l Program,
    ctx: &CompileContext,
    place: &'l LoweredPlace,
    span: &SourceSpan,
) -> Result<Load<'l>, Diagnostic> {
    let access = access(lowering, ctx, place).ok_or_else(|| no_storage(span))?;
    let Access::Deref(deref) = &access else {
        return storage(lowering, ctx, place, &access).map(Load::Stored);
    };
    // A reference in a variable keeps the indirect address that
    // `resolve` gives it.
    if let Access::Named(_) = deref.as_ref() {
        return storage(lowering, ctx, place, &access).map(Load::Stored);
    }
    Ok(Load::Referenced(Box::new(reference_load(
        lowering, ctx, deref,
    )?)))
}

/// Where the reference `reference` is, to be loaded and followed: in the
/// storage `refs[0]` or `s.r` names, or, for `pp^`, in what the reference
/// variable `pp` refers to, which is loaded and followed in turn
/// (REQ-LOW-codegen-203).
fn reference_load<'l>(
    lowering: &'l Program,
    ctx: &CompileContext,
    reference: &LoweredAccess<'l>,
) -> Result<Load<'l>, Diagnostic> {
    let Access::Deref(inner) = reference else {
        return Place::resolve(lowering.types(), ctx, reference).map(Load::Stored);
    };
    if let Access::Named(_) = inner.as_ref() {
        return Place::through_reference(ctx, inner, Some(REFERENCE_INFO)).map(Load::Stored);
    }
    Ok(Load::Referenced(Box::new(reference_load(
        lowering, ctx, inner,
    )?)))
}

/// Where codegen's layout stores `place`, which `access` names.
fn storage<'l>(
    lowering: &Program,
    ctx: &CompileContext,
    place: &LoweredPlace,
    access: &LoweredAccess<'l>,
) -> Result<Place<LoweredSubscript<'l>>, Diagnostic> {
    // The whole of what a reference variable refers to: `p^`.
    if let Access::Deref(deref) = access {
        let target = type_info(lowering.types(), place.ty());
        return Place::through_reference(ctx, deref, target);
    }
    Place::resolve(lowering.types(), ctx, access)
}

/// The variable `place` is the whole of, as this backend's layout keeps
/// it: a variable, a field of the current instance, or what a `VAR_IN_OUT`
/// parameter refers to. `None` for any other place.
pub(super) fn variable_name(
    lowering: &Program,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> Option<Named> {
    let Access::Named(name) = access(lowering, ctx, place)? else {
        return None;
    };
    Some(name)
}

/// The variable `place` is a field of, through fields only, as this
/// backend's layout keeps it, and those fields, outermost first: `h.og.ob`
/// is `h` with `og` and `ob`. A whole variable is itself with no fields.
/// `None` for any other place.
pub(super) fn field_path(
    lowering: &Program,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> Option<(Named, Vec<FieldIdx>)> {
    let mut access = access(lowering, ctx, place)?;
    let mut fields = Vec::new();
    loop {
        match access {
            Access::Named(name) => {
                fields.reverse();
                return Some((name, fields));
            }
            Access::Field(field) => {
                fields.push(field.idx);
                access = *field.record;
            }
            Access::Index(_) | Access::Deref(_) => return None,
        }
    }
}

/// The reference variable `place` is what it refers to, `p^`, as this
/// backend's layout keeps it. `None` for any other place, and for a
/// reference in a `VAR_IN_OUT` parameter, whose slot holds a reference to
/// the reference.
pub(super) fn referenced_by(
    lowering: &Program,
    ctx: &CompileContext,
    place: &LoweredPlace,
) -> Option<Named> {
    let Access::Deref(deref) = access(lowering, ctx, place)? else {
        return None;
    };
    let Access::Named(name) = *deref else {
        return None;
    };
    (!ctx.is_in_out(&name)).then_some(name)
}

/// Whether `expr` reads a field of an instance of a standard function block:
/// an instance this backend reaches as a variable, of no user-defined block's
/// type.
///
/// Until lowering generates the standard blocks (issue 2288), the VM's own
/// block gives such a field its value, and may leave it outside the field's
/// type: an `_INT` counter counts its `CV` in 32 bits ("Standard function
/// blocks" in `specs/design/lowered-program.md`). So the lowered program's
/// proof that a read fits the type it reads (REQ-LOW-lowering-046) does not
/// hold for one, and a store of it is narrowed.
pub(super) fn reads_standard_block_field(
    lowering: &Program,
    ctx: &CompileContext,
    expr: &Expr,
) -> bool {
    let Some(place) = super::read(expr) else {
        return false;
    };
    let Some(Access::Field(field)) = access(lowering, ctx, place) else {
        return false;
    };
    let Access::Named(record) = field.record.as_ref() else {
        return false;
    };
    ctx.fb_instance(record).is_some_and(|instance| {
        ctx.user_fb_types
            .values()
            .all(|block| block.type_id != instance.type_id)
    })
}

/// The slot of an instance that this backend reaches as a variable, other
/// than the current instance, that holds the field `place` is (`f.si`), whatever the
/// field's type. `None` for any other place.
///
/// This backend's layout keeps no data region for a string, structure or
/// array field of such an instance: its slot holds the index of a temporary
/// buffer for a string (issue 1753) and the region offset of the value stored
/// for a structure or an array.
pub(super) fn instance_slot<'l>(
    lowering: &'l Program,
    ctx: &CompileContext,
    place: &'l LoweredPlace,
) -> Option<Place<LoweredSubscript<'l>>> {
    let Access::Field(field) = access(lowering, ctx, place)? else {
        return None;
    };
    Place::fb_field(ctx, &field).ok()?
}

/// Where codegen's layout stores the string `place`: a string variable, an
/// element of a string array or a string field of a structure, or the string
/// a reference variable refers to, `rs^` or `rsa^[i]`, reached through the
/// reference.
pub(super) fn resolve_string<'l>(
    lowering: &'l Program,
    ctx: &CompileContext,
    place: &'l LoweredPlace,
    span: &SourceSpan,
) -> Result<StringPlace<LoweredSubscript<'l>>, Diagnostic> {
    let access = access(lowering, ctx, place).ok_or_else(|| no_storage(span))?;
    match StringPlace::through_reference(ctx, &access) {
        Some(string) => Ok(string),
        None => {
            StringPlace::resolve(lowering.types(), ctx, &access)?.ok_or_else(|| no_storage(span))
        }
    }
}

/// The internal error for a place `can_emit` should have rejected.
fn no_storage(span: &SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(span.clone(), "A lowered place has no storage"))
}

/// The access that names `place` in this backend's layout, with the lowered
/// subscripts; `None` for a place this backend does not reach yet.
fn access<'l>(
    lowering: &'l Program,
    ctx: &CompileContext,
    place: &'l LoweredPlace,
) -> Option<LoweredAccess<'l>> {
    let types = lowering.types();
    let root = lowering.variables().get(place.root())?;
    let (mut access, mut ty, path) = match instance_field(lowering, root.ty(), place.path()) {
        // A field of the current instance, which codegen copies into a slot
        // of the field's name.
        Some((instance, field, rest)) => {
            let declared = types.fields(instance)?.get(field.index() as usize)?;
            (
                Access::Named(stored_field(lowering, ctx, instance, field)?),
                declared.ty,
                rest,
            )
        }
        None => (
            Access::Named(stored_variable(lowering, ctx, place.root())?),
            root.ty(),
            place.path(),
        ),
    };
    for projection in path {
        access = match projection {
            Projection::Field(field) => {
                let declared = types.fields(ty)?.get(field.index() as usize)?;
                ty = declared.ty;
                Access::Field(Field {
                    record: Box::new(access),
                    idx: *field,
                    field: Id::from(&declared.name.to_string()),
                })
            }
            Projection::Index(exprs) => {
                let Some(TypeKind::Array { element, .. }) = types.kind(ty) else {
                    return None;
                };
                ty = *element;
                Access::Index(Index {
                    array: Box::new(access),
                    subscripts: exprs
                        .iter()
                        .map(|expr| LoweredSubscript { lowering, expr })
                        .collect(),
                })
            }
            Projection::Deref => {
                let (target, nullability) = types.reference(ty)?;
                ty = target;
                match nullability {
                    Nullability::Nullable => Access::Deref(Box::new(access)),
                    Nullability::NonNull => by_reference(ctx, root, access)?,
                }
            }
        };
    }
    Some(access)
}

/// `access`, when it names a parameter of the function being compiled,
/// `root`, that refers to a place of the caller: a `VAR_IN_OUT`, whose slot
/// holds a reference this backend follows, or a `VAR_OUTPUT`, whose value
/// this backend keeps in the parameter's own slot for the call to read once
/// the function returns (ADR-0046). They are the only non-null references
/// this backend follows outside a body's instance parameter.
fn by_reference<'l>(
    ctx: &CompileContext,
    root: &LoweredVariable,
    access: LoweredAccess<'l>,
) -> Option<LoweredAccess<'l>> {
    let Access::Named(name) = &access else {
        return None;
    };
    if ctx.in_out_ref_slot(name).is_some() {
        return Some(access);
    }
    // Lowering makes no other non-null reference a variable of a function
    // or method than its `VAR_IN_OUT`s and `VAR_OUTPUT`s
    // (REQ-LOW-lowering-035).
    let Owner::Pou(owner) = root.owner() else {
        return None;
    };
    let compiled = super::is_current_body(ctx, owner);
    (compiled && ctx.has_slot(name)).then_some(access)
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

/// The type a slot holding a value of the lowered type `ty` is operated
/// at. A `BOOL` holds one bit of value, which also marks it for the
/// conversion path (`type_info.rs`).
pub(crate) fn slot_type(types: &TypeTable, ty: TypeId) -> Option<VarTypeInfo> {
    match types.kind(ty) {
        Some(TypeKind::Bool) => Some(VarTypeInfo {
            storage_bits: 1,
            ..type_info(types, ty)?
        }),
        _ => type_info(types, ty),
    }
}
