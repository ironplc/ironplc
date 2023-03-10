//! Semantic rule that reference to a function block must be to a function
//! block that is declared.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK Callee
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK Caller
//!    VAR
//!       FB_INSTANCE : Callee;
//!    END_VAR
//!    FB_INSTANCE();
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails (Incorrect Parameters)
//!
//! ```ignore
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
//! ```
use ironplc_dsl::{
    common::*,
    core::Id,
    textual::*,
    visitor::{
        visit_function_block_declaration, visit_function_declaration, visit_program_declaration,
        Visitor,
    },
};
use std::collections::HashMap;

use crate::error::SemanticDiagnostic;

/// Returns the first variable matching the specified name and one of the
/// variable types or `None` if the owner does not contain a matching
/// variable.
fn find<'a>(
    owner: &'a dyn HasVariables,
    name: &'a Id,
    types: &[VariableType],
) -> Option<&'a VarDecl> {
    owner
        .variables()
        .iter()
        .find(|item| item.name.eq(name) && types.contains(&item.var_type))
}

fn count_input_type(owner: &dyn HasVariables) -> usize {
    owner
        .variables()
        .iter()
        .filter(|item| item.var_type == VariableType::Input)
        .count()
}

/// Returns the first VAR_INPUT or VAR_INOUT variable matching the name
/// or `None` if the owner does not contain a matching variable.
fn find_input_type<'a>(owner: &'a dyn HasVariables, name: &'a Id) -> Option<&'a VarDecl> {
    find(owner, name, &[VariableType::Input, VariableType::InOut])
}

/// Returns the first VAR_OUTPUT variable matching the name
/// or `None` if the owner does not contain a matching variable.
///
/// VAR_IN_OUT are output variables, but they are only assigned
/// through the input `:=` syntax so not included for this rule.
fn find_output_type<'a>(owner: &'a dyn HasVariables, name: &'a Id) -> Option<&'a VarDecl> {
    find(owner, name, &[VariableType::Output])
}

pub fn apply(lib: &Library) -> Result<(), SemanticDiagnostic> {
    // Collect the names from the library into a map so that
    // we can quickly look up invocations
    let mut function_blocks = HashMap::new();
    for x in lib.elements.iter() {
        if let LibraryElement::FunctionBlockDeclaration(fb) = x {
            function_blocks.insert(fb.name.clone(), fb);
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

    fn check_assignments(
        function_block: &FunctionBlockDeclaration,
        params: &[ParamAssignment],
    ) -> Result<(), SemanticDiagnostic> {
        // Sort the inputs as either named, positional, and outputs
        let mut formal: Vec<&NamedInput> = vec![];
        let mut non_formal: Vec<&PositionalInput> = vec![];
        let mut outputs: Vec<&Output> = vec![];
        for param in params.iter() {
            match param {
                ParamAssignment::NamedInput(n) => {
                    formal.push(n);
                }
                ParamAssignment::PositionalInput(p) => {
                    non_formal.push(p);
                }
                // Don't care outputs here
                ParamAssignment::Output(o) => {
                    outputs.push(o);
                }
            }
        }

        // Don't allow a mixture so assert that either named is empty or
        // positional is empty
        if !formal.is_empty() && !non_formal.is_empty() {
            return Err(SemanticDiagnostic::error(
                "S0001",
                format!(
                    "Function call {} mixes named (formal) and positional (non-format) input arguments",
                    function_block.name
                ),
            ));
        }

        // Check that the names and types match. Unassigned values are
        // permitted so we use the assignments as the set to iterate
        if !formal.is_empty() {
            // TODO check the types.
            for name in formal {
                match find_input_type(function_block, &name.name) {
                    Some(_) => {}
                    None => {
                        return Err(SemanticDiagnostic::error(
                            "S0001",
                            format!(
                                "Function invocation {} assigns named (formal) input that is not defined {}",
                                function_block.name, name.name
                            ),
                        ))
                    }
                }
            }
        }

        // Check that the number of variables matches exactly the number
        // of expected inputs and the types match.
        if !non_formal.is_empty() {
            let num_required_inputs = count_input_type(function_block);
            if non_formal.len() != num_required_inputs {
                return Err(SemanticDiagnostic::error(
                    "S0001",
                    format!(
                        "Function invocation {} requires {} non-formal inputs but the invocation has {} formal inputs",
                        function_block.name, num_required_inputs, non_formal.len()
                    ),
                ));
            }
        }

        // Check that the assigned output parameter names match the actual
        // output parameter names
        for output in outputs {
            match find_output_type(function_block, &output.src) {
                Some(_) => {},
                None => {
                    return Err(SemanticDiagnostic::error(
                        "S0001",
                        format!(
                            "Function invocation {} assigns from {} (to {:?}) but {} is not an output variable of the function",
                            function_block.name, output.src, output.tgt, output.src
                        ),
                    ))
                }
            }
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

    fn visit_variable_declaration(
        &mut self,
        node: &VarDecl,
    ) -> Result<Self::Value, SemanticDiagnostic> {
        if let InitialValueAssignmentKind::FunctionBlock(fbi) = &node.initializer {
            self.var_to_fb
                .insert(node.name.clone(), fbi.type_name.clone());
        }
        Ok(())
    }

    fn visit_fb_call(&mut self, fb_call: &FbCall) -> Result<Self::Value, SemanticDiagnostic> {
        // Check if function block is defined because you cannot
        // call a function block that doesn't exist
        let function_block_name = self.var_to_fb.get(&fb_call.var_name);
        match function_block_name {
            Some(function_block_name) => {
                let function_block_decl = self.function_blocks.get(function_block_name);
                match function_block_decl {
                    None => {
                        // Not defined, so this is not a valid use.
                        panic!("Invalid semantic analysis state")
                    }
                    Some(fb) => {
                        // Validate the parameter assignments
                        RuleFunctionBlockUse::check_assignments(fb, &fb_call.params)
                    }
                }
            }
            None => Err(SemanticDiagnostic::error(
                "S0001",
                format!(
                    "Function block invocation {} do not refer to a variable in scope",
                    fb_call.var_name
                ),
            )),
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

        assert!(result.is_ok())
    }

    #[test]
    fn apply_when_some_formal_input_names_assigned_then_ok() {
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

        assert!(result.is_ok())
    }

    #[test]
    fn apply_when_mixed_formal_nonformal_then_error() {
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
FB_INSTANCE(IN1 := TRUE, FALSE);
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_err())
    }

    #[test]
    fn apply_when_function_block_definition_not_defined_then_error() {
        let program = "
FUNCTION_BLOCK Caller
VAR
IN1: BOOL;
END_VAR
FB_INSTANCE(IN1 := TRUE);
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_err())
    }

    #[test]
    fn apply_when_nonformal_input_names_assigned_then_ok() {
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
FB_INSTANCE(TRUE, FALSE);
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_ok())
    }

    #[test]
    fn apply_when_some_output_names_assigned_then_ok() {
        let program = "
FUNCTION_BLOCK Callee
VAR_OUTPUT
OUT1: BOOL;
OUT2: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
LOCAL: BOOL;
END_VAR
FB_INSTANCE(OUT1 => LOCAL);
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_ok())
    }

    #[test]
    fn apply_when_all_formal_input_names_assigned_then_ok() {
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

        assert!(result.is_ok())
    }

    #[test]
    fn apply_when_formal_names_incorrect_then_error() {
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

        assert!(result.is_err())
    }

    #[test]
    fn apply_when_nonformal_names_too_few_then_error() {
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
FB_INSTANCE(TRUE);
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_err())
    }

    #[test]
    fn apply_when_nonformal_names_too_many_then_error() {
        let program = "
FUNCTION_BLOCK Callee
VAR_INPUT
IN2: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(TRUE, FALSE);
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_err())
    }

    #[test]
    fn apply_when_one_input_name_incorrect_then_error() {
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

        assert!(result.is_err())
    }

    #[test]
    fn apply_when_one_output_name_incorrect_then_error() {
        let program = "
FUNCTION_BLOCK Callee
VAR_OUTPUT
OUT1: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
LOCAL: BOOL;
END_VAR
FB_INSTANCE(OUT2 => LOCAL);
END_FUNCTION_BLOCK";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_err())
    }

    #[test]
    fn apply_when_program_invokes_function_block_then_ok() {
        let program = "
FUNCTION_BLOCK Callee
VAR_INPUT
IN1: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
PROGRAM prgm
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(IN1 := TRUE);
END_PROGRAM";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_ok())
    }
}
