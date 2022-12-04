//! Each reference to a function block must be to a function
//! block that is declared.
use ironplc_dsl::{ast::*, dsl::*, visitor::Visitor};
use std::collections::HashMap;

pub fn apply(lib: Library) -> Result<(), String> {
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
    let mut visitor = RuleFunctionBlockUse::new();
    visitor.walk(&lib)
}

struct RuleFunctionBlockUse {
    function_blocks: HashMap<String, FunctionBlockDeclaration>,
}
impl RuleFunctionBlockUse {
    fn new() -> Self {
        RuleFunctionBlockUse {
            function_blocks: HashMap::new(),
        }
    }

    fn check_inputs(
        function_block: &FunctionBlockDeclaration,
        params: &Vec<ParamAssignment>,
    ) -> Result<(), String> {
        // Sort the inputs as either named or positional
        let mut named: Vec<&NamedInput> = vec![];
        let mut positional: Vec<&PositionalInput> = vec![];
        for param in params.iter() {
            match param {
                ParamAssignment::NamedInput(n) => {
                    named.push(n);
                },
                ParamAssignment::PositionalInput(p) => {
                    positional.push(p);

                },
                ParamAssignment::Output { not, src, tgt } => {}
            }
        }

        // Don't allow a mixture so assert that either named is empty or
        // positional is empty
        if named.len() > 0 && positional.len() > 0 {
            return Err(format!("Function call {} mixes named and positional input arguments", function_block.name));
        }

        if !named.is_empty() {
            // Check that the names match
        }

        if !positional.is_empty() {
            // Check that the number of arguments matches the function

        }

        Ok(())
    }
}

impl Visitor<String> for RuleFunctionBlockUse {
    type Value = ();

    fn visit_fb_call(&mut self, fb_call: &FbCall) -> Result<Self::Value, String> {
        // Check if function block is defined because you cannot
        // call a function block that doesn't exist
        let function_block_decl = self.function_blocks.get(&fb_call.name);
        match function_block_decl {
            None => {
                // Not defined, so this is not a valid use.
                return Err(format!("Function block {} is not declared", fb_call.name));
            }
            Some (fb) => {
                // Validate the parameter assignments
                return RuleFunctionBlockUse::check_inputs(fb, &fb_call.params)
            }
        }
    }
}
