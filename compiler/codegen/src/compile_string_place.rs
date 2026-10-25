//! Addressing a string place: the run of the data region that holds one
//! `STRING` or `WSTRING`, such as the target of an assignment
//! (`names[i] := s`) or a string read (`t := p.name`).
//!
//! [`StringPlace::resolve`] works out where the string is from the
//! [`Access`] that names it, and [`StringPlace::through_reference`] where
//! the string a reference variable refers to is. [`StringPlace::emit_load`] and
//! [`StringPlace::emit_store`] then emit the load into a temporary buffer
//! and the store from one for that shape, as
//! [`Place`](super::compile_place::Place) does for a value in one slot.
//!
//! A string place's subscripts are any [`Subscript`]: the access's own.

use ironplc_container::VarIndex;
use ironplc_ir::lowered::{TypeKind, TypeTable};
use ironplc_ir::{Diagnostic, Label};
use ironplc_ir::{Located, SourceSpan};

use super::compile::CompileContext;
use super::compile_array::{emit_flat_index, resolve_access, DimensionInfo, ResolvedAccess};
use super::compile_struct::walk_struct_chain;
use super::compile_subscript::Subscript;
use crate::access::Access;
use crate::emit::Emitter;

/// The place that holds one string, with subscripts of type `S`.
pub(crate) struct StringPlace<S> {
    address: StringAddress<S>,
    /// Where the place is named, for a diagnostic about its subscripts.
    span: SourceSpan,
}

/// Where a [`StringPlace`] is, which decides the opcodes that load and store
/// it.
enum StringAddress<S> {
    /// At a fixed data region offset: a string variable, or a string field of
    /// a structure. `STR_LOAD_VAR` and `STR_STORE_VAR`.
    Fixed(u32),
    /// An element of a string array, at the flat index of `subscripts`:
    /// `STR_LOAD_ARRAY_ELEM` and `STR_STORE_ARRAY_ELEM`, which read the
    /// array's start from `var_index`.
    Element {
        var_index: VarIndex,
        desc_index: u16,
        /// For an array within a structure, whose variable holds the start
        /// of the whole structure: that variable and the array's byte offset
        /// in it, whose sum is first stored in `var_index`.
        within: Option<(VarIndex, u32)>,
        dimensions: Vec<DimensionInfo>,
        subscripts: Vec<S>,
    },
    /// What the reference in `ref_slot` refers to: a string, the one element
    /// of a one-element array, or an element of an array of strings, at the
    /// flat index of `subscripts`. `STR_LOAD_ARRAY_DEREF` and
    /// `STR_STORE_ARRAY_DEREF`, with the descriptor of that array.
    Referenced {
        ref_slot: VarIndex,
        desc_index: u16,
        dimensions: Vec<DimensionInfo>,
        subscripts: Vec<S>,
    },
}

impl<S: Clone> StringPlace<S> {
    /// The string place `variable` names, of the types `types`; `None` when
    /// it names no string.
    ///
    /// A field of a function block instance is not a string place; the
    /// caller addresses it first.
    pub(crate) fn resolve(
        types: &TypeTable,
        ctx: &CompileContext,
        variable: &Access<S>,
    ) -> Result<Option<Self>, Diagnostic> {
        let span = variable.span();
        let fixed = |offset| {
            Some(StringPlace {
                address: StringAddress::Fixed(offset),
                span: span.clone(),
            })
        };
        // A field at a fixed offset in a structure, not in an element of an
        // array of structures (`s.arr[i].field`, `arr[i].inner.field`),
        // which `resolve_access` addresses.
        if let Access::Field(structured) = variable {
            if crate::compile_array_struct::element_field_chain(structured).is_none() {
                let (root, slot_offset, field_type) = walk_struct_chain(types, ctx, structured, 0)?;
                let Some(TypeKind::String(_)) = types.kind(field_type) else {
                    return Ok(None);
                };
                // `walk_struct_chain` found this structure variable.
                let structure = ctx.struct_var(&root).ok_or_else(|| {
                    Diagnostic::internal_error_at(Label::span(
                        structured.field.span(),
                        format!("Variable '{root}' is not a structure"),
                    ))
                })?;
                let offset = structure.data_offset + slot_offset.raw() * 8;
                return Ok(fixed(offset));
            }
        }
        if let Some(info) = variable.name().and_then(|n| ctx.string_var(n)) {
            return Ok(fixed(info.data_offset));
        }
        let address = match resolve_access(types, ctx, variable)? {
            ResolvedAccess::ArrayElement { info, subscripts } if info.is_string_element => {
                StringAddress::Element {
                    var_index: info.var_index,
                    desc_index: info.desc_index,
                    within: None,
                    dimensions: info.dimensions.clone(),
                    subscripts,
                }
            }
            ResolvedAccess::StructFieldStringArrayElement(element) => StringAddress::Element {
                var_index: element.scratch_var_index,
                desc_index: element.string_desc_index,
                within: Some((element.var_index, element.field_byte_offset)),
                dimensions: element.dimensions,
                subscripts: element.subscripts,
            },
            ResolvedAccess::Scalar { .. }
            | ResolvedAccess::InOut { .. }
            | ResolvedAccess::ArrayElement { .. }
            | ResolvedAccess::DerefArrayElement { .. }
            | ResolvedAccess::StructFieldArrayElement { .. } => return Ok(None),
        };
        Ok(Some(StringPlace { address, span }))
    }

    /// The string place `variable` names when it is the string a reference
    /// variable refers to, `rs^`, or an element of the array of strings one
    /// refers to, `rsa^[i]`. `None` for any other variable, and for a
    /// reference in a `VAR_IN_OUT` parameter, whose slot holds a reference
    /// to the reference.
    pub(crate) fn through_reference(ctx: &CompileContext, variable: &Access<S>) -> Option<Self> {
        let (deref, subscripts) = match variable {
            Access::Deref(deref) => (deref, Vec::new()),
            Access::Index(array) => {
                let Access::Deref(deref) = array.array.as_ref() else {
                    return None;
                };
                (deref, array.subscripts.clone())
            }
            Access::Named(_) | Access::Field(_) => return None,
        };
        let name = deref.name()?;
        if ctx.is_in_out(name) {
            return None;
        }
        let strings = ctx.string_ref(name)?;
        // An element of an array of strings has its subscripts, and a string
        // none; neither a whole array nor a subscript of a string is one.
        let dimensions = match (ctx.array_var(name), subscripts.is_empty()) {
            (Some(array), false) => array.dimensions.clone(),
            (None, true) => Vec::new(),
            (Some(_), true) | (None, false) => return None,
        };
        Some(StringPlace {
            address: StringAddress::Referenced {
                ref_slot: ctx.var_index(name).ok()?,
                desc_index: strings.desc_index,
                dimensions,
                subscripts,
            },
            span: variable.span(),
        })
    }
}

impl<S: Subscript> StringPlace<S> {
    /// Copies the string the place holds into a temporary buffer, leaving
    /// the buffer's index on the stack.
    pub(crate) fn emit_load(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
    ) -> Result<(), Diagnostic> {
        match &self.address {
            StringAddress::Fixed(offset) => emitter.emit_str_load_var(*offset),
            StringAddress::Element {
                var_index,
                desc_index,
                ..
            } => {
                self.emit_element_index(emitter, ctx)?;
                emitter.emit_str_load_array_elem(*var_index, *desc_index);
            }
            StringAddress::Referenced {
                ref_slot,
                desc_index,
                ..
            } => {
                self.emit_element_index(emitter, ctx)?;
                emitter.emit_str_load_array_deref(*ref_slot, *desc_index);
            }
        }
        Ok(())
    }

    /// Pops a temporary buffer's index and stores the string it holds in the
    /// place, keeping the code units the place's capacity holds.
    pub(crate) fn emit_store(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
    ) -> Result<(), Diagnostic> {
        match &self.address {
            StringAddress::Fixed(offset) => emitter.emit_str_store_var(*offset),
            StringAddress::Element {
                var_index,
                desc_index,
                ..
            } => {
                self.emit_element_index(emitter, ctx)?;
                emitter.emit_str_store_array_elem(*var_index, *desc_index);
            }
            StringAddress::Referenced {
                ref_slot,
                desc_index,
                ..
            } => {
                self.emit_element_index(emitter, ctx)?;
                emitter.emit_str_store_array_deref(*ref_slot, *desc_index);
            }
        }
        Ok(())
    }

    /// For an element, what the string array element instructions need
    /// before they run: the array's start, when it is within a structure,
    /// then the flat index. The flat index of a string a reference refers
    /// to, which has no subscripts, is 0.
    fn emit_element_index(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
    ) -> Result<(), Diagnostic> {
        let (dimensions, subscripts) = match &self.address {
            StringAddress::Fixed(_) => return Ok(()),
            StringAddress::Element {
                var_index,
                within,
                dimensions,
                subscripts,
                ..
            } => {
                if let Some((structure, byte_offset)) = within {
                    emitter.emit_load_var_i32(*structure);
                    let offset = ctx.add_i32_constant(*byte_offset as i32);
                    emitter.emit_load_const_i32(offset);
                    emitter.emit_add_i32();
                    emitter.emit_store_var_i32(*var_index);
                }
                (dimensions, subscripts)
            }
            StringAddress::Referenced {
                dimensions,
                subscripts,
                ..
            } => (dimensions, subscripts),
        };
        emit_flat_index(emitter, ctx, subscripts, dimensions, &self.span)
    }
}
