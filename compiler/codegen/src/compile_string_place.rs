//! Addressing a string place: the run of the data region that holds one
//! `STRING` or `WSTRING`, such as the target of an assignment
//! (`names[i] := s`) or a string read (`t := p.name`).
//!
//! [`StringPlace::resolve`] works out where the string is from the variable
//! reference. [`StringPlace::emit_load`] and [`StringPlace::emit_store`] then
//! emit the load into a temporary buffer and the store from one for that
//! shape, as [`Place`](super::compile_place::Place) does for a value in one
//! slot.
//!
//! A string place's subscripts are any [`Subscript`]: the AST expressions
//! [`StringPlace::resolve`] finds, by default.

use ironplc_analyzer::semantic_type::SemanticType;
use ironplc_container::{CharWidth, VarIndex};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, SymbolicVariableKind, Variable};

use super::compile::CompileContext;
use super::compile_array::{emit_flat_index, resolve_access, DimensionInfo, ResolvedAccess};
use super::compile_expr::{resolve_variable_name, variable_span};
use super::compile_struct::walk_struct_chain;
use super::compile_subscript::Subscript;
use crate::emit::Emitter;

/// The place that holds one string, with subscripts of type `S`.
pub(crate) struct StringPlace<S> {
    address: StringAddress<S>,
    /// The encoding of the string, which a value stored there must be
    /// produced at (ADR-0034).
    char_width: CharWidth,
    /// The variable reference, for a diagnostic about its subscripts.
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
}

impl<'ast> StringPlace<&'ast Expr> {
    /// The string place `variable` names; `None` when it names no string.
    ///
    /// A field of a function block instance is not a string place; the
    /// caller addresses it first.
    pub(crate) fn resolve(
        ctx: &CompileContext,
        variable: &'ast Variable,
    ) -> Result<Option<Self>, Diagnostic> {
        let span = variable_span(variable);
        let fixed = |offset, char_width| {
            Some(StringPlace {
                address: StringAddress::Fixed(offset),
                char_width,
                span: span.clone(),
            })
        };
        // A field at a fixed offset in a structure, not of an element of an
        // array of structures (`s.arr[i].field`), which `resolve_access`
        // addresses.
        if let Variable::Symbolic(SymbolicVariableKind::Structured(structured)) = variable {
            if !matches!(structured.record.as_ref(), SymbolicVariableKind::Array(_)) {
                let (root, slot_offset, field_type) =
                    walk_struct_chain(ctx, &structured.record, &structured.field, 0)?;
                let SemanticType::String { char_width, .. } = field_type else {
                    return Ok(None);
                };
                // `walk_struct_chain` found this structure variable.
                let structure = ctx.struct_vars.get(&root).ok_or_else(|| {
                    Diagnostic::internal_error_at(Label::span(
                        structured.span(),
                        format!("Variable '{root}' is not a structure"),
                    ))
                })?;
                let offset = structure.data_offset + slot_offset.raw() * 8;
                return Ok(fixed(offset, char_width));
            }
        }
        if let Some(info) = resolve_variable_name(variable).and_then(|n| ctx.string_vars.get(n)) {
            return Ok(fixed(info.data_offset, info.char_width));
        }
        let (address, char_width) = match resolve_access(ctx, variable)? {
            ResolvedAccess::ArrayElement { info, subscripts } if info.is_string_element => (
                StringAddress::Element {
                    var_index: info.var_index,
                    desc_index: info.desc_index,
                    within: None,
                    dimensions: info.dimensions.clone(),
                    subscripts,
                },
                info.string_char_width,
            ),
            ResolvedAccess::StructFieldStringArrayElement(element) => (
                StringAddress::Element {
                    var_index: element.scratch_var_index,
                    desc_index: element.string_desc_index,
                    within: Some((element.var_index, element.field_byte_offset)),
                    dimensions: element.dimensions,
                    subscripts: element.subscripts,
                },
                element.char_width,
            ),
            ResolvedAccess::Scalar { .. }
            | ResolvedAccess::InOut { .. }
            | ResolvedAccess::ArrayElement { .. }
            | ResolvedAccess::DerefArrayElement { .. }
            | ResolvedAccess::StructFieldArrayElement { .. } => return Ok(None),
        };
        Ok(Some(StringPlace {
            address,
            char_width,
            span,
        }))
    }
}

impl<S> StringPlace<S> {
    /// The encoding of the string the place holds.
    pub(crate) fn char_width(&self) -> CharWidth {
        self.char_width
    }

    /// The same place with each subscript replaced by what `replace` makes
    /// of it, or the first error `replace` returns.
    pub(crate) fn try_map_subscripts<T, E>(
        self,
        replace: impl FnMut(S) -> Result<T, E>,
    ) -> Result<StringPlace<T>, E> {
        let address = match self.address {
            StringAddress::Fixed(offset) => StringAddress::Fixed(offset),
            StringAddress::Element {
                var_index,
                desc_index,
                within,
                dimensions,
                subscripts,
            } => StringAddress::Element {
                var_index,
                desc_index,
                within,
                dimensions,
                subscripts: subscripts
                    .into_iter()
                    .map(replace)
                    .collect::<Result<_, E>>()?,
            },
        };
        Ok(StringPlace {
            address,
            char_width: self.char_width,
            span: self.span,
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
        }
        Ok(())
    }

    /// For an element, what `STR_LOAD_ARRAY_ELEM` and `STR_STORE_ARRAY_ELEM`
    /// need before they run: the array's start, when it is within a
    /// structure, then the flat index.
    fn emit_element_index(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
    ) -> Result<(), Diagnostic> {
        let StringAddress::Element {
            var_index,
            within,
            dimensions,
            subscripts,
            ..
        } = &self.address
        else {
            return Ok(());
        };
        if let Some((structure, byte_offset)) = within {
            emitter.emit_load_var_i32(*structure);
            let offset = ctx.add_i32_constant(*byte_offset as i32);
            emitter.emit_load_const_i32(offset);
            emitter.emit_add_i32();
            emitter.emit_store_var_i32(*var_index);
        }
        emit_flat_index(emitter, ctx, subscripts, dimensions, &self.span)
    }
}
