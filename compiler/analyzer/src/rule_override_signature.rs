//! Semantic rule that a method or property which overrides an inherited one
//! matches the one it overrides (OOP extension).
//!
//! A function block overrides a method by declaring one with the name of a
//! method it inherits through `EXTENDS`. The two must have the same
//! signature: the same return type, the same `VAR_INPUT`, `VAR_OUTPUT` and
//! `VAR_IN_OUT` variables (names, sections and types, in order) and the same
//! `R_EDGE`/`F_EDGE` inputs. Locals are not part of a signature. The
//! signatures compared are those the type environment records, compared by
//! [`TypeEnvironment::same_signature`], as interface conformance compares
//! them. A type that did not resolve is reported elsewhere and matches here.
//!
//! An override is compared with the method it overrides: the nearest one up
//! the `EXTENDS` chain, so each block whose method differs from its base's is
//! reported once, at its own method (P4089).
//!
//! A property overrides an inherited property of its name in the same way,
//! and must have the type of the property it overrides, compared as above
//! (P4090). Which accessors an override must keep is not checked: an
//! override with a `GET` and no `SET` has not been probed in XAE.
//!
//! A method or property that implements a member of an interface the block
//! implements, and differs from that member, is reported by interface
//! conformance (P4068) and not again here. TwinCAT XAE gives both one message,
//! `Interface of overridden method ... doesn't match declaration`.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Root
//! METHOD Describe : INT
//! END_METHOD
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
//! METHOD Describe : INT
//! END_METHOD
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Root
//! METHOD Describe : INT
//! END_METHOD
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
//! METHOD Describe : BOOL
//! END_METHOD
//! END_FUNCTION_BLOCK
//! ```

use std::collections::HashSet;

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    type_id::TypeId,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    semantic_context::SemanticContext,
    type_environment::{
        MethodParameter, MethodResult, MethodSignature, PropertySignature, TypeEnvironment,
    },
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let types = context.types();
    let mut checked: HashSet<&TypeName> = HashSet::new();
    let mut diagnostics = vec![];

    for element in &lib.elements {
        let LibraryElementKind::FunctionBlockDeclaration(fb) = element else {
            continue;
        };
        // A repeated block name keeps its first declaration (P4013).
        if !checked.insert(&fb.name) {
            continue;
        }
        let Some(block) = types.id_of(&fb.name) else {
            continue;
        };
        // A block with no recorded base overrides nothing; a base that is not
        // a declared block is reported for itself (P4088).
        let Some(base) = types.base_of(block) else {
            continue;
        };
        for method in &fb.methods {
            let Some((declaring, own)) = types.method(block, &method.name) else {
                continue;
            };
            if declaring != block {
                continue;
            }
            let Some((overridden_block, overridden)) = types.method(base, &method.name) else {
                continue;
            };
            if types.same_signature(own, overridden)
                || differs_from_interface_member(types, &fb.name, own)
            {
                continue;
            }
            let overridden_name = format!(
                "{}.{}",
                types
                    .name_of(overridden_block)
                    .map(|name| name.to_string())
                    .unwrap_or_default(),
                overridden.name
            );
            diagnostics.push(
                Diagnostic::problem(
                    Problem::OverrideSignatureMismatch,
                    Label::span(
                        method.name.span(),
                        difference(types, own, overridden, &overridden_name),
                    ),
                )
                .with_secondary(Label::span(overridden.name.span(), "Overridden method"))
                .with_context_id("function block", &fb.name.name)
                .with_context_id("method", &method.name),
            );
        }
        for property in &fb.properties {
            let (Some(own), Some(overridden)) = (
                types.property(block, &property.name),
                types.property(base, &property.name),
            ) else {
                continue;
            };
            if same_type(types, own.property_type, overridden.property_type)
                || differs_from_interface_property(types, &fb.name, own)
            {
                continue;
            }
            diagnostics.push(
                Diagnostic::problem(
                    Problem::OverridePropertyTypeMismatch,
                    Label::span(
                        property.name.span(),
                        format!(
                            "Has type {}, where the property it overrides has type {}",
                            type_text(types, own.property_type),
                            type_text(types, overridden.property_type)
                        ),
                    ),
                )
                .with_secondary(Label::span(overridden.name.span(), "Overridden property"))
                .with_context_id("function block", &fb.name.name)
                .with_context_id("property", &property.name),
            );
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Whether `method` of the block `block` differs from a member of the same
/// name of an interface the block implements, which interface conformance
/// reports (P4068).
fn differs_from_interface_member(
    types: &TypeEnvironment,
    block: &TypeName,
    method: &MethodSignature,
) -> bool {
    types
        .supertypes()
        .all_of(block)
        .iter()
        .filter_map(|supertype| types.interface_methods(types.id_of(supertype)?))
        .flatten()
        .any(|member| member.name == method.name && !types.same_signature(method, member))
}

/// Whether `property` of the block `block` does not provide a property
/// prototype of the same name of an interface the block implements, which
/// interface conformance reports (P4068).
fn differs_from_interface_property(
    types: &TypeEnvironment,
    block: &TypeName,
    property: &PropertySignature,
) -> bool {
    types
        .supertypes()
        .all_of(block)
        .iter()
        .filter_map(|supertype| types.interface_properties(types.id_of(supertype)?))
        .flatten()
        .any(|member| member.name == property.name && !types.provides_property(property, member))
}

/// What the label says about how `own` differs from `overridden`, the method
/// `overridden_name`: the first difference found.
fn difference(
    types: &TypeEnvironment,
    own: &MethodSignature,
    overridden: &MethodSignature,
    overridden_name: &str,
) -> String {
    if !same_result(types, &own.result, &overridden.result) {
        return format!(
            "Returns {}, where {overridden_name} returns {}",
            result_text(types, &own.result),
            result_text(types, &overridden.result)
        );
    }
    if own.parameters.len() != overridden.parameters.len() {
        return format!(
            "Declares {} parameters, where {overridden_name} declares {}",
            own.parameters.len(),
            overridden.parameters.len()
        );
    }
    let differing = own
        .parameters
        .iter()
        .zip(&overridden.parameters)
        .find(|(a, b)| {
            a.name != b.name || a.section != b.section || !same_type(types, a.type_id, b.type_id)
        });
    if let Some((a, b)) = differing {
        return format!(
            "Declares {}, where {overridden_name} declares {}",
            parameter_text(types, a),
            parameter_text(types, b)
        );
    }
    format!("Declares other R_EDGE or F_EDGE inputs than {overridden_name}")
}

fn same_result(types: &TypeEnvironment, a: &MethodResult, b: &MethodResult) -> bool {
    match (a, b) {
        (MethodResult::None, MethodResult::None) => true,
        (MethodResult::Value(a), MethodResult::Value(b)) => same_type(types, *a, *b),
        _ => false,
    }
}

/// As the signature comparison treats types: a type that did not resolve
/// matches any.
fn same_type(types: &TypeEnvironment, a: Option<TypeId>, b: Option<TypeId>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => types.same_type(a, b),
        _ => true,
    }
}

fn result_text(types: &TypeEnvironment, result: &MethodResult) -> String {
    match result {
        MethodResult::None => "nothing".to_string(),
        MethodResult::Value(type_id) => type_text(types, *type_id),
    }
}

fn parameter_text(types: &TypeEnvironment, parameter: &MethodParameter) -> String {
    let section = match parameter.section {
        VariableType::Input => "VAR_INPUT",
        VariableType::Output => "VAR_OUTPUT",
        VariableType::InOut => "VAR_IN_OUT",
        _ => "VAR",
    };
    format!(
        "{section} {} : {}",
        parameter.name,
        type_text(types, parameter.type_id)
    )
}

/// The name of the type `type_id`; an elementary type in upper case, as a
/// program writes it.
fn type_text(types: &TypeEnvironment, type_id: Option<TypeId>) -> String {
    let Some(name) = type_id.and_then(|id| types.name_of(id)) else {
        return "an unresolved type".to_string();
    };
    match ElementaryTypeName::try_from(&name.name) {
        Ok(_) => name.to_string().to_uppercase(),
        Err(_) => name.to_string(),
    }
}

#[cfg(test)]
mod tests;
