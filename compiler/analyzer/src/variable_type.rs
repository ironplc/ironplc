//! Resolves a variable reference to the type of the variable it names.
//!
//! A rule that checks something about a variable's type needs two things: the
//! variable's declared type, and a way to walk from a reference such as
//! `s.field[i]` to the type of the element it names. The declared type comes
//! from the symbol environment, looked up from the scope the rule is in; the
//! walk lives here so that every reader shares one answer rather than each
//! carrying its own copy.
//!
//! The walk goes by [`TypeId`] ([`type_id`]): a variable's declared type, then
//! [`TypeEnvironment::field_type`] for a field, [`TypeEnvironment::element_type`]
//! for a subscript list that gives one subscript for each dimension of the
//! array it subscripts, and [`TypeEnvironment::referenced_type`] for a
//! dereference. A subscript of a reference to an array indexes the array it
//! refers to, as a subscript of its dereference does.
//! It never reads the fields a structure's or function block's representation
//! lists, so a field has one type, the one its declaration records.
//!
//! ```ignore
//! // In the visitor: track the scope the traversal is in.
//! fn enter_scope(&mut self, node: ScopeNode<'_>) { self.scope.enter(&node) }
//! fn exit_scope(&mut self) { self.scope.exit() }
//!
//! let element = variable_type::of(&kind, context, &self.scope.current());
//! ```

use ironplc_dsl::{common::*, core::Id, textual::*, type_id::TypeId};

use crate::{
    semantic_context::SemanticContext,
    semantic_type::{ArrayDimension, SemanticType},
    symbol_environment::{ScopeKind, ScopePath, SymbolEnvironment, SymbolInfo},
    type_environment::TypeEnvironment,
};

/// Resolves the [`SemanticType`] a declaration denotes.
pub(crate) fn resolve_initializer(
    init: &InitialValueAssignmentKind,
    type_env: &TypeEnvironment,
) -> Option<SemanticType> {
    match init {
        InitialValueAssignmentKind::Simple(si) => {
            Some(type_env.get(&si.type_name)?.representation.clone())
        }
        InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer {
            type_name: tn,
            ..
        }) => Some(type_env.get(tn)?.representation.clone()),
        InitialValueAssignmentKind::Structure(si) => {
            Some(type_env.get(&si.type_name)?.representation.clone())
        }
        InitialValueAssignmentKind::FunctionBlock(fbi) => {
            Some(type_env.get(&fbi.type_name)?.representation.clone())
        }
        InitialValueAssignmentKind::Array(ai) => match &ai.spec {
            SpecificationKind::Named(tn) => Some(type_env.get(tn)?.representation.clone()),
            SpecificationKind::Inline(subranges) => {
                let element_type = type_env
                    .get(&subranges.type_name.to_type_name())?
                    .representation
                    .clone();
                Some(SemanticType::Array {
                    element_type: Box::new(element_type),
                    dimensions: vec![],
                })
            }
        },
        _ => None,
    }
}

/// The id of the type of the value the reference `kind` names from `scope`:
/// the declared type of a variable, the declared type of a field, the element
/// type of a subscript, the type a dereference refers to, and for a bit or
/// partial access the `BOOL` or bit string it selects. `None` when a step of
/// the walk has no type, such as a field the record does not have, or for
/// a bare `THIS^` or `SUPER^`, which is an instance rather than a value.
pub(crate) fn type_id(
    kind: &SymbolicVariableKind,
    symbols: &SymbolEnvironment,
    types: &TypeEnvironment,
    scope: &ScopeKind,
) -> Option<TypeId> {
    match kind {
        SymbolicVariableKind::Named(named) => declared_type_id(&named.name, symbols, scope),
        SymbolicVariableKind::Structured(structured) => {
            // A member of `THIS^` or `SUPER^` is found from the function
            // block's own scope, with its declared type.
            if let SymbolicVariableKind::SelfRef(self_ref) = structured.record.as_ref() {
                return self_member(self_ref.kind, &structured.field, symbols, scope)?.type_id;
            }
            let record = type_id(&structured.record, symbols, types, scope)?;
            types.field_type(record, &structured.field)
        }
        SymbolicVariableKind::Array(array) => {
            let subscripted = type_id(&array.subscripted_variable, symbols, types, scope)?;
            // A subscript of a reference to an array indexes the array it
            // refers to, as `r^[i]` does: a TwinCAT `REFERENCE TO`, and a
            // `REF_TO` a subscript follows without `^`.
            let subscripted = types.referenced_type(subscripted).unwrap_or(subscripted);
            // A subscript list subscripts one array, with one subscript for
            // each of its dimensions: `m[1]` on a two-dimensional array and
            // `rows[1, 2]` on an array of arrays select no element.
            if array_dimensions(subscripted, types)
                .is_some_and(|dimensions| dimensions.len() != array.subscripts.len())
            {
                return None;
            }
            types.element_type(subscripted)
        }
        // `p^` is the variable `p` references, so it has the referenced
        // type, not `REF_TO`.
        SymbolicVariableKind::Deref(deref) => {
            types.referenced_type(type_id(&deref.variable, symbols, types, scope)?)
        }
        SymbolicVariableKind::BitAccess(_) => types.id_of(&TypeName::from("BOOL")),
        SymbolicVariableKind::PartialAccess(partial) => {
            types.id_of(&TypeName::from(match partial.size {
                PartialAccessSize::Byte => "BYTE",
                PartialAccessSize::Word => "WORD",
                PartialAccessSize::DWord => "DWORD",
                PartialAccessSize::LWord => "LWORD",
            }))
        }
        // A bare `THIS^` or `SUPER^` is a function block instance, not a
        // value with a type of its own; its members are typed above.
        SymbolicVariableKind::SelfRef(_) => None,
    }
}

/// The dimensions of the array type `id`, or `None` when `id` is not an
/// array type or records no dimensions, so that its subscripts cannot be
/// counted.
pub(crate) fn array_dimensions(id: TypeId, types: &TypeEnvironment) -> Option<&[ArrayDimension]> {
    match &types.get_by_id(id)?.representation {
        SemanticType::Array { dimensions, .. } if !dimensions.is_empty() => Some(dimensions),
        _ => None,
    }
}

/// Resolves the [`SemanticType`] of the variable a reference names: the
/// representation of the type [`type_id`] walks to.
///
/// A bit or partial access answers with the type of the variable it accesses,
/// **not** with what the selection denotes: `x.3` answers `x`'s type rather
/// than `BOOL`, and `w.B1` answers `w`'s type rather than a byte. That is the
/// question an index check asks -- the variable's width is what bounds the
/// index -- and it is the wrong question for a caller asking what type a
/// value read from or written to the reference has, which [`type_id`]
/// answers.
pub(crate) fn of<'a>(
    kind: &SymbolicVariableKind,
    context: &'a SemanticContext,
    scope: &ScopeKind,
) -> Option<&'a SemanticType> {
    // A selection answers with the variable it selects from; see the note
    // on this function. Nothing selects from a selection, so it is only
    // ever the outermost step of a reference.
    let accessed = match kind {
        SymbolicVariableKind::BitAccess(bit_access) => &bit_access.variable,
        SymbolicVariableKind::PartialAccess(partial) => &partial.variable,
        _ => kind,
    };
    let types = context.types();
    let id = type_id(accessed, context.symbols(), types, scope)?;
    Some(&types.get_by_id(id)?.representation)
}

/// The symbol of the member `field` of what `THIS^` or `SUPER^` names from
/// `scope`.
///
/// The field is looked up from the function block's own scope, not from
/// `scope`, so a method parameter or local that hides it does not answer for
/// it. `find` walks the `EXTENDS` chain, and for `SUPER^` it starts at the
/// base so only inherited fields are seen. `None` outside a function block,
/// for `SUPER^` without a base, and for a field the block does not have.
pub(crate) fn self_member<'a>(
    kind: SelfRefKind,
    field: &Id,
    symbols: &'a SymbolEnvironment,
    scope: &ScopeKind,
) -> Option<&'a SymbolInfo> {
    let block = symbols.self_type(scope, kind)?;
    member_of_block(&block, field, symbols)
}

/// The symbol of the member `field` of the function block `block`, found in
/// its own scope and then those of its `EXTENDS` chain.
///
/// `find` falls back to the globals, which are not members. Neither is a
/// `VAR_EXTERNAL` declaration, which is a view of a global: TwinCAT reports
/// `THIS^.g` as no component of the block.
pub(crate) fn member_of_block<'a>(
    block: &TypeName,
    field: &Id,
    symbols: &'a SymbolEnvironment,
) -> Option<&'a SymbolInfo> {
    let block_scope = ScopeKind::Named(ScopePath::from(block.name.clone()));
    let member = symbols.find(field, &block_scope)?;
    (member.scope != ScopeKind::Global && !member.is_external).then_some(member)
}

/// The declared type of the variable `name` names from `scope`: a variable,
/// parameter or result variable, looked up in the symbol environment.
pub(crate) fn declared<'a>(
    name: &Id,
    context: &'a SemanticContext,
    scope: &ScopeKind,
) -> Option<&'a SemanticType> {
    let type_id = declared_type_id(name, context.symbols(), scope)?;
    Some(&context.types().get_by_id(type_id)?.representation)
}

/// The id of the declared type of the variable `name` names from `scope`.
pub(crate) fn declared_type_id(
    name: &Id,
    symbols: &SymbolEnvironment,
    scope: &ScopeKind,
) -> Option<TypeId> {
    symbols.resolve_variable(name, scope).declared()?.type_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_type::ByteSized;
    use crate::symbol_environment::ScopePath;
    use crate::test_helpers::{edition3_options, parse_and_resolve_types_with_options};
    use ironplc_container::CharWidth;
    use ironplc_dsl::visitor::Visitor;
    use ironplc_parser::options::CompilerOptions;
    use spec_test_macro::spec_test;
    use std::convert::Infallible;

    /// A program whose `main` has an instance `f` of a block with a
    /// structure output, a sized string output and an array variable, an
    /// instance `d` of a block that inherits an `INT` input, and an `LINT`.
    /// Each assignment's target is a reference the tests ask about.
    const PROGRAM: &str = "
TYPE Point : STRUCT x : LINT; END_STRUCT; END_TYPE
FUNCTION_BLOCK Block
  VAR_OUTPUT p : Point; s : STRING[8]; END_VAR
  VAR a : ARRAY[1..3] OF INT; END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK Base
  VAR_INPUT count : INT; END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS Base
END_FUNCTION_BLOCK
PROGRAM main
  VAR f : Block; d : Derived; n : LINT; END_VAR
  f.p := 0;
  f.s := 0;
  f.a := 0;
  f.a[2] := 0;
  f.p.x := 0;
  d.count := 0;
  n.3 := 0;
  f.missing := 0;
END_PROGRAM";

    /// The target of each assignment in `library`, in order.
    fn targets(library: &Library) -> Vec<SymbolicVariableKind> {
        struct Targets(Vec<SymbolicVariableKind>);
        impl Visitor<Infallible> for Targets {
            type Value = ();
            fn visit_assignment(&mut self, node: &Assignment) -> Result<(), Infallible> {
                if let Variable::Symbolic(kind) = &node.target {
                    self.0.push(kind.clone());
                }
                Ok(())
            }
        }
        let mut targets = Targets(Vec::new());
        let _ = targets.walk(library);
        targets.0
    }

    /// The representation `of` gives each assignment target of [`PROGRAM`],
    /// asked from the scope of `main`.
    fn representations() -> Vec<Option<SemanticType>> {
        let (library, context) = parse_and_resolve_types_with_options(PROGRAM, &edition3_options());
        let main = ScopeKind::Named(ScopePath::new(vec![Id::from("main")]));
        targets(&library)
            .iter()
            .map(|kind| of(kind, &context, &main).cloned())
            .collect()
    }

    #[spec_test(REQ_ETR_analyzer_004)]
    #[test]
    fn of_when_field_of_function_block_then_declared_type_of_field() {
        let (_, context) = parse_and_resolve_types_with_options(PROGRAM, &edition3_options());
        let point = context
            .types()
            .get(&TypeName::from("Point"))
            .map(|attributes| attributes.representation.clone());
        let types = representations();

        assert_eq!(types[0], point);
        assert_eq!(
            types[1],
            Some(SemanticType::String {
                max_len: Some(8),
                char_width: CharWidth::Narrow,
            })
        );
        assert!(types[2].as_ref().is_some_and(SemanticType::is_array));
        assert_eq!(
            types[3],
            Some(SemanticType::Int {
                size: ByteSized::B16
            })
        );
        assert_eq!(
            types[4],
            Some(SemanticType::Int {
                size: ByteSized::B64
            })
        );
    }

    #[test]
    fn of_when_inherited_field_then_type_the_base_block_declares() {
        assert_eq!(
            representations()[5],
            Some(SemanticType::Int {
                size: ByteSized::B16
            })
        );
    }

    #[test]
    fn of_when_bit_access_then_type_of_variable_accessed() {
        assert_eq!(
            representations()[6],
            Some(SemanticType::Int {
                size: ByteSized::B64
            })
        );
    }

    #[test]
    fn of_when_field_record_does_not_have_then_none() {
        assert_eq!(representations()[7], None);
    }

    /// REQ-ETR-analyzer-008: each subscript list subscripts one array and
    /// gives one subscript for each of its dimensions.
    #[spec_test(REQ_ETR_analyzer_008)]
    #[test]
    fn type_id_when_subscripts_then_element_only_when_one_per_dimension() {
        let program = "
TYPE Row : ARRAY[1..3] OF DINT; END_TYPE
PROGRAM main
  VAR rows : ARRAY[1..2] OF Row; m : ARRAY[1..2, 1..3] OF DINT; END_VAR
  rows[1] := 0;
  rows[1][2] := 0;
  m[1, 2] := 0;
  rows[1, 2] := 0;
  m[1] := 0;
  m[1][2] := 0;
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_options(program, &edition3_options());
        let main = ScopeKind::Named(ScopePath::new(vec![Id::from("main")]));
        let types = context.types();
        let ids: Vec<Option<TypeId>> = targets(&library)
            .iter()
            .map(|kind| type_id(kind, context.symbols(), types, &main))
            .collect();
        let dint = types.id_of(&TypeName::from("DINT"));

        assert_eq!(
            ids,
            [
                types.id_of(&TypeName::from("Row")),
                dint,
                dint,
                None,
                None,
                None
            ]
        );
    }

    /// REQ-ETR-analyzer-009: a subscript of a reference to an array selects
    /// an element of the array it refers to, as a subscript of its
    /// dereference does.
    #[spec_test(REQ_ETR_analyzer_009)]
    #[test]
    fn type_id_when_reference_to_array_subscripted_then_element_of_referenced_array() {
        let program = "
TYPE Row : ARRAY[1..3] OF DINT; END_TYPE
PROGRAM main
  VAR pa : REF_TO ARRAY[1..3] OF DINT; ra : REFERENCE TO ARRAY[1..3] OF DINT;
      prows : REF_TO ARRAY[1..2] OF Row; END_VAR
  pa[2] := 0;
  pa^[2] := 0;
  ra[2] := 0;
  prows[1][2] := 0;
  prows[1] := 0;
  pa[1, 2] := 0;
END_PROGRAM";
        let options = CompilerOptions {
            allow_reference_to: true,
            ..edition3_options()
        };
        let (library, context) = parse_and_resolve_types_with_options(program, &options);
        let main = ScopeKind::Named(ScopePath::new(vec![Id::from("main")]));
        let types = context.types();
        let ids: Vec<Option<TypeId>> = targets(&library)
            .iter()
            .map(|kind| type_id(kind, context.symbols(), types, &main))
            .collect();
        let dint = types.id_of(&TypeName::from("DINT"));

        assert_eq!(
            ids,
            [
                dint,
                dint,
                dint,
                dint,
                types.id_of(&TypeName::from("Row")),
                None
            ]
        );
    }

    #[test]
    fn type_id_when_bit_access_then_bool() {
        let (library, context) = parse_and_resolve_types_with_options(PROGRAM, &edition3_options());
        let main = ScopeKind::Named(ScopePath::new(vec![Id::from("main")]));
        let bit_access = &targets(&library)[6];

        assert_eq!(
            type_id(bit_access, context.symbols(), context.types(), &main),
            context.types().id_of(&TypeName::from("BOOL"))
        );
    }
}
