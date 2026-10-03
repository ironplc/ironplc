//! Transformation pass that records, on every variable declaration, the
//! [`TypeId`] of the type it declares.
//!
//! A declaration that names its type (`x : INT`, `p : Point`) takes the id of
//! that name. A declaration that spells its type out in place has no name to
//! look up, so this pass enters that type in the environment as an anonymous
//! type and records the new id:
//!
//! ```ignore
//! VAR
//!     a : ARRAY[1..2] OF DINT;   (* an anonymous array type *)
//!     e : (RED, GREEN);          (* an anonymous enumeration *)
//!     s : INT (0..10);            (* an anonymous subrange *)
//!     r : REF_TO INT;            (* an anonymous reference type *)
//! END_VAR
//! ```
//!
//! Each such declaration declares a type of its own, so two declarations
//! that spell the same shape get two ids (ADR-0055).
//!
//! A declaration whose type cannot be resolved keeps `type_id: None`. The
//! rules that check declarations report why; this pass stays silent.
use ironplc_dsl::common::*;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::type_id::TypeId;

use crate::intermediates::{array, enumeration, subrange};
use crate::type_environment::TypeEnvironment;

pub fn apply(
    lib: Library,
    type_environment: &mut TypeEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    let mut resolver = DeclTypeResolver { type_environment };
    resolver.fold_library(lib).map_err(|e| vec![e])
}

struct DeclTypeResolver<'a> {
    type_environment: &'a mut TypeEnvironment,
}

impl DeclTypeResolver<'_> {
    /// The id of the type `init` declares, entering it as an anonymous type
    /// when it has no name. `name` labels the anonymous type's diagnostics,
    /// which are discarded.
    fn declared_type_id(
        &mut self,
        name: &TypeName,
        init: &InitialValueAssignmentKind,
    ) -> Option<TypeId> {
        let env = &*self.type_environment;
        let anonymous = match init {
            InitialValueAssignmentKind::None(_) => return None,
            InitialValueAssignmentKind::Simple(si) => return env.id_of(&si.type_name),
            // A sized string is its unsized elementary type for now.
            InitialValueAssignmentKind::String(si) => return env.id_of(&si.type_name()),
            InitialValueAssignmentKind::EnumeratedType(e) => return env.id_of(&e.type_name),
            InitialValueAssignmentKind::FunctionBlock(fb) => return env.id_of(&fb.type_name),
            InitialValueAssignmentKind::FunctionBlockCall(fbc) => return env.id_of(&fbc.type_name),
            InitialValueAssignmentKind::Structure(s) => return env.id_of(&s.type_name),
            InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer {
                type_name,
                ..
            }) => return env.id_of(type_name),
            InitialValueAssignmentKind::SimpleExpr(se) => return env.id_of(&se.type_name),
            InitialValueAssignmentKind::EnumeratedValues(values) => {
                enumeration::try_from_values(values, None).ok()?
            }
            InitialValueAssignmentKind::Subrange(spec) => {
                match subrange::try_from(name, spec, env).ok()? {
                    subrange::IntermediateResult::Type(attributes) => attributes,
                    subrange::IntermediateResult::Alias(alias) => return env.id_of(&alias),
                }
            }
            InitialValueAssignmentKind::Array(a) => {
                match array::try_from(name, &a.spec, env).ok()? {
                    array::IntermediateResult::Type(attributes) => {
                        let id = self.type_environment.insert_anonymous(attributes);
                        if let SpecificationKind::Inline(elements) = &a.spec {
                            self.type_environment.record_array_element(id, elements);
                        }
                        return Some(id);
                    }
                    array::IntermediateResult::Alias(alias) => return env.id_of(&alias),
                }
            }
            // A reference type is one type however often it is spelled
            // (see `TypeEnvironment::reference_to`).
            InitialValueAssignmentKind::Reference(r) => {
                let target = match &r.target {
                    ReferenceTarget::Named(target) => env.id_of(target)?,
                    ReferenceTarget::Array(subranges) => {
                        let spec = SpecificationKind::Inline(subranges.clone());
                        match array::try_from(name, &spec, env).ok()? {
                            array::IntermediateResult::Type(attributes) => {
                                let id = self.type_environment.insert_anonymous(attributes);
                                self.type_environment.record_array_element(id, subranges);
                                id
                            }
                            array::IntermediateResult::Alias(alias) => env.id_of(&alias)?,
                        }
                    }
                };
                return self.type_environment.reference_to(target);
            }
        };
        Some(self.type_environment.insert_anonymous(anonymous))
    }
}

impl Fold<Diagnostic> for DeclTypeResolver<'_> {
    fn fold_var_decl(&mut self, node: VarDecl) -> Result<VarDecl, Diagnostic> {
        let name = match node.identifier.symbolic_id() {
            Some(id) => TypeName::from_id(id),
            None => TypeName::from("_"),
        };
        let type_id = self.declared_type_id(&name, &node.initializer);
        Ok(VarDecl { type_id, ..node })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intermediate_type::IntermediateType;
    use crate::semantic_context::SemanticContext;
    use crate::test_helpers::parse_and_resolve_types_with_options;
    use ironplc_dsl::visitor::Visitor;
    use ironplc_parser::options::{CompilerOptions, Dialect};
    use std::collections::HashMap;
    use std::convert::Infallible;

    /// Every symbolic declaration's recorded type id, by variable name.
    fn declared_ids(library: &Library) -> HashMap<String, Option<TypeId>> {
        struct Collect(HashMap<String, Option<TypeId>>);
        impl Visitor<Infallible> for Collect {
            type Value = ();
            fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
                if let Some(id) = node.identifier.symbolic_id() {
                    self.0.insert(id.to_string(), node.type_id);
                }
                Ok(())
            }
        }
        let mut collect = Collect(HashMap::new());
        let _ = collect.walk(library);
        collect.0
    }

    /// Resolves `program` under edition 3, which has `REF_TO`.
    fn resolve(program: &str) -> (Library, SemanticContext) {
        parse_and_resolve_types_with_options(
            program,
            &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
        )
    }

    const PROGRAM: &str = "
TYPE
  POINT : STRUCT x : DINT; END_STRUCT;
  ARR : ARRAY[1..2] OF DINT;
END_TYPE
PROGRAM main
VAR
  n : DINT;
  p : POINT;
  na : ARR;
  a : ARRAY[1..2] OF DINT;
  b : ARRAY[1..2] OF DINT;
  e : (RED, GREEN);
  r : REF_TO INT;
  t : STRING[8];
END_VAR
END_PROGRAM
";

    #[test]
    fn apply_when_named_type_then_id_of_name() {
        let (library, context) = resolve(PROGRAM);
        let ids = declared_ids(&library);
        let types = context.types();

        assert_eq!(ids["n"], types.id_of(&TypeName::from("DINT")));
        assert_eq!(ids["p"], types.id_of(&TypeName::from("POINT")));
        assert_eq!(ids["t"], types.id_of(&TypeName::from("STRING")));
    }

    #[test]
    fn apply_when_inline_array_then_anonymous_array_with_dimensions() {
        let (library, context) = resolve(PROGRAM);
        let id = declared_ids(&library)["a"].unwrap();

        assert_eq!(context.types().name_of(id), None);
        match &context.types().get_by_id(id).unwrap().representation {
            IntermediateType::Array { dimensions, .. } => assert_eq!(dimensions.len(), 1),
            other => panic!("expected an array, got {other:?}"),
        }
    }

    #[test]
    fn apply_when_same_shape_twice_then_two_ids() {
        let (library, _) = resolve(PROGRAM);
        let ids = declared_ids(&library);

        assert_ne!(ids["a"].unwrap(), ids["b"].unwrap());
    }

    #[rstest::rstest]
    #[case::enumeration("e")]
    #[case::reference("r")]
    fn apply_when_inline_type_then_anonymous_id(#[case] variable: &str) {
        let (library, context) = resolve(PROGRAM);
        let id = declared_ids(&library)[variable].unwrap();

        assert_eq!(context.types().name_of(id), None);
        assert!(context.types().get_by_id(id).is_some());
    }

    #[test]
    fn apply_when_named_array_then_id_of_array_type() {
        let (library, context) = resolve(PROGRAM);

        assert_eq!(
            declared_ids(&library)["na"],
            context.types().id_of(&TypeName::from("ARR"))
        );
    }

    /// An inline subrange cannot be written in a declaration, but the
    /// initializer can hold one, so the pass has an answer for it.
    #[test]
    fn declared_type_id_when_inline_subrange_then_anonymous_subrange() {
        let (_, mut context) = resolve(PROGRAM);
        let bound = |value: &str| {
            SignedIntegerRef::Literal(
                SignedInteger::new(value, ironplc_dsl::core::SourceSpan::default()).unwrap(),
            )
        };
        let init = InitialValueAssignmentKind::Subrange(SpecificationKind::Inline(
            SubrangeSpecification {
                type_name: ElementaryTypeName::INT,
                subrange: Subrange {
                    start: bound("0"),
                    end: bound("10"),
                },
            },
        ));
        let mut resolver = DeclTypeResolver {
            type_environment: context.types_mut(),
        };

        let id = resolver
            .declared_type_id(&TypeName::from("s"), &init)
            .unwrap();

        assert_eq!(resolver.type_environment.name_of(id), None);
        assert!(resolver
            .type_environment
            .get_by_id(id)
            .unwrap()
            .representation
            .is_subrange());
    }

    const ARRAYS: &str = "
TYPE
  POINT : STRUCT x : DINT; END_STRUCT;
  POINTS : ARRAY[1..2] OF POINT;
  MORE_POINTS : POINTS;
END_TYPE
PROGRAM main
VAR
  inline_points : ARRAY[1..2] OF POINT;
  named_points : POINTS;
  alias_points : MORE_POINTS;
  ints : ARRAY[1..2] OF INT;
  refs : ARRAY[1..2] OF REF_TO INT;
  ref_points : REF_TO ARRAY[1..2] OF POINT;
END_VAR
END_PROGRAM
";

    /// The element type of the array variable `name` declares.
    fn element_of(variable: &str) -> (Option<TypeId>, SemanticContext) {
        let (library, context) = resolve(ARRAYS);
        let array = declared_ids(&library)[variable].unwrap();
        (context.types().element_type(array), context)
    }

    #[rstest::rstest]
    #[case::inline("inline_points")]
    #[case::named("named_points")]
    #[case::alias("alias_points")]
    fn element_type_when_array_of_structure_then_structure(#[case] variable: &str) {
        let (element, context) = element_of(variable);
        assert_eq!(element, context.types().id_of(&TypeName::from("POINT")));
    }

    #[test]
    fn element_type_when_array_of_elementary_then_elementary() {
        let (element, context) = element_of("ints");
        assert_eq!(element, context.types().id_of(&TypeName::from("INT")));
    }

    #[test]
    fn element_type_when_array_of_references_then_reference_to_element() {
        let (element, context) = element_of("refs");
        let types = context.types();
        assert_eq!(
            types.referenced_type(element.unwrap()),
            types.id_of(&TypeName::from("INT"))
        );
    }

    #[test]
    fn element_type_when_reference_to_array_then_array_element() {
        let (library, context) = resolve(ARRAYS);
        let types = context.types();
        let reference = declared_ids(&library)["ref_points"].unwrap();
        let array = types.referenced_type(reference).unwrap();
        assert_eq!(
            types.element_type(array),
            types.id_of(&TypeName::from("POINT"))
        );
    }

    #[test]
    fn element_type_when_not_array_then_none() {
        let (_library, context) = resolve(ARRAYS);
        let types = context.types();
        let point = types.id_of(&TypeName::from("POINT")).unwrap();
        assert_eq!(types.element_type(point), None);
    }
}
