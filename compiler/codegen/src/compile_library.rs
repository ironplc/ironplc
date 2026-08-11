//! Compatibility-library declare-only support for code generation.
//!
//! Codegen consumes the declare-only side-table (`ironplc_dsl::bindings`,
//! produced by the `sources` loader and threaded in via
//! [`CodegenOptions::library_bindings`](crate::CodegenOptions)) at two
//! points:
//!
//! 1. **Body compilation** — a declare-only library POU's `;` body is never
//!    compiled ([`is_bound_library_function`]), so its name is not
//!    registered as a callable user function and a call site reaches the
//!    check below. The `FileId` check preserves user shadowing: a
//!    user-defined POU with the same name still compiles as the user's
//!    function.
//! 2. **Call lowering** — after the `user_functions` check in
//!    `compile_function_call`, a call to a declare-only POU is the
//!    dedicated compile error P4046, naming the library and POU — never
//!    silently-wrong codegen and never a runtime trap.
//!
//! Bindings deliberately cannot select an implementation (see
//! `ironplc_dsl::bindings`): native behavior is exposed as typed
//! `__`-prefixed compiler intrinsics that library ST bodies call, so every
//! `BUILTIN` emission originates from compiler-owned tables.

use ironplc_dsl::bindings::LibraryBindings;
use ironplc_dsl::common::FunctionDeclaration;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::Function;
use ironplc_problems::Problem;

use crate::compile::CompileContext;

/// True when this function declaration is a declare-only library POU whose
/// body must not be compiled: its name carries a binding *and* it was
/// declared in a library source file. A user-defined function that shadows
/// the name fails the `FileId` check and compiles normally.
pub(crate) fn is_bound_library_function(
    decl: &FunctionDeclaration,
    bindings: &LibraryBindings,
) -> bool {
    bindings.get_declare_only(decl.name.original()).is_some()
        && bindings.is_library_file(&decl.name.span.file_id)
}

/// Rejects a call to a declare-only library POU with P4046, if the name is
/// declare-only.
///
/// Returns `None` when the name carries no binding, so the caller can fall
/// through to the remaining call-lowering paths.
pub(crate) fn compile_bound_call(
    ctx: &CompileContext,
    func: &Function,
) -> Option<Result<(), Diagnostic>> {
    let library = ctx
        .library_bindings
        .get_declare_only(func.name.original())?;
    Some(Err(Diagnostic::problem(
        Problem::LibraryFunctionNotImplemented,
        Label::span(
            func.name.span(),
            format!(
                "`{}` is declared by compatibility library `{library}` as \
                 declare-only: its implementation has not been built, so a \
                 call to it cannot be compiled",
                func.name.original()
            ),
        ),
    )))
}
