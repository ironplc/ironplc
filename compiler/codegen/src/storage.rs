//! Where each variable's value lives, keyed by its declaration.
//!
//! The analyzer gives every variable declaration a [`DeclId`] and records on
//! every reference the declaration the reference names (ADR-0058,
//! `specs/design/variable-binding.md`). Codegen allocates storage per
//! declaration, in the maps on [`CompileContext`], and looks a reference up
//! by the declaration it records. It never resolves a name, so a body needs
//! no copy of another body's names: a global is found from a function body
//! because the reference names the global's declaration, and a local that
//! hides the global is found because the reference names the local's.
//!
//! A body's storage stays in the maps only while the body (and, for a
//! function block, its methods) compiles; [`CompileContext::release`] takes
//! it out afterwards. A reference from elsewhere then finds no storage and is
//! reported as not implemented: the storage of a function block's fields is
//! its own type's frame, which a block that `EXTENDS` it cannot address yet.

use ironplc_container::VarIndex;
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::decl_id::DeclId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{NamedVariable, SymbolicVariableKind, Variable};

use crate::compile::{CompileContext, OpType, VarTypeInfo, DEFAULT_OP_TYPE};

/// A variable reference as the analyzer bound it: the declaration it names,
/// and the name it was written with, which diagnostics show.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Binding<'a> {
    pub(crate) decl: DeclId,
    pub(crate) name: &'a Id,
}

impl<'a> Binding<'a> {
    /// The binding the analyzer recorded for the reference `name`. A
    /// reference the analyzer did not bind is an internal error: analysis
    /// reports a name that declares no variable before codegen runs.
    pub(crate) fn new(decl: Option<DeclId>, name: &'a Id) -> Result<Self, Diagnostic> {
        decl.map(|decl| Binding { decl, name }).ok_or_else(|| {
            Diagnostic::internal_error_at(Label::span(
                name.span(),
                "Variable reference was not bound to a declaration before code generation",
            ))
            .with_context("variable", &name.to_string())
        })
    }

    /// The binding of the named variable `named`.
    pub(crate) fn of(named: &'a NamedVariable) -> Result<Self, Diagnostic> {
        Self::new(named.decl_id, &named.name)
    }

    /// The binding of `variable` when it is a bare named variable, `None`
    /// for any other shape (an element, a field, a dereference, ...).
    pub(crate) fn of_variable(variable: &'a Variable) -> Result<Option<Self>, Diagnostic> {
        match variable {
            Variable::Symbolic(SymbolicVariableKind::Named(named)) => Self::of(named).map(Some),
            _ => Ok(None),
        }
    }
}

impl CompileContext {
    /// Takes the next slot of the frame being laid out.
    ///
    /// The program-level frame (globals, then the program's own variables,
    /// with the scratch slots some aggregates need between them) starts at
    /// slot 0; a function, function block or method body moves the cursor to
    /// its own region while it lays itself out.
    pub(crate) fn allocate_slot(&mut self) -> VarIndex {
        let index = VarIndex::new(self.next_slot);
        self.next_slot += 1;
        index
    }

    /// Looks up the slot of the declaration `binding` names.
    ///
    /// A `VAR_IN_OUT` parameter's slot holds a reference, not the value, so
    /// loading or storing it directly would be wrong. Sites that handle one
    /// ask [`Self::in_out_ref_slot`] first; every other site reaches here
    /// and is refused.
    ///
    /// A declaration with no slot is not an undeclared variable: analysis
    /// reports that first (`rule_use_declared_symbolic_var`, P4007). It is one
    /// codegen does not yet give storage to, such as a field inherited through
    /// `EXTENDS` or a `RESOURCE`'s `VAR_GLOBAL`, so it is reported as not
    /// implemented rather than as a mistake in the program.
    pub(crate) fn var_index(&self, binding: Binding<'_>) -> Result<VarIndex, Diagnostic> {
        if self.in_out_params.contains(&binding.decl) {
            return Err(Diagnostic::not_implemented(Label::span(
                binding.name.span(),
                "VAR_IN_OUT parameter used where only a local or global variable is supported",
            )));
        }
        self.variables.get(&binding.decl).copied().ok_or_else(|| {
            Diagnostic::not_implemented(Label::span(
                binding.name.span(),
                "Variable that code generation does not yet give storage to",
            ))
            .with_context("variable", &binding.name.to_string())
        })
    }

    /// Returns the slot holding the reference when `decl` is a `VAR_IN_OUT`
    /// parameter, or `None` for any other variable.
    pub(crate) fn in_out_ref_slot(&self, decl: DeclId) -> Option<VarIndex> {
        if !self.in_out_params.contains(&decl) {
            return None;
        }
        self.variables.get(&decl).copied()
    }

    /// Looks up the type information of the declaration `decl`.
    pub(crate) fn var_type_info(&self, decl: DeclId) -> Option<VarTypeInfo> {
        self.var_types.get(&decl).copied()
    }

    /// Returns the op type of the declaration `decl`, falling back to the
    /// default.
    pub(crate) fn var_op_type(&self, decl: DeclId) -> OpType {
        self.var_types
            .get(&decl)
            .map(|info| (info.op_width, info.signedness))
            .unwrap_or(DEFAULT_OP_TYPE)
    }

    /// Takes a scratch slot from the frame being laid out, for an aggregate
    /// that needs one to address its `STRING` elements.
    pub(crate) fn allocate_scratch_variable(&mut self) -> VarIndex {
        self.allocate_slot()
    }

    /// Takes the storage of `decls` out of every map, once the body that
    /// declares them is compiled.
    pub(crate) fn release(&mut self, decls: impl IntoIterator<Item = DeclId>) {
        for decl in decls {
            self.variables.remove(&decl);
            self.var_types.remove(&decl);
            self.string_vars.remove(&decl);
            self.fb_instances.remove(&decl);
            self.array_vars.remove(&decl);
            self.struct_vars.remove(&decl);
            self.struct_array_vars.remove(&decl);
            self.in_out_params.remove(&decl);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spec_test_macro::spec_test;

    fn binding(name: &Id, raw: u32) -> Binding<'_> {
        Binding {
            decl: DeclId::from_raw(raw),
            name,
        }
    }

    #[spec_test(REQ_VB_codegen_030)]
    #[test]
    fn binding_new_when_unbound_then_internal_error() {
        let name = Id::from("x");

        let result = Binding::new(None, &name);

        assert!(result.is_err_and(|d| d.code == "P9998"));
    }

    #[test]
    fn allocate_slot_when_called_twice_then_consecutive_slots() {
        let mut ctx = CompileContext::new();
        ctx.next_slot = 4;

        assert_eq!(ctx.allocate_slot(), VarIndex::new(4));
        assert_eq!(ctx.allocate_scratch_variable(), VarIndex::new(5));
        assert_eq!(ctx.next_slot, 6);
    }

    #[spec_test(REQ_VB_codegen_031)]
    #[test]
    fn var_index_when_two_declarations_share_a_name_then_each_has_its_own_slot() {
        let name = Id::from("x");
        let mut ctx = CompileContext::new();
        ctx.variables.insert(DeclId::from_raw(1), VarIndex::new(0));
        ctx.variables.insert(DeclId::from_raw(2), VarIndex::new(7));

        assert_eq!(ctx.var_index(binding(&name, 1)).unwrap(), VarIndex::new(0));
        assert_eq!(ctx.var_index(binding(&name, 2)).unwrap(), VarIndex::new(7));
    }

    #[spec_test(REQ_VB_codegen_031)]
    #[test]
    fn var_index_when_in_out_parameter_then_not_implemented() {
        let name = Id::from("io");
        let mut ctx = CompileContext::new();
        ctx.variables.insert(DeclId::from_raw(1), VarIndex::new(3));
        ctx.in_out_params.insert(DeclId::from_raw(1));

        assert!(ctx
            .var_index(binding(&name, 1))
            .is_err_and(|d| d.code == "P9999"));
        assert_eq!(
            ctx.in_out_ref_slot(DeclId::from_raw(1)),
            Some(VarIndex::new(3))
        );
    }

    #[test]
    fn release_when_body_compiled_then_its_storage_is_gone() {
        let name = Id::from("x");
        let mut ctx = CompileContext::new();
        ctx.variables.insert(DeclId::from_raw(1), VarIndex::new(0));
        ctx.variables.insert(DeclId::from_raw(2), VarIndex::new(1));

        ctx.release([DeclId::from_raw(2)]);

        assert!(ctx.var_index(binding(&name, 1)).is_ok());
        assert!(ctx
            .var_index(binding(&name, 2))
            .is_err_and(|d| d.code == "P9999"));
    }

    #[spec_test(REQ_VB_codegen_040)]
    #[test]
    fn compile_expr_when_late_bound_then_internal_error() {
        use ironplc_dsl::textual::{Expr, ExprKind};

        let mut ctx = CompileContext::new();
        let mut emitter = crate::emit::Emitter::new();
        let expr = Expr::new(ExprKind::late_bound("x"));

        let result =
            crate::compile_expr::compile_expr(&mut emitter, &mut ctx, &expr, DEFAULT_OP_TYPE);

        assert!(result.is_err_and(|diagnostic| diagnostic.code == "P9998"));
    }
}
