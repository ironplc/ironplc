//! Post-resolution transform giving TwinCAT `REFERENCE TO` variables their
//! auto-dereferencing semantics.
//!
//! IEC 61131-3 `REF_TO` (and CODESYS `POINTER TO`) require an explicit `^` to
//! read or write the referenced value. The genuine IEC 61131-3:2013 / Beckhoff
//! TwinCAT `REFERENCE TO` type instead *auto-dereferences*: a bare use of the
//! variable reads through the reference, and a bare `:=` assignment writes
//! through it. This transform rewrites those bare uses into the explicit
//! dereference forms the backend already understands:
//!
//! * a bare read `y := r;` becomes `y := r^;` (wrap the read in `ExprKind::Deref`),
//! * a bare write `r := v;` becomes `r^ := v;` (set `Assignment::deref`).
//!
//! It also lowers `__ISVALIDREF(r)` to `r <> NULL` when `allow_reference_to` is
//! set; with the flag off the call is left alone (and later reported as an
//! undeclared function).
//!
//! The transform keys on each declaration's `RefSyntax` tag, so only variables
//! declared `REFERENCE TO` are affected -- `REF_TO`/`POINTER TO` variables keep
//! their explicit-`^` behavior even when both features are enabled. It is scoped
//! to directly-declared `VAR ... : REFERENCE TO ...;` variables (the only site
//! that carries the `ReferenceTo` tag), leaving array elements and
//! `TYPE`-alias-declared references on explicit `^`.
//!
//! Runs before symbol/function resolution (so `__ISVALIDREF` is lowered before
//! it would be flagged as an undeclared function) and before the reference
//! semantic rules (`rule_ref_to`), so arithmetic/ordering on an
//! auto-dereferenced value is checked against the dereferenced value, not the
//! reference.
//!
//! See `specs/design/reference-to-twincat.md` (PR 2).

use ironplc_dsl::common::*;
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;
use ironplc_parser::options::CompilerOptions;
use std::collections::HashSet;

/// The TwinCAT reference-validity builtin, recognized only when
/// `allow_reference_to` is set.
const IS_VALID_REF: &str = "__ISVALIDREF";

pub fn apply(lib: Library, options: &CompilerOptions) -> Result<Library, Vec<Diagnostic>> {
    let mut resolver = ImplicitDeref {
        reference_to_vars: HashSet::new(),
        allow_reference_to: options.allow_reference_to,
        suppress_wrap: false,
    };
    resolver.fold_library(lib).map_err(|e| vec![e])
}

struct ImplicitDeref {
    /// Names of variables in the current POU scope declared `REFERENCE TO`.
    reference_to_vars: HashSet<Id>,
    allow_reference_to: bool,
    /// When set, the next folded `ExprKind::Variable` is left un-wrapped -- used
    /// so an explicit `r^` is not turned into `r^^`.
    suppress_wrap: bool,
}

impl ImplicitDeref {
    fn collect_reference_to_vars(&mut self, variables: &[VarDecl]) {
        for var in variables {
            if let (VariableIdentifier::Symbol(id), InitialValueAssignmentKind::Reference(init)) =
                (&var.identifier, &var.initializer)
            {
                if init.syntax == RefSyntax::ReferenceTo {
                    self.reference_to_vars.insert(id.clone());
                }
            }
        }
    }

    /// True if `var` is a bare named variable declared `REFERENCE TO` in scope.
    fn is_auto_deref_var(&self, var: &Variable) -> bool {
        matches!(
            var,
            Variable::Symbolic(SymbolicVariableKind::Named(named))
                if self.reference_to_vars.contains(&named.name)
        )
    }

    /// If `func` is a recognized `__ISVALIDREF(r)` call (and the feature is
    /// enabled), returns its single argument expression.
    fn isvalidref_arg(&self, func: &Function) -> Option<Expr> {
        if !self.allow_reference_to {
            return None;
        }
        if !func.name.to_string().eq_ignore_ascii_case(IS_VALID_REF) {
            return None;
        }
        if func.param_assignment.len() != 1 {
            return None;
        }
        match &func.param_assignment[0] {
            ParamAssignmentKind::PositionalInput(pos) => Some(pos.expr.clone()),
            _ => None,
        }
    }
}

impl Fold<Diagnostic> for ImplicitDeref {
    fn fold_function_declaration(
        &mut self,
        node: FunctionDeclaration,
    ) -> Result<FunctionDeclaration, Diagnostic> {
        self.collect_reference_to_vars(&node.variables);
        let result = node.recurse_fold(self);
        self.reference_to_vars.clear();
        result
    }

    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> Result<FunctionBlockDeclaration, Diagnostic> {
        self.collect_reference_to_vars(&node.variables);
        let result = node.recurse_fold(self);
        self.reference_to_vars.clear();
        result
    }

    fn fold_program_declaration(
        &mut self,
        node: ProgramDeclaration,
    ) -> Result<ProgramDeclaration, Diagnostic> {
        self.collect_reference_to_vars(&node.variables);
        let result = node.recurse_fold(self);
        self.reference_to_vars.clear();
        result
    }

    fn fold_assignment(&mut self, node: Assignment) -> Result<Assignment, Diagnostic> {
        // A bare `:=` write to a REFERENCE TO variable stores *through* the
        // reference. Skip `REF=` bindings (which rebind the reference itself)
        // and skip when the value is itself a reference-binding form (`REF(x)`
        // or `NULL`), which set/clear the reference rather than write through
        // it. Determined on the original value kind (folding never changes a
        // `Ref`/`Null` into something else).
        let target_is_auto_deref = self.is_auto_deref_var(&node.target);
        let value_binds = matches!(&node.value.kind, ExprKind::Ref(_) | ExprKind::Null(_));

        let mut folded = node.recurse_fold(self)?;

        if target_is_auto_deref && !folded.ref_bind && !value_binds {
            folded.deref = true;
        }
        Ok(folded)
    }

    fn fold_expr_kind(&mut self, node: ExprKind) -> Result<ExprKind, Diagnostic> {
        match node {
            // Lower `__ISVALIDREF(r)` to `r <> NULL` -- a comparison of the
            // reference *value* (not its dereference) against the null
            // sentinel. The argument is used verbatim so it is not wrapped.
            ExprKind::Function(func) => {
                if let Some(arg) = self.isvalidref_arg(&func) {
                    let span = arg.span();
                    return Ok(ExprKind::Compare(Box::new(CompareExpr {
                        op: CompareOp::Ne,
                        left: arg,
                        right: Expr::new(ExprKind::Null(span)),
                    })));
                }
                Ok(ExprKind::Function(func.recurse_fold(self)?))
            }
            // An explicit `r^`: fold the inner variable's own children but do
            // not auto-wrap the variable itself (that would produce `r^^`).
            ExprKind::Deref(inner) => {
                if matches!(inner.kind, ExprKind::Variable(_)) {
                    self.suppress_wrap = true;
                    let folded = self.fold_expr(*inner)?;
                    self.suppress_wrap = false;
                    Ok(ExprKind::Deref(Box::new(folded)))
                } else {
                    Ok(ExprKind::Deref(Box::new(self.fold_expr(*inner)?)))
                }
            }
            // A bare read of a REFERENCE TO variable reads *through* it.
            ExprKind::Variable(var) => {
                let suppress = self.suppress_wrap;
                self.suppress_wrap = false;
                let folded = var.recurse_fold(self)?;
                if !suppress && self.is_auto_deref_var(&folded) {
                    Ok(ExprKind::Deref(Box::new(Expr::new(ExprKind::Variable(
                        folded,
                    )))))
                } else {
                    Ok(ExprKind::Variable(folded))
                }
            }
            other => other.recurse_fold(self),
        }
    }
}
