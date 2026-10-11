//! Semantic rule that an array initializer gives no more values than the
//! array has elements.
//!
//! An initializer lists element values in order, a repeated value (`3(0)`)
//! once for each repetition. Fewer values than elements is allowed: the rest
//! start at the element type's default. More has no element to go to.
//!
//! An array of arrays takes its values flat, element by element of its
//! innermost arrays, so the values are counted against every element of
//! every level.
//!
//! The rule checks every place an array initializer can appear: a variable
//! declaration, the default of an array type or alias, the default of an
//! array field of a structure, and an array member of a structure or
//! function block instance initializer.
//!
//! See section 2.4.3.2.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       a : ARRAY[1..3] OF DINT := [1, 2];
//!       b : ARRAY[1..4] OF DINT := [2(0), 1, 2];
//!    END_VAR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       a : ARRAY[1..3] OF DINT := [1, 2, 3, 4];
//!    END_VAR
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::*,
    core::{Located, SourceSpan},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    semantic_type::SemanticType,
    type_environment::TypeEnvironment,
    variable_type::struct_field_type,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleArrayInitializerLength {
            types: context.types(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleArrayInitializerLength<'a> {
    types: &'a TypeEnvironment,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleArrayInitializerLength<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// The number of values `elements` list, each repetition counted.
fn value_count(elements: &[ArrayInitialElementKind]) -> u128 {
    elements.iter().fold(0u128, |count, element| {
        count.saturating_add(match element {
            ArrayInitialElementKind::Constant(_)
            | ArrayInitialElementKind::EnumValue(_)
            | ArrayInitialElementKind::Structure(_)
            | ArrayInitialElementKind::Expression(_) => 1,
            ArrayInitialElementKind::Repeated(repeated) => {
                let each = match repeated.init.as_ref() {
                    Some(inner) => value_count(std::slice::from_ref(inner)),
                    None => 1,
                };
                repeated.size.value.saturating_mul(each)
            }
        })
    })
}

/// The number of values an array of `representation` takes: its elements,
/// and for an array of arrays the elements of every level. `None` when the
/// type is not an array or its size is not known.
fn element_count(representation: &SemanticType) -> Option<u128> {
    let SemanticType::Array { element_type, .. } = representation else {
        return None;
    };
    let count = u128::from(representation.array_total_elements()?);
    match element_type.as_ref() {
        inner @ SemanticType::Array { .. } => count.checked_mul(element_count(inner)?),
        _ => Some(count),
    }
}

impl RuleArrayInitializerLength<'_> {
    /// Reports `elements` when an array of `declared` has fewer elements
    /// than they list values. `span` locates what the initializer
    /// initializes.
    fn check(
        &mut self,
        declared: &SemanticType,
        elements: &[ArrayInitialElementKind],
        span: SourceSpan,
    ) {
        let Some(capacity) = element_count(declared) else {
            return;
        };
        let given = value_count(elements);
        if given > capacity {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::ArrayInitializerTooManyValues,
                    Label::span(span, "Array initializer"),
                )
                .with_context("elements", &capacity.to_string())
                .with_context("values", &given.to_string()),
            );
        }
    }

    /// Checks the array members of a structure or function block instance
    /// initializer against the fields of `declared`.
    fn check_members(&mut self, declared: &SemanticType, elements: &[StructureElementInit]) {
        for element in elements {
            let Some(field) = struct_field_type(declared, &element.name) else {
                continue;
            };
            match &element.init {
                StructInitialValueAssignmentKind::Array(values) => {
                    self.check(&field, values, element.name.span())
                }
                StructInitialValueAssignmentKind::Structure(members) => {
                    self.check_members(&field, members)
                }
                _ => {}
            }
        }
    }

    /// Checks `initializer`, which initializes a place of `declared`.
    fn check_initializer(
        &mut self,
        initializer: &InitialValueAssignmentKind,
        declared: &SemanticType,
        span: SourceSpan,
    ) {
        match initializer {
            InitialValueAssignmentKind::Array(array) => {
                self.check(declared, &array.initial_values, span)
            }
            InitialValueAssignmentKind::Structure(structure) => {
                self.check_members(declared, &structure.elements_init)
            }
            InitialValueAssignmentKind::FunctionBlock(block) => {
                self.check_members(declared, &block.init)
            }
            _ => {}
        }
    }

    /// The representation of the type `type_name` names.
    fn representation(&self, type_name: &TypeName) -> Option<SemanticType> {
        Some(self.types.get(type_name)?.representation.clone())
    }
}

impl Visitor<Infallible> for RuleArrayInitializerLength<'_> {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
        if let Some(declared) = node
            .type_id
            .and_then(|id| self.types.get_by_id(id))
            .map(|attributes| attributes.representation.clone())
        {
            self.check_initializer(&node.initializer, &declared, node.identifier.span());
        }
        node.recurse_visit(self)
    }

    fn visit_simple_declaration(&mut self, node: &SimpleDeclaration) -> Result<(), Infallible> {
        if let Some(declared) = self.representation(&node.type_name) {
            self.check_initializer(&node.spec_and_init, &declared, node.type_name.span());
        }
        node.recurse_visit(self)
    }

    fn visit_array_declaration(&mut self, node: &ArrayDeclaration) -> Result<(), Infallible> {
        if let Some(declared) = self.representation(&node.type_name) {
            self.check(&declared, &node.init, node.type_name.span());
        }
        node.recurse_visit(self)
    }

    fn visit_structure_declaration(
        &mut self,
        node: &StructureDeclaration,
    ) -> Result<(), Infallible> {
        if let Some(declared) = self.representation(&node.type_name) {
            for element in &node.elements {
                if let Some(field) = struct_field_type(&declared, &element.name) {
                    self.check_initializer(&element.init, &field, element.name.span());
                }
            }
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    rule_ok!(
        apply_when_fewer_values_than_elements_then_ok,
        "
PROGRAM main
VAR
    a : ARRAY[1..3] OF DINT := [1, 2];
    b : ARRAY[1..4] OF DINT := [2(0), 1, 2];
    c : ARRAY[1..2, 1..2] OF DINT := [1, 2, 3, 4];
END_VAR
END_PROGRAM"
    );

    rule_err!(
        apply_when_variable_initializer_has_more_values_than_elements_then_error,
        "
PROGRAM main
VAR
    a : ARRAY[1..3] OF DINT := [1, 2, 3, 4];
END_VAR
END_PROGRAM",
        [Problem::ArrayInitializerTooManyValues]
    );

    rule_err!(
        apply_when_repetition_exceeds_elements_then_error,
        "
PROGRAM main
VAR
    a : ARRAY[1..3] OF DINT := [1, 3(0)];
END_VAR
END_PROGRAM",
        [Problem::ArrayInitializerTooManyValues]
    );

    rule_err!(
        apply_when_array_type_default_has_too_many_values_then_error,
        "
TYPE
    ARR : ARRAY[1..2] OF INT := [1, 2, 3];
END_TYPE",
        [Problem::ArrayInitializerTooManyValues]
    );

    rule_err!(
        apply_when_structure_field_default_has_too_many_values_then_error,
        "
TYPE
    S : STRUCT arr : ARRAY[1..2] OF INT := [1, 2, 3]; END_STRUCT;
END_TYPE",
        [Problem::ArrayInitializerTooManyValues]
    );

    rule_err!(
        apply_when_structure_initializer_member_has_too_many_values_then_error,
        "
TYPE
    S : STRUCT arr : ARRAY[1..2] OF INT; END_STRUCT;
END_TYPE
PROGRAM main
VAR
    s : S := (arr := [1, 2, 3]);
END_VAR
END_PROGRAM",
        [Problem::ArrayInitializerTooManyValues]
    );

    rule_err_at!(
        apply_when_too_many_values_then_labels_variable,
        "
PROGRAM main
VAR
    limits : ARRAY[1..1] OF DINT := [10, 20];
END_VAR
END_PROGRAM",
        Problem::ArrayInitializerTooManyValues,
        "limits"
    );
}
