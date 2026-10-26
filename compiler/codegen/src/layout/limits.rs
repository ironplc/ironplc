//! The limits of the bytecode format a declaration can exceed, checked over
//! every declaration the layout lays out before it is built
//! ([`super::declarations`]): an array of more elements, or a structure or an
//! array of structures of more slots, than flat-index arithmetic reaches
//! (P9997); a structure that holds a function block, which this backend does
//! not lay out yet (P9999); and a function block with more fields than an
//! instruction names (P9997).

use ironplc_ir::lowered::{Declaration, Program};
use ironplc_ir::lowered::{TypeId, TypeKind, TypeTable};
use ironplc_ir::SourceSpan;
use ironplc_ir::{Diagnostic, Label};

use super::declarations::{copy_slots, frame, globals, instance_type, Pous};
use super::region::{elements, slots};
use crate::compile::MAX_DATA_REGION_SLOTS;

/// The most fields a user function block has: its inputs, outputs and
/// internal variables. `FB_CALL` and `METHOD_CALL` count them, and
/// `FB_LOAD_PARAM` and `FB_STORE_PARAM` name one, by a byte.
const MAX_USER_FB_FIELDS: usize = u8::MAX as usize;

/// Checks every declaration of the POUs `pous` of `lowering` within the
/// limits, at the first that is not.
pub(super) fn check(lowering: &Program, pous: &Pous) -> Option<Result<(), Diagnostic>> {
    let types = lowering.types();
    let variables = lowering.variables();
    let mut declared = Vec::new();
    for (var, _) in globals(lowering) {
        let variable = variables.get(var)?;
        declared.push((variable.ty(), variable.name().span()));
    }
    let program_type = instance_type(types, pous.program)?;
    for declaration in lowering.declarations(pous.program)? {
        match declaration {
            Declaration::Field(field) => {
                let field = types.fields(program_type)?.get(field.index() as usize)?;
                declared.push((field.ty, field.name.span()));
            }
            Declaration::Variable(var) => {
                let variable = variables.get(*var)?;
                declared.push((variable.ty(), variable.name().span()));
            }
            Declaration::External(_) => {}
        }
    }
    let methods = pous.methods.iter().map(|(_, method)| *method);
    for pou in pous.functions.iter().copied().chain(methods) {
        let frame = frame(lowering, pou)?;
        for var in frame.variables.into_iter().chain(frame.result) {
            let variable = variables.get(var)?;
            declared.push((variable.ty(), variable.name().span()));
        }
    }
    for block in &pous.blocks {
        let fields = copy_slots(lowering, *block)?;
        if fields.len() > MAX_USER_FB_FIELDS {
            let span = lowering.pous().name(*block)?.span();
            return Some(Err(Diagnostic::not_supported(Label::span(
                span,
                format!("Function block with more than {MAX_USER_FB_FIELDS} fields"),
            ))
            .with_context("fields", &fields.len().to_string())));
        }
        declared.extend(
            fields
                .iter()
                .map(|(_, field)| (field.ty, field.name.span())),
        );
    }
    Some(
        declared
            .into_iter()
            .try_for_each(|(ty, span)| within(types, ty, &span)),
    )
}

/// Whether a declaration of the type `ty`, at `span`, is within the limits:
/// an array's, a structure's, an array of structures' and what a reference
/// refers to.
fn within(types: &TypeTable, ty: TypeId, span: &SourceSpan) -> Result<(), Diagnostic> {
    let too_large = |message: &str| Diagnostic::not_supported(Label::span(span.clone(), message));
    match types.kind(ty) {
        Some(TypeKind::Array { element, .. }) => {
            let total = elements(types, ty).filter(|total| *total <= MAX_DATA_REGION_SLOTS);
            let total = total.ok_or_else(|| too_large("Array exceeds maximum 32768 elements"))?;
            if let Some(TypeKind::Structure { .. }) = types.kind(*element) {
                structure(types, *element, span)?;
                let element_slots = slots(types, *element).unwrap_or(u32::MAX);
                if total.saturating_mul(element_slots) > MAX_DATA_REGION_SLOTS {
                    return Err(too_large("Array exceeds maximum 32768 slots"));
                }
            }
            Ok(())
        }
        Some(TypeKind::Structure { .. }) => structure(types, ty, span),
        Some(TypeKind::Reference { target, .. }) => within(types, *target, span),
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
            | TypeKind::Program { .. },
        )
        | None => Ok(()),
    }
}

/// Whether a structure of the type `ty`, declared at `span`, holds no
/// function block and fits the slots flat-index arithmetic reaches.
fn structure(types: &TypeTable, ty: TypeId, span: &SourceSpan) -> Result<(), Diagnostic> {
    if holds_block(types, ty) {
        return Err(Diagnostic::not_implemented(Label::span(
            span.clone(),
            "Structure contains unsupported field types (STRING, WSTRING, or FunctionBlock)",
        )));
    }
    match slots(types, ty) {
        Some(total) if total <= MAX_DATA_REGION_SLOTS => Ok(()),
        _ => Err(Diagnostic::not_supported(Label::span(
            span.clone(),
            "Structure exceeds maximum 32768 slots",
        ))),
    }
}

/// Whether a value of the type `ty` holds a function block instance: is
/// one, or a structure or an array that holds one.
fn holds_block(types: &TypeTable, ty: TypeId) -> bool {
    match types.kind(ty) {
        Some(TypeKind::FunctionBlock { .. } | TypeKind::Program { .. }) => true,
        Some(TypeKind::Structure { fields }) => {
            fields.iter().any(|field| holds_block(types, field.ty))
        }
        Some(TypeKind::Array { element, .. }) => holds_block(types, *element),
        Some(
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::String(_)
            | TypeKind::Reference { .. },
        )
        | None => false,
    }
}
