//! The signatures of the methods of function blocks and interfaces.
//!
//! A method's signature is what a call of it, and a method that overrides
//! it, see: its result and its parameters, by the ids of their types. It is
//! recorded with the block that declares the method (see
//! [`TypeEnvironment::record_block`]), and a block's methods are found
//! through the blocks it `EXTENDS` (see [`TypeEnvironment::method`]). The
//! method prototypes an interface declares have signatures of the same kind,
//! recorded with the interface (see [`TypeEnvironment::record_interface`]).

use ironplc_dsl::common::{
    EdgeDirection, EdgeVarDecl, FunctionReturnType, MethodDeclaration, MethodPrototype, VarDecl,
    VariableType,
};
use ironplc_dsl::core::Id;
use ironplc_dsl::type_id::TypeId;

use super::TypeEnvironment;
use crate::intermediates::string;

/// The signature of a method a function block declares.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MethodSignature {
    /// The method's name, as the block declares it.
    pub(crate) name: Id,
    /// What a call of the method returns.
    pub(crate) result: MethodResult,
    /// The method's `VAR_INPUT`, `VAR_OUTPUT` and `VAR_IN_OUT` variables, in
    /// the order it declares them.
    pub(crate) parameters: Vec<MethodParameter>,
    /// The method's `R_EDGE` and `F_EDGE` inputs, in the order it declares
    /// them. They are `BOOL` inputs, declared apart from the other variables,
    /// so their order among the other inputs is not known.
    pub(crate) edge_inputs: Vec<(Id, EdgeDirection)>,
}

/// What a call of a method returns.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MethodResult {
    /// The method declares no return type: a call of it is a statement.
    None,
    /// The method returns a value of the type with this id; `None` when the
    /// return type did not resolve, which a rule reports.
    Value(Option<TypeId>),
}

/// A `VAR_INPUT`, `VAR_OUTPUT` or `VAR_IN_OUT` variable of a method.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MethodParameter {
    /// The variable's name, as the method declares it.
    pub(crate) name: Id,
    /// Which of the three sections declares it.
    pub(crate) section: VariableType,
    /// The id of the type the variable is declared with; `None` when it did
    /// not resolve, which a rule reports.
    pub(crate) type_id: Option<TypeId>,
}

impl MethodSignature {
    /// The id of the type of the output `output`, when the method declares
    /// an output of that name whose type resolved.
    pub(crate) fn output_type(&self, output: &Id) -> Option<TypeId> {
        self.parameters
            .iter()
            .find(|parameter| {
                parameter.section == VariableType::Output && parameter.name == *output
            })?
            .type_id
    }

    /// The id of the type the method returns, when it declares a return type
    /// that resolved.
    pub(crate) fn result_type(&self) -> Option<TypeId> {
        match self.result {
            MethodResult::Value(type_id) => type_id,
            MethodResult::None => None,
        }
    }
}

impl TypeEnvironment {
    /// The signature of `method`, whose variables already record the ids of
    /// the types they are declared with. A result that is a string of a
    /// stated length enters a sized string type, as
    /// [`Self::result_type_id`] does for any result.
    pub(crate) fn method_signature(&mut self, method: &MethodDeclaration) -> MethodSignature {
        self.signature(
            &method.name,
            method.return_type.as_ref(),
            &method.variables,
            &method.edge_variables,
        )
    }

    /// The signature of the method prototype `prototype` an interface
    /// declares, as [`Self::method_signature`] gives a method's: a method
    /// provides the prototype when the two signatures match.
    pub(crate) fn prototype_signature(&mut self, prototype: &MethodPrototype) -> MethodSignature {
        self.signature(
            &prototype.name,
            prototype.return_type.as_ref(),
            &prototype.variables,
            &prototype.edge_variables,
        )
    }

    /// Whether the signatures `a` and `b` are the same: the same result
    /// type, the same parameters (names, sections and types, in order) and
    /// the same edge inputs. Types are compared by
    /// [`Self::same_type_if_resolved`], and locals are not part of a
    /// signature.
    pub(crate) fn same_signature(&self, a: &MethodSignature, b: &MethodSignature) -> bool {
        let same_result = match (&a.result, &b.result) {
            (MethodResult::None, MethodResult::None) => true,
            (MethodResult::Value(a), MethodResult::Value(b)) => self.same_type_if_resolved(*a, *b),
            _ => false,
        };
        same_result
            && a.parameters.len() == b.parameters.len()
            && a.parameters.iter().zip(&b.parameters).all(|(a, b)| {
                a.name == b.name
                    && a.section == b.section
                    && self.same_type_if_resolved(a.type_id, b.type_id)
            })
            && a.edge_inputs == b.edge_inputs
    }

    /// The signature of the method or method prototype `name` with the
    /// return type, variables and edge inputs given.
    fn signature(
        &mut self,
        name: &Id,
        return_type: Option<&FunctionReturnType>,
        variables: &[VarDecl],
        edge_variables: &[EdgeVarDecl],
    ) -> MethodSignature {
        let result = match return_type {
            Some(return_type) => MethodResult::Value(self.result_type_id(return_type)),
            None => MethodResult::None,
        };
        let parameters = variables
            .iter()
            .filter(|decl| {
                matches!(
                    decl.var_type,
                    VariableType::Input | VariableType::Output | VariableType::InOut
                )
            })
            .filter_map(|decl| {
                Some(MethodParameter {
                    name: decl.identifier.symbolic_id()?.clone(),
                    section: decl.var_type.clone(),
                    type_id: decl.type_id,
                })
            })
            .collect();
        let edge_inputs = edge_variables
            .iter()
            .map(|edge| (edge.identifier.clone(), edge.direction.clone()))
            .collect();
        MethodSignature {
            name: name.clone(),
            result,
            parameters,
            edge_inputs,
        }
    }

    /// The id of the type of a result of `return_type`. A string of a stated
    /// length gets a type of its own that records the length as its capacity
    /// (REQ-LOW-analyzer-053), as a declared string does
    /// (`xform_resolve_decl_types`); any other result has the type its name
    /// names.
    pub(crate) fn result_type_id(&mut self, return_type: &FunctionReturnType) -> Option<TypeId> {
        let named = self.id_of(&return_type.to_type_name())?;
        match return_type {
            FunctionReturnType::String(spec) | FunctionReturnType::WString(spec)
                if spec.length.is_some() =>
            {
                Some(self.insert_sized_string(named, string::from_spec(spec)))
            }
            FunctionReturnType::String(_)
            | FunctionReturnType::WString(_)
            | FunctionReturnType::Named(_) => Some(named),
        }
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::common::{EdgeDirection, TypeName, VariableType};
    use ironplc_dsl::core::Id;
    use rstest::rstest;

    use super::{MethodParameter, MethodResult};
    use crate::semantic_context::SemanticContext;
    use crate::semantic_type::SemanticType;
    use crate::test_helpers::{fb_inheritance_options, parse_and_resolve_types_with_options};
    use crate::type_environment::TypeEnvironment;

    fn resolve(program: &str) -> SemanticContext {
        parse_and_resolve_types_with_options(program, &fb_inheritance_options()).1
    }

    fn id(types: &TypeEnvironment, name: &str) -> ironplc_dsl::type_id::TypeId {
        types.id_of(&TypeName::from(name)).unwrap()
    }

    const BASES: &str = "
TYPE Pair : STRUCT f : INT; END_STRUCT; END_TYPE
FUNCTION_BLOCK FB_Base
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Plain
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_OfStructure EXTENDS Pair
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_OfStandard EXTENDS TON
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_OfMissing EXTENDS FB_Missing
END_FUNCTION_BLOCK";

    #[rstest]
    #[case::declared_base("FB_Derived", Some("FB_Base"))]
    #[case::no_base("FB_Plain", None)]
    #[case::structure("FB_OfStructure", None)]
    #[case::standard_block("FB_OfStandard", None)]
    #[case::undeclared("FB_OfMissing", None)]
    fn base_of_when_extends_then_only_a_declared_block(
        #[case] block: &str,
        #[case] expected: Option<&str>,
    ) {
        let context = resolve(BASES);
        let types = context.types();

        assert_eq!(
            expected.map(|base| id(types, base)),
            types.base_of(id(types, block))
        );
    }

    const METHODS: &str = "
FUNCTION_BLOCK FB_Root
METHOD Describe : INT
END_METHOD
METHOD Name : STRING[10]
END_METHOD
METHOD Put
VAR_INPUT a : INT; END_VAR
VAR_OUTPUT done : BOOL; END_VAR
VAR_IN_OUT io : DINT; END_VAR
VAR tmp : INT; END_VAR
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Mid EXTENDS FB_Root
METHOD Describe : BOOL
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Leaf EXTENDS FB_Mid
METHOD Run
VAR_INPUT go : BOOL R_EDGE; END_VAR
END_METHOD
END_FUNCTION_BLOCK";

    /// The block that declares `method` called on `block`, by name.
    fn declaring(types: &TypeEnvironment, block: &str, method: &str) -> Option<String> {
        let (declaring, _) = types.method(id(types, block), &Id::from(method))?;
        Some(types.name_of(declaring)?.to_string())
    }

    #[rstest]
    #[case::own("FB_Leaf", "Run", Some("FB_Leaf"))]
    #[case::inherited("FB_Leaf", "Put", Some("FB_Root"))]
    #[case::overridden_nearest_wins("FB_Leaf", "Describe", Some("FB_Mid"))]
    #[case::base_does_not_see_derived("FB_Root", "Run", None)]
    #[case::not_declared("FB_Leaf", "Missing", None)]
    fn method_when_called_on_block_then_declared_by_nearest_block(
        #[case] block: &str,
        #[case] method: &str,
        #[case] expected: Option<&str>,
    ) {
        let context = resolve(METHODS);

        assert_eq!(
            expected.map(str::to_string),
            declaring(context.types(), block, method)
        );
    }

    #[test]
    fn method_when_overridden_then_result_of_override() {
        let context = resolve(METHODS);
        let types = context.types();
        let (_, describe) = types
            .method(id(types, "FB_Leaf"), &Id::from("Describe"))
            .unwrap();

        assert_eq!(
            MethodResult::Value(Some(id(types, "BOOL"))),
            describe.result
        );
    }

    #[test]
    fn method_signature_when_parameters_then_inputs_outputs_and_in_outs_in_order() {
        let context = resolve(METHODS);
        let types = context.types();
        let (_, put) = types
            .method(id(types, "FB_Root"), &Id::from("Put"))
            .unwrap();
        let parameter = |name: &str, section: VariableType, type_name: &str| MethodParameter {
            name: Id::from(name),
            section,
            type_id: Some(id(types, type_name)),
        };

        assert_eq!(MethodResult::None, put.result);
        assert_eq!(
            vec![
                parameter("a", VariableType::Input, "INT"),
                parameter("done", VariableType::Output, "BOOL"),
                parameter("io", VariableType::InOut, "DINT"),
            ],
            put.parameters
        );
        assert_eq!(Some(id(types, "BOOL")), put.output_type(&Id::from("done")));
        assert_eq!(None, put.output_type(&Id::from("a")));
    }

    #[test]
    fn method_signature_when_sized_string_result_then_type_of_its_capacity() {
        let context = resolve(METHODS);
        let types = context.types();
        let (_, name) = types
            .method(id(types, "FB_Root"), &Id::from("Name"))
            .unwrap();
        let result = name.result_type().unwrap();

        assert_ne!(id(types, "STRING"), result);
        assert!(matches!(
            types.get_by_id(result).unwrap().representation,
            SemanticType::String {
                max_len: Some(10),
                ..
            }
        ));
    }

    #[test]
    fn method_signature_when_edge_input_then_recorded_apart() {
        let context = resolve(METHODS);
        let types = context.types();
        let (_, run) = types
            .method(id(types, "FB_Leaf"), &Id::from("Run"))
            .unwrap();

        assert_eq!(
            vec![(Id::from("go"), EdgeDirection::Rising)],
            run.edge_inputs
        );
    }

    #[test]
    fn method_signature_when_parameter_type_unresolved_then_recorded_as_unknown() {
        let context = resolve(
            "
FUNCTION_BLOCK FB_A
METHOD M
VAR_INPUT x : Undeclared; END_VAR
END_METHOD
END_FUNCTION_BLOCK",
        );
        let types = context.types();
        let (_, m) = types.method(id(types, "FB_A"), &Id::from("M")).unwrap();

        assert_eq!(
            vec![MethodParameter {
                name: Id::from("x"),
                section: VariableType::Input,
                type_id: None,
            }],
            m.parameters
        );
    }
}
