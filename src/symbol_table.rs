use crate::ironplc_dsl::dsl::*;
use crate::ironplc_dsl::visitor::{walk_library, LibraryVisitor};
use std::collections::HashMap;

pub fn from(lib: &Library) -> HashMap<String, TypeDefinitionKind>{
    let type_map = HashMap::new();
    let mut visitor = TypeDefinitionFinder { types: type_map };
    walk_library(&mut visitor, lib);
    return type_map;
}



// Finds types that are valid as variable types. These include enumerations,
// function blocks, functions, structures.
struct TypeDefinitionFinder {
    types: HashMap<String, TypeDefinitionKind>,
}
impl ironplc_dsl::visitor::LibraryVisitor<()> for TypeDefinitionFinder {
    fn visit_configuration_declaration(&mut self, l: &ConfigurationDeclaration) {}
    fn visit_data_type_declaration(&mut self, dts: &Vec<EnumerationDeclaration>) {
        for dt in dts {
            self.types
                .insert(dt.name.clone(), TypeDefinitionKind::Enumeration);
        }
    }
    fn visit_function_declaration(&mut self, l: &FunctionDeclaration) {}
    fn visit_function_block_declaration(&mut self, l: &FunctionBlockDeclaration) {}
    fn visit_program_declaration(&mut self, l: &ProgramDeclaration) {}
}

struct LateBoundTypeResolver {
    types: HashMap<String, String>,
}
impl ironplc_dsl::visitor::LibraryVisitor<()> for LateBoundTypeResolver {
    fn visit_configuration_declaration(&mut self, l: &ConfigurationDeclaration) {}
    fn visit_data_type_declaration(&mut self, dts: &Vec<EnumerationDeclaration>) {}
    fn visit_function_declaration(&mut self, l: &FunctionDeclaration) {}
    fn visit_function_block_declaration(&mut self, fb: &FunctionBlockDeclaration) {
        for var_decl in &fb.var_decls {
            match var_decl {
                VarInitKind::LocatedVarInit(located) => {
                    if let TypeInitializer::LateResolvedType(type_name) = &located.initializer {
                        let type_kind = self.types.get(type_name);
                        /*located.initializer = TypeInitializer::FunctionBlock{
                            type_name: type_name.to_string(),
                        };*/
                    }
                }
                VarInitKind::VarInit(var) => {
                    if let Some(TypeInitializer::LateResolvedType(tn)) = &var.initializer {}
                }
            }
        }
    }
    fn visit_program_declaration(&mut self, l: &ProgramDeclaration) {}
}
