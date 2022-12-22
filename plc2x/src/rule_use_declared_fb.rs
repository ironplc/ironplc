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
//! END_FUNCTION_BLOCK
//!
//! ## Fails
//!
//! FUNCTION_BLOCK Caller
//!    VAR
//!       FB_INSTANCE : UndeclaredFunctionBlock;
//!    END_VAR
//! END_FUNCTION_BLOCK
//!
//! ## Todo
//!
//! I'm not certain this rule is quite right.
use ironplc_dsl::{ast::*, dsl::*, visitor::Visitor};
use std::collections::HashMap;

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

pub fn apply(lib: &Library) -> Result<(), String> {
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

    for (key, _) in &function_blocks {
        println!("{}", key);
    }

    // Walk the library to find all references to function blocks
    let mut visitor = RuleFunctionBlockUse::new(&function_blocks);
    visitor.walk(lib)
}

struct RuleFunctionBlockUse<'a> {
    function_blocks: &'a HashMap<Id, &'a FunctionBlockDeclaration>,
}
impl<'a> RuleFunctionBlockUse<'a> {
    fn new(decls: &'a HashMap<Id, &'a FunctionBlockDeclaration>) -> Self {
        RuleFunctionBlockUse {
            function_blocks: decls,
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
            return Err(format!(
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
                        return Err(format!(
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

impl Visitor<String> for RuleFunctionBlockUse<'_> {
    type Value = ();

    fn visit_fb_call(&mut self, fb_call: &FbCall) -> Result<Self::Value, String> {
        // Check if function block is defined because you cannot
        // call a function block that doesn't exist
        println!("{}", fb_call.name);

        for (key, _) in self.function_blocks {
            println!("{}", key);
        }

        let function_block_decl = self.function_blocks.get(&fb_call.name);
        match function_block_decl {
            None => {
                // Not defined, so this is not a valid use.
                return Err(format!("Function block {} is not declared", fb_call.name));
            }
            Some(fb) => {
                // Validate the parameter assignments
                return RuleFunctionBlockUse::check_inputs(fb, &fb_call.params);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::ast::*;
    use ironplc_dsl::dsl::*;

    use super::*;

    fn make_fb_call(params: Vec<ParamAssignment>) -> Library {
        Library {
            elems: vec![
                LibraryElement::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("CALLEE"),
                    inputs: vec![
                        VarInitDecl::simple("IN1", "BOOL"),
                        VarInitDecl::simple("IN2", "BOOL"),
                    ],
                    outputs: vec![],
                    inouts: vec![],
                    vars: vec![],
                    externals: vec![],
                    body: FunctionBlockBody::stmts(vec![]),
                }),
                LibraryElement::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("CALLER"),
                    inputs: vec![],
                    outputs: vec![],
                    inouts: vec![],
                    vars: vec![],
                    externals: vec![],
                    body: FunctionBlockBody::stmts(vec![StmtKind::FbCall(FbCall {
                        name: Id::from("CALLEE"),
                        params: params,
                    })]),
                }),
            ],
        }
    }

    #[test]
    fn apply_when_no_names_uses_default_then_return_ok() {
        let input = make_fb_call(vec![]);

        let result = apply(&input);
        assert_eq!(true, result.is_ok());
    }

    #[test]
    fn apply_when_some_names_assigned_then_ok() {
        let input = make_fb_call(vec![ParamAssignment::named(
            "IN1",
            ExprKind::symbolic_variable("LOCAL1"),
        )]);

        let result = apply(&input);
        assert_eq!(true, result.is_ok());
    }

    #[test]
    fn apply_when_all_names_assigned_then_ok() {
        let input = make_fb_call(vec![
            ParamAssignment::named("IN1", ExprKind::symbolic_variable("LOCAL1")),
            ParamAssignment::named("IN2", ExprKind::symbolic_variable("LOCAL1")),
        ]);

        let result = apply(&input);
        assert_eq!(true, result.is_ok());
    }

    #[test]
    fn apply_when_names_incorrect_then_error() {
        let input = make_fb_call(vec![ParamAssignment::named(
            "BAR",
            ExprKind::symbolic_variable("LOCAL1"),
        )]);

        let result = apply(&input);
        assert_eq!(true, result.is_err());
    }
}
