//! Reference (`REF_TO`) variable registration for IEC 61131-3 code generation.
//!
//! A reference variable is stored as a 64-bit unsigned value, in one of the
//! forms `ironplc_container::reference` describes: [`reference_to`] makes a
//! string's a data-region reference to its header, and any other variable's
//! its variable-table index. When the target is an array, the variable also
//! carries array metadata so that `PT^[idx]` can be compiled with the deref
//! array opcodes, and when the target is a string or an array of strings,
//! the descriptor that `STR_LOAD_ARRAY_DEREF` and `STR_STORE_ARRAY_DEREF`
//! reach the strings with ([`StringRefInfo`]). When the target is an array
//! or a structure, it carries the descriptor that sizes the whole of it, for
//! `COPY_REGION_DEREF`. Every declaration site
//! (program, function parameter, function local, function block) registers
//! references through [`register_reference_variable`], or, given the type
//! referenced, [`register_reference_of_type`].

use crate::access::Named;
use ironplc_container::reference::data_region_reference;
use ironplc_container::CharWidth;
use ironplc_ir::Diagnostic;

use super::compile::CompileContext;

/// The strings a reference variable refers to, a string or an array of
/// strings, as `STR_LOAD_ARRAY_DEREF` and `STR_STORE_ARRAY_DEREF` reach them
/// through the reference: a string is the one element of a one-element
/// array.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StringRefInfo {
    /// The descriptor of the array of strings the reference refers to.
    pub(crate) desc_index: u16,
    /// The encoding of each string, which a value stored through the
    /// reference must be produced at (ADR-0034).
    pub(crate) char_width: CharWidth,
}

/// The reference to the variable `name`, as `REF(name)` makes it: a
/// data-region reference to the header of a string, which lives in the data
/// region, and a variable reference to the slot of any other variable
/// (`specs/design/var-in-out-parameters.md`, "References").
pub(crate) fn reference_to(ctx: &CompileContext, name: &Named) -> Result<u64, Diagnostic> {
    match ctx.string_var(name) {
        Some(string) => Ok(data_region_reference(string.data_offset)),
        None => Ok(u64::from(ctx.var_index(name)?.raw())),
    }
}
