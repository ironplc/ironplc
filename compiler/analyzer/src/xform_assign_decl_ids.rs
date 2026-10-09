//! Transformation pass that gives every variable declaration its identity.
//!
//! Each [`VarDecl`] and [`EdgeVarDecl`] gets a [`DeclId`] of its own, and so
//! does the implicit result variable of a function, and of a method that
//! declares a return type. The symbol environment records the same id for
//! the symbol, and `xform_bind_variables` records it on every reference, so
//! a back end keys storage by declaration and never resolves a name
//! (ADR-0058, `specs/design/variable-binding.md`).
//!
//! The ids are unique within one compilation and say nothing on their own.
use ironplc_dsl::common::*;
use ironplc_dsl::decl_id::DeclId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;

/// Hands out the [`DeclId`]s of one compilation, so every declaration --
/// those this pass visits and those the compiler provides -- gets an id no
/// other declaration has.
#[derive(Debug, Default)]
pub struct DeclIdAllocator {
    next: u32,
}

impl DeclIdAllocator {
    /// The next unused id.
    pub fn allocate(&mut self) -> DeclId {
        let id = DeclId::from_raw(self.next);
        self.next += 1;
        id
    }
}

/// Gives every declaration in `lib` a [`DeclId`] from `ids`. Cannot fail.
pub fn apply(lib: Library, ids: &mut DeclIdAllocator) -> Result<Library, Vec<Diagnostic>> {
    let mut assigner = DeclIdAssigner { ids };
    assigner.fold_library(lib).map_err(|e| vec![e])
}

struct DeclIdAssigner<'a> {
    ids: &'a mut DeclIdAllocator,
}

impl Fold<Diagnostic> for DeclIdAssigner<'_> {
    fn fold_var_decl(&mut self, node: VarDecl) -> Result<VarDecl, Diagnostic> {
        let decl_id = Some(self.ids.allocate());
        // The initializer can hold no declaration, so there is nothing below
        // this node to number.
        Ok(VarDecl { decl_id, ..node })
    }

    fn fold_edge_var_decl(&mut self, node: EdgeVarDecl) -> Result<EdgeVarDecl, Diagnostic> {
        let decl_id = Some(self.ids.allocate());
        Ok(EdgeVarDecl { decl_id, ..node })
    }

    fn fold_function_declaration(
        &mut self,
        node: FunctionDeclaration,
    ) -> Result<FunctionDeclaration, Diagnostic> {
        let result_decl_id = Some(self.ids.allocate());
        let node = node.recurse_fold(self)?;
        Ok(FunctionDeclaration {
            result_decl_id,
            ..node
        })
    }

    fn fold_method_declaration(
        &mut self,
        node: MethodDeclaration,
    ) -> Result<MethodDeclaration, Diagnostic> {
        // A method without a return type has no result variable.
        let result_decl_id = node.return_type.is_some().then(|| self.ids.allocate());
        let node = node.recurse_fold(self)?;
        Ok(MethodDeclaration {
            result_decl_id,
            ..node
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use ironplc_dsl::common::{EdgeVarDecl, FunctionDeclaration, MethodDeclaration, VarDecl};
    use ironplc_dsl::decl_id::DeclId;
    use ironplc_dsl::visitor::Visitor;
    use ironplc_parser::options::CompilerOptions;
    use spec_test_macro::spec_test;

    use crate::test_helpers::{fb_inheritance_options, parse_and_resolve_types_with_options};

    /// Every id the pass recorded, in traversal order: the declarations'
    /// and the result variables' together, and each method's result id.
    #[derive(Default)]
    struct Ids {
        ids: Vec<Option<DeclId>>,
        method_results: Vec<Option<DeclId>>,
    }

    impl Visitor<()> for Ids {
        type Value = ();

        fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), ()> {
            self.ids.push(node.decl_id);
            node.recurse_visit(self)
        }

        fn visit_edge_var_decl(&mut self, node: &EdgeVarDecl) -> Result<(), ()> {
            self.ids.push(node.decl_id);
            node.recurse_visit(self)
        }

        fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), ()> {
            self.ids.push(node.result_decl_id);
            node.recurse_visit(self)
        }

        fn visit_method_declaration(&mut self, node: &MethodDeclaration) -> Result<(), ()> {
            self.method_results.push(node.result_decl_id);
            node.recurse_visit(self)
        }
    }

    fn ids(program: &str, options: &CompilerOptions) -> Ids {
        let (library, _) = parse_and_resolve_types_with_options(program, options);
        let mut ids = Ids::default();
        let _ = ids.walk(&library);
        ids
    }

    #[spec_test(REQ_VB_analyzer_001)]
    #[test]
    fn apply_when_every_kind_of_declaration_then_each_has_a_distinct_id() {
        let program = "
VAR_GLOBAL top : INT; END_VAR
FUNCTION F : INT VAR_INPUT a : INT; END_VAR VAR b : INT; END_VAR F := a; END_FUNCTION
FUNCTION_BLOCK FB
VAR_INPUT i : BOOL; e : BOOL R_EDGE; END_VAR
VAR_OUTPUT o : INT; END_VAR
VAR v : INT; END_VAR
VAR_TEMP t : INT; END_VAR
METHOD M : INT VAR_INPUT p : INT; END_VAR M := p; END_METHOD
END_FUNCTION_BLOCK
PROGRAM main VAR x : INT; END_VAR VAR_EXTERNAL c : INT; END_VAR END_PROGRAM
CONFIGURATION config
  VAR_GLOBAL c : INT; END_VAR
  RESOURCE res ON PLC
    VAR_GLOBAL r : INT; END_VAR
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION";

        let options = CompilerOptions {
            allow_top_level_var_global: true,
            ..fb_inheritance_options()
        };
        let ids = ids(program, &options);

        let all: Vec<_> = ids.ids.iter().chain(&ids.method_results).collect();
        assert_eq!(all.len(), 15);
        assert!(all.iter().all(|id| id.is_some()));
        let distinct: HashSet<_> = all.iter().collect();
        assert_eq!(distinct.len(), all.len());
    }

    #[spec_test(REQ_VB_analyzer_002)]
    #[test]
    fn apply_when_method_has_no_return_type_then_no_result_id() {
        let program = "
FUNCTION F : INT F := 1; END_FUNCTION
FUNCTION_BLOCK FB
METHOD M END_METHOD
METHOD N : INT N := 1; END_METHOD
END_FUNCTION_BLOCK";

        let ids = ids(program, &fb_inheritance_options());

        assert_eq!(ids.ids.len(), 1);
        assert!(ids.ids[0].is_some());
        assert_eq!(ids.method_results.len(), 2);
        assert_eq!(ids.method_results[0], None);
        assert!(ids.method_results[1].is_some());
    }
}
