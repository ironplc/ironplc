//! Semantic rule that a method call (`instance.MethodName(args)`, the
//! CODESYS/TwinCAT OOP extension) refers to a method that is actually
//! declared -- either directly on the instance's function block type, or
//! on a function block reached by following that type's `EXTENDS` chain.
//!
//! This is the static-dispatch resolution algorithm from ADR-0041 Phase 1:
//! walk the static type's own methods first, then its base, then the
//! base's base, and so on.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Base
//!    METHOD Start
//!    END_METHOD
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
//! END_FUNCTION_BLOCK
//!
//! PROGRAM main
//!    VAR
//!       inst : FB_Derived;
//!    END_VAR
//!    inst.Start();
//! END_PROGRAM
//! ```
//!
//! ## Fails (Method Not Declared Anywhere In The Chain)
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Base
//! END_FUNCTION_BLOCK
//!
//! PROGRAM main
//!    VAR
//!       inst : FB_Base;
//!    END_VAR
//!    inst.Start();
//! END_PROGRAM
//! ```
use std::collections::{HashMap, HashSet};

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{result::SemanticResult, semantic_context::SemanticContext};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let mut function_blocks = HashMap::new();
    for x in lib.elements.iter() {
        if let LibraryElementKind::FunctionBlockDeclaration(fb) = x {
            function_blocks.insert(fb.name.clone(), fb);
        }
    }

    let mut visitor = RuleMethodCallDeclared::new(&function_blocks);
    visitor.walk(lib).map_err(|e| vec![e])
}

struct RuleMethodCallDeclared<'a> {
    // Map of the name of a function block declaration to the
    // declaration itself.
    function_blocks: &'a HashMap<TypeName, &'a FunctionBlockDeclaration>,

    // Map of variable name to the function block name that is the
    // declared type of that variable.
    var_to_fb: HashMap<Id, TypeName>,
}

impl<'a> RuleMethodCallDeclared<'a> {
    fn new(decls: &'a HashMap<TypeName, &'a FunctionBlockDeclaration>) -> Self {
        Self {
            function_blocks: decls,
            var_to_fb: HashMap::new(),
        }
    }

    /// Resolves `method_name` against `fb_name`'s own methods, then its
    /// `EXTENDS` base, then that base's base, and so on (ADR-0041 Phase 1
    /// static dispatch). Returns the function block that actually declares
    /// the method (which may be a base, not `fb_name` itself) together
    /// with the method declaration.
    fn resolve_method(
        &self,
        fb_name: &TypeName,
        method_name: &Id,
    ) -> Option<(&'a FunctionBlockDeclaration, &'a MethodDeclaration)> {
        let mut current = self.function_blocks.get(fb_name).copied();
        let mut visited: HashSet<TypeName> = HashSet::new();

        while let Some(fb) = current {
            // Guards against an EXTENDS cycle causing an infinite loop.
            // Cycles are also independently invalid (and expected to be
            // rejected elsewhere); this is just a safety net for this
            // rule specifically.
            if !visited.insert(fb.name.clone()) {
                return None;
            }

            if let Some(method) = fb.methods.iter().find(|m| &m.name == method_name) {
                return Some((fb, method));
            }

            current = fb
                .oop
                .as_ref()
                .and_then(|oop| oop.base.as_ref())
                .and_then(|base| self.function_blocks.get(base).copied());
        }

        None
    }

    fn check_assignments(
        owner_label: &str,
        method: &MethodDeclaration,
        call: &MethodCall,
    ) -> Result<(), Diagnostic> {
        crate::call_assignment_check::check_assignments(
            method,
            method.span(),
            call.span(),
            &call.params,
            &crate::call_assignment_check::AssignmentCheckLabels {
                call_label: "Method invocation",
                context_key: "method",
                owner_name: owner_label,
                decl_label: "Method declaration",
            },
        )
    }
}

impl Visitor<Diagnostic> for RuleMethodCallDeclared<'_> {
    type Value = ();

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let res = node.recurse_visit(self);
        self.var_to_fb.clear();
        res
    }

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let res = node.recurse_visit(self);
        self.var_to_fb.clear();
        res
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let res = node.recurse_visit(self);
        self.var_to_fb.clear();
        res
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        if let InitialValueAssignmentKind::FunctionBlock(fbi) = &node.initializer {
            if let Some(id) = node.identifier.symbolic_id() {
                self.var_to_fb.insert(id.clone(), fbi.type_name.clone());
            }
        }
        Ok(())
    }

    fn visit_method_call(&mut self, call: &MethodCall) -> Result<Self::Value, Diagnostic> {
        let fb_type = match self.var_to_fb.get(&call.instance) {
            Some(t) => t,
            None => {
                return Err(Diagnostic::problem(
                    Problem::FunctionBlockNotInScope,
                    Label::span(call.span(), "Method invocation"),
                )
                .with_context_id("invocation", &call.instance))
            }
        };

        if !self.function_blocks.contains_key(fb_type) {
            return Err(Diagnostic::problem(
                Problem::FunctionBlockNotInScope,
                Label::span(call.span(), "Method invocation"),
            )
            .with_context_id("invocation", &call.instance));
        }

        match self.resolve_method(fb_type, &call.method) {
            None => Err(Diagnostic::problem(
                Problem::MethodNotFound,
                Label::span(call.span(), "Method invocation"),
            )
            .with_context_type("function block", fb_type)
            .with_context_id("method", &call.method)),
            Some((owning_fb, method)) => {
                let owner_label = format!("{}.{}", owning_fb.name, method.name);
                Self::check_assignments(&owner_label, method, call)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts_with_fb_inheritance() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    rule_ok_with!(
        apply_when_method_declared_on_own_type_then_ok,
        opts_with_fb_inheritance(),
        "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
METHOD Start
    bRunning := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
m.Start();
END_PROGRAM"
    );

    rule_ok_with!(
        apply_when_method_declared_on_base_via_extends_then_ok,
        opts_with_fb_inheritance(),
        "
FUNCTION_BLOCK FB_Base
METHOD Start
    ;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Derived;
END_VAR
m.Start();
END_PROGRAM"
    );

    rule_ok_with!(
        apply_when_method_declared_two_levels_up_extends_chain_then_ok,
        opts_with_fb_inheritance(),
        "
FUNCTION_BLOCK FB_Base
METHOD Start
    ;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Mid EXTENDS FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Mid
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Derived;
END_VAR
m.Start();
END_PROGRAM"
    );

    rule_err1_with!(
        apply_when_method_not_declared_anywhere_then_error,
        opts_with_fb_inheritance(),
        "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
m.Start();
END_PROGRAM",
        Problem::MethodNotFound
    );

    rule_err1_with!(
        apply_when_method_call_has_wrong_arg_count_then_error,
        opts_with_fb_inheritance(),
        "
FUNCTION_BLOCK FB_Motor
METHOD SetSpeed
VAR_INPUT
    rSpeed : REAL;
END_VAR
    ;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
m.SetSpeed(1.0, 2.0);
END_PROGRAM",
        Problem::FunctionInvocationRequiresFormal
    );
}
