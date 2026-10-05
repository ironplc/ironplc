//! Addressing a place: the storage that holds one value in a single slot,
//! such as the target of an assignment (`a[i] := v`) or the base of a bit
//! access (`x.3`) or a partial access (`x.%B1`).
//!
//! [`Place::resolve`] works out where the value is from the variable
//! reference. [`Place::emit_load`] and [`Place::emit_store`] then emit the
//! load and the store for that shape, so a caller that reads, modifies and
//! writes the value does not depend on the shape.

use ironplc_container::{SlotIndex, VarIndex};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, SymbolicVariableKind};

use super::compile::{CompileContext, OpType, VarTypeInfo, DEFAULT_OP_TYPE};
use super::compile_array::{
    emit_flat_index, resolve_symbolic_access, ArrayVarInfo, DimensionInfo, ResolvedAccess,
};
use super::compile_expr::{emit_load_in_out, emit_load_var, emit_store_var, emit_truncation};
use super::compile_struct::{resolve_struct_field_access, var_type_info_for_field};
use crate::emit::Emitter;

/// A place that holds one value in a single slot.
pub(crate) struct Place<'ast> {
    address: Address<'ast>,
    /// The type of the value the place holds. `None` only for a variable the
    /// context records no type for, which is loaded and stored at the default
    /// type and not truncated.
    type_info: Option<VarTypeInfo>,
    /// The variable reference, for a diagnostic about its subscripts.
    span: SourceSpan,
}

/// Where a [`Place`] is, which decides the opcodes that load and store it.
enum Address<'ast> {
    /// A variable: `LOAD_VAR` and `STORE_VAR`.
    Variable(VarIndex),
    /// A `VAR_IN_OUT` parameter. Its slot holds a reference to the caller's
    /// variable: `LOAD_INDIRECT` and `STORE_INDIRECT` through it.
    InOut(VarIndex),
    /// An element of the slot array `var_index` holds: an array, or a
    /// structure laid out as one. `LOAD_ARRAY` and `STORE_ARRAY`, or their
    /// `_DEREF` forms when `var_index` holds a reference to the array.
    Element {
        var_index: VarIndex,
        desc_index: u16,
        through_ref: bool,
        index: ElementIndex<'ast>,
    },
}

/// The index of an [`Address::Element`] within its slot array.
enum ElementIndex<'ast> {
    /// A field of a structure, at a fixed slot.
    Slot(SlotIndex),
    /// An array element, at the flat index of `subscripts`. `offset` is the
    /// slot the array starts at when it is part of a structure (`s.arr[i]`)
    /// or is an array of structures (`a[i].field`).
    Subscripts {
        dimensions: Vec<DimensionInfo>,
        subscripts: Vec<&'ast Expr>,
        offset: Option<SlotIndex>,
    },
}

impl<'ast> Place<'ast> {
    /// Resolves the place that `base` names.
    pub(crate) fn resolve(
        ctx: &CompileContext,
        base: &'ast SymbolicVariableKind,
    ) -> Result<Self, Diagnostic> {
        let span = base.span();

        // A field at a fixed offset in a structure. In `a[i].field` the record
        // is an element of an array of structures, which resolves below.
        if let SymbolicVariableKind::Structured(structured) = base {
            if !matches!(structured.record.as_ref(), SymbolicVariableKind::Array(_)) {
                let (var_index, desc_index, slot, _op_type, field_type) =
                    resolve_struct_field_access(ctx, structured)?;
                return Ok(Place {
                    address: Address::Element {
                        var_index,
                        desc_index,
                        through_ref: false,
                        index: ElementIndex::Slot(slot),
                    },
                    type_info: var_type_info_for_field(&field_type),
                    span,
                });
            }
        }

        let name = match base {
            SymbolicVariableKind::Named(named) => Some(&named.name),
            _ => None,
        };
        Self::from_access(ctx, resolve_symbolic_access(ctx, base)?, name, span)
    }

    /// Builds the place that `access`, already resolved from a variable
    /// reference, addresses. `name` is the variable when the reference is a
    /// plain name, and `span` is the reference.
    ///
    /// A field at a fixed offset in a structure has no [`ResolvedAccess`];
    /// [`Place::resolve`] addresses it.
    pub(crate) fn from_access(
        ctx: &CompileContext,
        access: ResolvedAccess<'_, 'ast>,
        name: Option<&Id>,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        let (address, type_info) = match access {
            ResolvedAccess::Scalar { .. } if is_string_variable(ctx, name) => {
                return Err(string_place(span));
            }
            ResolvedAccess::Scalar { var_index } => {
                (Address::Variable(var_index), variable_type_info(ctx, name))
            }
            ResolvedAccess::InOut { ref_slot } => {
                (Address::InOut(ref_slot), variable_type_info(ctx, name))
            }
            ResolvedAccess::ArrayElement { info, subscripts } => {
                array_element(info, subscripts, false, &span)?
            }
            ResolvedAccess::DerefArrayElement { info, subscripts } => {
                array_element(info, subscripts, true, &span)?
            }
            ResolvedAccess::StructFieldArrayElement {
                var_index,
                desc_index,
                field_slot_offset,
                dimensions,
                subscripts,
                element_type,
                ..
            } => (
                Address::Element {
                    var_index,
                    desc_index,
                    through_ref: false,
                    index: ElementIndex::Subscripts {
                        dimensions,
                        subscripts,
                        offset: Some(field_slot_offset),
                    },
                },
                var_type_info_for_field(&element_type),
            ),
            ResolvedAccess::StructFieldStringArrayElement(_) => return Err(string_place(span)),
        };
        Ok(Place {
            address,
            type_info,
            span,
        })
    }

    /// The type the place's value is loaded and stored at.
    pub(crate) fn op_type(&self) -> OpType {
        self.type_info
            .map(|ti| (ti.op_width, ti.signedness))
            .unwrap_or(DEFAULT_OP_TYPE)
    }

    /// Pushes the value the place holds.
    pub(crate) fn emit_load(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
    ) -> Result<(), Diagnostic> {
        match &self.address {
            Address::Variable(var_index) => emit_load_var(emitter, *var_index, self.op_type()),
            Address::InOut(ref_slot) => emit_load_in_out(emitter, *ref_slot),
            Address::Element {
                var_index,
                desc_index,
                through_ref,
                index,
            } => {
                index.emit(emitter, ctx, &self.span)?;
                if *through_ref {
                    emitter.emit_load_array_deref(*var_index, *desc_index);
                } else {
                    emitter.emit_load_array(*var_index, *desc_index);
                }
            }
        }
        Ok(())
    }

    /// Pops a value, truncates it to the place's type, and stores it in the
    /// place.
    ///
    /// The address is emitted again, not kept from [`Place::emit_load`], so a
    /// subscript is evaluated for the store as well as for the load.
    pub(crate) fn emit_store(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
    ) -> Result<(), Diagnostic> {
        if let Some(type_info) = self.type_info {
            emit_truncation(emitter, type_info);
        }
        match &self.address {
            Address::Variable(var_index) => emit_store_var(emitter, *var_index, self.op_type()),
            Address::InOut(ref_slot) => {
                emitter.emit_load_var_i64(*ref_slot);
                emitter.emit_store_indirect();
            }
            Address::Element {
                var_index,
                desc_index,
                through_ref,
                index,
            } => {
                index.emit(emitter, ctx, &self.span)?;
                if *through_ref {
                    emitter.emit_store_array_deref(*var_index, *desc_index);
                } else {
                    emitter.emit_store_array(*var_index, *desc_index);
                }
            }
        }
        Ok(())
    }
}

impl ElementIndex<'_> {
    /// Pushes the index.
    fn emit(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        match self {
            ElementIndex::Slot(slot) => {
                let slot_const = ctx.add_i32_constant(slot.raw() as i32);
                emitter.emit_load_const_i32(slot_const);
            }
            ElementIndex::Subscripts {
                dimensions,
                subscripts,
                offset,
            } => {
                emit_flat_index(emitter, ctx, subscripts, dimensions, span)?;
                if let Some(offset) = offset {
                    let offset_const = ctx.add_i64_constant(offset.raw() as i64);
                    emitter.emit_load_const_i64(offset_const);
                    emitter.emit_add_i64();
                }
            }
        }
        Ok(())
    }
}

/// The address and type of an element of an array variable, or of the array
/// a reference variable refers to when `through_ref` is set.
fn array_element<'ast>(
    info: &ArrayVarInfo,
    subscripts: Vec<&'ast Expr>,
    through_ref: bool,
    span: &SourceSpan,
) -> Result<(Address<'ast>, Option<VarTypeInfo>), Diagnostic> {
    if info.is_string_element {
        return Err(string_place(span.clone()));
    }
    let address = Address::Element {
        var_index: info.var_index,
        desc_index: info.desc_index,
        through_ref,
        index: ElementIndex::Subscripts {
            dimensions: info.dimensions.clone(),
            subscripts,
            offset: None,
        },
    };
    Ok((address, Some(info.element_var_type_info)))
}

/// The type of the variable `name`, when the reference is a plain name.
fn variable_type_info(ctx: &CompileContext, name: Option<&Id>) -> Option<VarTypeInfo> {
    name.and_then(|name| ctx.var_type_info(name))
}

/// Returns `true` if `name` is a STRING variable.
fn is_string_variable(ctx: &CompileContext, name: Option<&Id>) -> bool {
    name.is_some_and(|name| ctx.string_vars.contains_key(name))
}

/// A STRING lives in the data region rather than in one slot, so it is not a
/// place.
fn string_place(span: SourceSpan) -> Diagnostic {
    Diagnostic::not_implemented(Label::span(
        span,
        "Bit or partial access of a STRING is not supported",
    ))
}
