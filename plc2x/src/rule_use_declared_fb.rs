//! Semantic rule that reference to a function block must be to a function
//! block that is declared.
//!
//! ## Passes
//!
//! FUNCTION_BLOCK Callee
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK Caller
//!    VAR
//!       FB_INSTANCE : Callee;
//!    END_VAR
//!    FB_INSTANCE();
//! END_FUNCTION_BLOCK
//!
//! ## Fails (Incorrect Parameters)
//!
//! FUNCTION_BLOCK Callee
//!    VAR_INPUT
//!       IN1: BOOL;
//!    END_VAR
//! END_FUNCTION_BLOCK
//!     
//! FUNCTION_BLOCK Caller
//!    VAR
//!       FB_INSTANCE : Callee;
//!    END_VAR
//!    FB_INSTANCE(IN1 := TRUE, BAR := TRUE);
//! END_FUNCTION_BLOCK
use ironplc_dsl::{
    ast::*,
    dsl::*,
    visitor::{
        visit_function_block_declaration, visit_function_declaration, visit_program_declaration,
        Visitor,
    },
};
use std::collections::HashMap;

use crate::error::SemanticDiagnostic;

trait FindIOVariable {
    fn find_input(&self, name: &Id) -> Option<&VarInitDecl>;
    fn find_output(&self, name: &Id) -> Option<&VarInitDecl>;
}

impl FindIOVariable for FunctionBlockDeclaration {
    fn find_input(&self, name: &Id) -> Option<&VarInitDecl> {
        match self.inputs.iter().find(|item| item.name.eq(name)) {
            Some(v) => return Some(v),
            None => {}
        }

        self.inouts.iter().find(|item| item.name.eq(name))
    }

    fn find_output(&self, name: &Id) -> Option<&VarInitDecl> {
        match self.outputs.iter().find(|item| item.name.eq(name)) {
            Some(v) => return Some(v),
            None => {}
        }

        self.inouts.iter().find(|item| item.name.eq(name))
    }
}

pub fn apply(lib: &Library) -> Result<(), SemanticDiagnostic> {
    // Collect the names from the library into a map so that
    // we can quickly look up invocations
    let mut function_blocks = HashMap::new();
    for x in lib.elems.iter() {
        match x {
            LibraryElement::FunctionBlockDeclaration(fb) => {
                function_blocks.insert(fb.name.clone(), fb);
            }
            _ => {}
        }
    }

    // Walk the library to find all references to function blocks
    let mut visitor = RuleFunctionBlockUse::new(&function_blocks);
    visitor.walk(lib)
}

struct RuleFunctionBlockUse<'a> {
    // Map of the name of a function block declaration to the
    // declaration itself.
    function_blocks: &'a HashMap<Id, &'a FunctionBlockDeclaration>,

    // Map of variable name to the function block name that is the implementation
    var_to_fb: HashMap<Id, Id>,
}
impl<'a> RuleFunctionBlockUse<'a> {
    fn new(decls: &'a HashMap<Id, &'a FunctionBlockDeclaration>) -> Self {
        RuleFunctionBlockUse {
            function_blocks: decls,
            var_to_fb: HashMap::new(),
        }
    }

    fn check_inputs(
        function_block: &FunctionBlockDeclaration,
        params: &Vec<ParamAssignment>,
    ) -> Result<(), SemanticDiagnostic> {
        // Sort the inputs as either named or positional
        let mut named: Vec<&NamedInput> = vec![];
        let mut positional: Vec<&PositionalInput> = vec![];
        for param in params.iter() {
            match param {
                ParamAssignment::NamedInput(n) => {
                    named.push(&n);
                }
                ParamAssignment::PositionalInput(p) => {
                    positional.push(&p);
                }
                ParamAssignment::Output {
                    not: _,
                    src: _,
                    tgt: _,
                } => {
                    // TODO what's this about
                }
            }
        }

        // Don't allow a mixture so assert that either named is empty or
        // positional is empty
        if named.len() > 0 && positional.len() > 0 {
            return SemanticDiagnostic::error("S0001", format!(
                "Function call {} mixes named and positional input arguments",
                function_block.name
            ));
        }

        if !named.is_empty() {
            // TODO Check that the names and types match. Unassigned values are
            // permitted so we use the assignments as the set to iterate
            for name in named {
                match function_block.find_input(&name.name) {
                    Some(_) => {}
                    None => {
                        return SemanticDiagnostic::error("S0001", format!(
                            "Function call {} assigns input that is not defined {}",
                            function_block.name, name.name
                        ))
                    }
                }
            }
        }

        if !positional.is_empty() {
            todo!()
        }

        Ok(())
    }
}

impl Visitor<SemanticDiagnostic> for RuleFunctionBlockUse<'_> {
    type Value = ();

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, SemanticDiagnostic> {
        let res = visit_function_block_declaration(self, node);

        // Remove all items from var init decl since we have left this context
        self.var_to_fb.clear();
        res
    }

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, SemanticDiagnostic> {
        let res = visit_function_declaration(self, node);

        // Remove all items from var init decl since we have left this context
        self.var_to_fb.clear();
        res
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, SemanticDiagnostic> {
        let res = visit_program_declaration(self, node);

        // Remove all items from var init decl since we have left this context
        self.var_to_fb.clear();
        res
    }

    fn visit_var_init_decl(&mut self, node: &VarInitDecl) -> Result<Self::Value, SemanticDiagnostic> {
        match &node.initializer {
            TypeInitializer::Simple {
                type_name: _,
                initial_value: _,
            } => {}
            TypeInitializer::EnumeratedValues {
                values: _,
                default: _,
            } => {}
            TypeInitializer::EnumeratedType(_) => {}
            TypeInitializer::FunctionBlock(fbi) => {
                self.var_to_fb
                    .insert(node.name.clone(), fbi.type_name.clone());
            }
            TypeInitializer::Structure { type_name: _ } => {}
            TypeInitializer::LateResolvedType(_) => {
                panic!()
            }
        }
        Ok(Self::Value::default())
    }

    fn visit_fb_call(&mut self, fb_call: &FbCall) -> Result<Self::Value, SemanticDiagnostic> {
        // Check if function block is defined because you cannot
        // call a function block that doesn't exist
        println!("FB Invocation: {}", fb_call.var_name);

        let function_block_name = self.var_to_fb.get(&fb_call.var_name);
        match function_block_name {
            Some(function_block_name) => {
                let function_block_decl = self.function_blocks.get(function_block_name);
                match function_block_decl {
                    None => {
                        // Not defined, so this is not a valid use.
                        return SemanticDiagnostic::error("S0001", format!(
                            "Function block {} is not declared",
                            function_block_name
                        ));
                    }
                    Some(fb) => {
                        // Validate the parameter assignments
                        return RuleFunctionBlockUse::check_inputs(fb, &fb_call.params);
                    }
                }
            }
            None => {
                return SemanticDiagnostic::error("S0001", format!(
                    "Function block invocation {} do not refer to a variable in scope",
                    fb_call.var_name
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stages::parse;

    #[test]
    fn apply_when_no_names_uses_default_then_return_ok() {
        let program = "
FUNCTION_BLOCK Callee

END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE();
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_ok())
    }

    #[test]
    fn apply_when_some_names_assigned_then_ok() {
        let program = "
FUNCTION_BLOCK Callee
VAR_INPUT
IN1: BOOL;
IN2: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(IN1 := TRUE);
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_ok())
    }

    #[test]
    fn apply_when_all_names_assigned_then_ok() {
        let program = "
FUNCTION_BLOCK Callee
VAR_INPUT
IN1: BOOL;
IN2: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(IN1 := TRUE, IN2 := FALSE);
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_ok())
    }

    #[test]
    fn apply_when_names_incorrect_then_error() {
        let program = "
FUNCTION_BLOCK Callee
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(BAR := TRUE);
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_err())
    }

    #[test]
    fn apply_when_one_name_incorrect_then_error() {
        let program = "
FUNCTION_BLOCK Callee
VAR_INPUT
IN1: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(IN1 := TRUE, BAR := TRUE);
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert_eq!(true, result.is_err())
    }
}
