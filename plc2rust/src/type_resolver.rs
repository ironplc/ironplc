use ironplc_dsl::dsl::*;
use ironplc_dsl::fold::Fold;
use std::collections::HashMap;

use crate::symbol_table;

pub struct TypeResolver {
    types: HashMap<String, TypeDefinitionKind>,
}
impl TypeResolver {
    pub fn apply(library: Library, symbol_table: HashMap<String, TypeDefinitionKind>) -> Library {
        let mut type_resolver = TypeResolver {
            types: symbol_table,
        };
        type_resolver.fold(library)
    }
}

impl Fold for TypeResolver {
    fn fold_type_initializer(&mut self, node: TypeInitializer) -> TypeInitializer {
        match node {
            TypeInitializer::LateResolvedType(name) => {
                // Try to find the type for the specified name.
                // TODO error handling
                let type_kind = self.types.get(&name).unwrap();
                match type_kind {
                    TypeDefinitionKind::Enumeration => TypeInitializer::EnumeratedType {
                        type_name: name,
                        initial_value: None,
                    },
                    TypeDefinitionKind::FunctionBlock => {
                        TypeInitializer::FunctionBlock { type_name: name }
                    }
                    TypeDefinitionKind::Function => {
                        // TODO this is wrong and should be an error
                        TypeInitializer::Structure { type_name: name }
                    }
                    TypeDefinitionKind::Structure => TypeInitializer::Structure { type_name: name },
                }
            }
            _ => node,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::*;
    use crate::type_resolver::TypeResolver;
    use ironplc_dsl::dsl::*;
    use ironplc_dsl::fold::Fold;
    use std::collections::HashMap;

    #[test]
    fn test_resolves_function_block_type() {
        let input = new_library::<String>(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: String::from("LOGGER"),
                var_decls: vec![VarInitKind::late_bound("var_name", "var_type")],
                body: FunctionBlockBody::Statements(vec![]),
            },
        ))
        .unwrap();

        let mut type_map = HashMap::new();
        type_map.insert(String::from("var_type"), TypeDefinitionKind::FunctionBlock);
        let mut type_resolver = TypeResolver { types: type_map };

        let result = type_resolver.fold(input);

        let expected = new_library::<String>(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: String::from("LOGGER"),
                var_decls: vec![VarInitKind::VarInit(VarInitDecl::function_block(
                    "var_name", "var_type",
                ))],
                body: FunctionBlockBody::Statements(vec![]),
            },
        ))
        .unwrap();

        assert_eq!(result, expected)
    }
}
