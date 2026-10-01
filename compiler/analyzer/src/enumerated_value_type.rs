//! The type of an enumerated value written without its type (`GREEN`
//! rather than `COLOR#GREEN`).
//!
//! Two enumerations may declare the same value name, so the name alone does
//! not say which enumeration is meant. The value takes its type from where
//! it is used:
//!
//! ```ignore
//! TYPE
//!     COLOR : (RED, GREEN);
//!     LIGHT : (OFF, GREEN);
//! END_TYPE
//! VAR c : COLOR; l : LIGHT; e : (IDLE, GREEN); END_VAR
//!
//! c := GREEN;          (* the assignment target's type: COLOR *)
//! IF l = GREEN THEN    (* the other operand's type: LIGHT *)
//! e := GREEN;          (* the anonymous type of e *)
//! ```
//!
//! Without such a context, the value has the type of the one enumeration in
//! scope that declares it. When several do, it is ambiguous (`P2043`).
//!
//! The enumerations in scope are every named enumeration, and the anonymous
//! enumerations of the variables in scope (`e : (IDLE, GREEN)`).

use std::collections::HashMap;

use ironplc_dsl::common::{EnumeratedValue, Library, LibraryElementKind, TypeName, VarDecl};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::type_id::TypeId;
use ironplc_problems::Problem;

use crate::enumeration_members::EnumerationMembers;
use crate::type_environment::TypeEnvironment;

/// What the place an unqualified enumerated value is used in expects.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Context<'a> {
    /// A value of the type `TypeId` identifies.
    Type(TypeId),
    /// A value of an enumeration with these members, whose id is not known
    /// there: a structure field's type is recorded as a representation.
    Members(&'a EnumerationMembers),
}

/// A [`Context`] that owns its members, so it can be held while the
/// expression it applies to is changed.
pub(crate) enum OwnedContext {
    Type(TypeId),
    Members(EnumerationMembers),
}

impl From<Context<'_>> for OwnedContext {
    fn from(context: Context<'_>) -> Self {
        match context {
            Context::Type(id) => OwnedContext::Type(id),
            Context::Members(members) => OwnedContext::Members(members.clone()),
        }
    }
}

impl OwnedContext {
    pub(crate) fn get(&self) -> Context<'_> {
        match self {
            OwnedContext::Type(id) => Context::Type(*id),
            OwnedContext::Members(members) => Context::Members(members),
        }
    }
}

/// What the type of an unqualified enumerated value resolves to.
#[derive(Debug, PartialEq)]
pub(crate) enum EnumeratedValueType {
    /// The type of the value.
    Resolved(TypeId),
    /// Several enumerations in scope declare the value, and nothing where it
    /// is used says which one is meant.
    Ambiguous,
    /// No enumeration in scope declares the value.
    Undeclared,
}

/// The type of the unqualified enumerated value `value`.
///
/// `context` is what the place of use expects, if any: the value has its
/// type when it is an enumeration that declares `value`, or for members, the
/// type of the first candidate with those members. Otherwise `candidates`,
/// the enumerations in scope, decide: the value has the type of the one that
/// declares it. Candidates with the same members (an enumeration and its
/// aliases, or two declarations that spell the same list) count as one,
/// and the first of them is taken.
pub(crate) fn resolve(
    types: &TypeEnvironment,
    value: &Id,
    context: Option<Context>,
    candidates: impl IntoIterator<Item = TypeId>,
) -> EnumeratedValueType {
    let candidates: Vec<TypeId> = candidates.into_iter().collect();
    match context {
        Some(Context::Type(context)) => {
            if members(types, context).is_some_and(|m| m.contains(value)) {
                return EnumeratedValueType::Resolved(context);
            }
        }
        Some(Context::Members(expected)) if expected.contains(value) => {
            let same = candidates
                .iter()
                .find(|id| members(types, **id).is_some_and(|m| m.same_members(expected)));
            if let Some(id) = same {
                return EnumeratedValueType::Resolved(*id);
            }
        }
        Some(Context::Members(_)) | None => {}
    }

    let mut found: Option<(TypeId, &EnumerationMembers)> = None;
    for candidate in candidates {
        let Some(candidate_members) = members(types, candidate) else {
            continue;
        };
        if !candidate_members.contains(value) {
            continue;
        }
        match found {
            None => found = Some((candidate, candidate_members)),
            Some((_, first)) if first.same_members(candidate_members) => {}
            Some(_) => return EnumeratedValueType::Ambiguous,
        }
    }
    match found {
        Some((id, _)) => EnumeratedValueType::Resolved(id),
        None => EnumeratedValueType::Undeclared,
    }
}

/// The ids of the named enumeration types, in id order.
pub(crate) fn named_enumerations(types: &TypeEnvironment) -> Vec<TypeId> {
    let mut ids: Vec<TypeId> = types
        .iter_ids()
        .filter(|(id, attributes)| {
            types.name_of(*id).is_some() && attributes.representation.is_enumeration()
        })
        .map(|(id, _)| id)
        .collect();
    ids.sort();
    ids
}

/// The type of each variable of each function block, own and inherited
/// through `EXTENDS`, by function block and variable name: the type a value
/// passed to an input in a call (`fb(i := R)`) is expected to have.
pub(crate) fn function_block_inputs(
    lib: &Library,
    inherited_fields: &HashMap<TypeName, Vec<VarDecl>>,
) -> HashMap<TypeName, HashMap<Id, TypeId>> {
    let typed = |decl: &VarDecl| Some((decl.identifier.symbolic_id()?.clone(), decl.type_id?));
    lib.elements
        .iter()
        .filter_map(|element| match element {
            LibraryElementKind::FunctionBlockDeclaration(fb) => Some(fb),
            _ => None,
        })
        .map(|fb| {
            let inherited = inherited_fields.get(&fb.name).into_iter().flatten();
            let fields = inherited.chain(&fb.variables).filter_map(typed).collect();
            (fb.name.clone(), fields)
        })
        .collect()
}

/// The problem an unqualified enumerated value whose type is ambiguous is.
pub(crate) fn ambiguous(value: &EnumeratedValue) -> Diagnostic {
    Diagnostic::problem(
        Problem::EnumValueAmbiguous,
        Label::span(value.span(), "Enumerated value"),
    )
    .with_context_id("value", &value.value)
}

fn members(types: &TypeEnvironment, id: TypeId) -> Option<&EnumerationMembers> {
    types.get_by_id(id)?.representation.enumeration_members()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::parse_and_resolve_types_with_context;
    use ironplc_dsl::common::TypeName;

    fn context_of(source: &str) -> crate::semantic_context::SemanticContext {
        parse_and_resolve_types_with_context(source).1
    }

    fn id(types: &TypeEnvironment, name: &str) -> TypeId {
        types.id_of(&TypeName::from(name)).unwrap()
    }

    const TWO_COLORS: &str = "TYPE COLOR : (RED, GREEN); LIGHT : (OFF, GREEN); END_TYPE";

    #[test]
    fn resolve_when_context_declares_value_then_context_type() {
        let context = context_of(TWO_COLORS);
        let types = context.types();
        let light = id(types, "LIGHT");

        let result = resolve(
            types,
            &Id::from("GREEN"),
            Some(Context::Type(light)),
            named_enumerations(types),
        );

        assert_eq!(result, EnumeratedValueType::Resolved(light));
    }

    #[test]
    fn resolve_when_no_context_and_two_candidates_then_ambiguous() {
        let context = context_of(TWO_COLORS);
        let types = context.types();

        let result = resolve(types, &Id::from("GREEN"), None, named_enumerations(types));

        assert_eq!(result, EnumeratedValueType::Ambiguous);
    }

    #[test]
    fn resolve_when_context_not_an_enumeration_then_only_candidate() {
        let context = context_of(TWO_COLORS);
        let types = context.types();
        let dint = types.id_of(&TypeName::from("DINT")).unwrap();

        let result = resolve(
            types,
            &Id::from("RED"),
            Some(Context::Type(dint)),
            named_enumerations(types),
        );

        assert_eq!(result, EnumeratedValueType::Resolved(id(types, "COLOR")));
    }

    #[test]
    fn resolve_when_alias_then_counts_once_and_takes_base() {
        let context = context_of("TYPE COLOR : (RED, GREEN); PAINT : COLOR; END_TYPE");
        let types = context.types();

        let result = resolve(types, &Id::from("GREEN"), None, named_enumerations(types));

        assert_eq!(result, EnumeratedValueType::Resolved(id(types, "COLOR")));
    }

    #[test]
    fn resolve_when_no_candidate_declares_value_then_undeclared() {
        let context = context_of(TWO_COLORS);
        let types = context.types();

        let result = resolve(types, &Id::from("BLUE"), None, named_enumerations(types));

        assert_eq!(result, EnumeratedValueType::Undeclared);
    }
}
