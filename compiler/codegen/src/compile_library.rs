//! Compatibility-library binding support for code generation.
//!
//! Codegen consumes the bindings side-table (`ironplc_dsl::bindings`,
//! produced by the `sources` loader and threaded in via
//! [`CodegenOptions::library_bindings`](crate::CodegenOptions)) at two
//! points:
//!
//! 1. **Body compilation** — a bound library POU's `;` body is never
//!    compiled ([`is_bound_library_function`]); the `FileId` check preserves
//!    user shadowing, so a user-defined POU with the same name still
//!    compiles as the user's function (`REQ-CL-analyzer-004`).
//! 2. **Call lowering** — after the `user_functions` check in
//!    `compile_function_call`, a call to an intrinsic-bound POU compiles its
//!    arguments at the declared parameter types and emits `BUILTIN func_id`
//!    per ADR-0008 (`REQ-CL-codegen-001`); a call to a declare-only POU is
//!    the dedicated compile error P4046, never silently-wrong codegen and
//!    never a runtime trap (`REQ-CL-codegen-002`).

use std::collections::HashMap;

use ironplc_container::opcode;
use ironplc_dsl::bindings::{LibraryBindings, PouBinding};
use ironplc_dsl::common::{
    FunctionDeclaration, InitialValueAssignmentKind, Library, LibraryElementKind, VariableType,
};
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, Function, ParamAssignmentKind};
use ironplc_problems::Problem;

use crate::compile::{CompileContext, OpType, DEFAULT_OP_TYPE};
use crate::compile_expr::compile_expr;
use crate::compile_setup::resolve_type_name;
use crate::emit::Emitter;

/// A binding resolved against the merged library's declarations, ready for
/// call lowering.
#[derive(Clone)]
pub(crate) enum ResolvedBinding {
    /// Calls lower to `BUILTIN func_id`, compiling each argument at the
    /// declared parameter's op type.
    Intrinsic {
        func_id: u16,
        param_op_types: Vec<OpType>,
    },
    /// Calls fail compilation with P4046, naming the declaring library.
    DeclareOnly { library: String },
}

/// Bindings pre-resolved for codegen, keyed by uppercased POU name.
///
/// `Default` is empty: consumers that never thread bindings (benchmarks,
/// direct `compile()` callers, the playground before its threading phase)
/// see no bound names, so an intrinsic-bound call falls through to the
/// generic-builtin path and fails closed rather than lowering wrongly.
#[derive(Clone, Default)]
pub(crate) struct ResolvedBindings {
    map: HashMap<String, ResolvedBinding>,
}

impl ResolvedBindings {
    /// Looks up the resolved binding for a POU name (case-insensitive).
    pub(crate) fn get(&self, pou_name: &str) -> Option<&ResolvedBinding> {
        self.map.get(&pou_name.to_uppercase())
    }
}

/// True when this function declaration is a bound library POU whose body must
/// not be compiled: its name carries a binding *and* it was declared in a
/// library source file. A user-defined function that shadows a bound name
/// fails the `FileId` check and compiles normally.
pub(crate) fn is_bound_library_function(
    decl: &FunctionDeclaration,
    bindings: &LibraryBindings,
) -> bool {
    bindings.get(decl.name.original()).is_some()
        && bindings.is_library_file(&decl.name.span.file_id)
}

/// Resolves every binding that has a matching library declaration.
///
/// Intrinsic names resolve through the single name→func_id table beside the
/// func_id constants (`opcode::builtin::intrinsic_func_id`). A name that does
/// not resolve is a packaging error in the library — bundled manifests are
/// guarded by a conformance test, and this is the defensive backstop — and
/// fails with P6010 anchored on the library's manifest file. Parameter op
/// types come from the library's own `.st` declaration, which is what makes
/// argument compilation match the signature `check` validated against.
pub(crate) fn resolve_bindings(
    library: &Library,
    bindings: &LibraryBindings,
) -> Result<ResolvedBindings, Diagnostic> {
    let mut map = HashMap::new();
    for element in &library.elements {
        let LibraryElementKind::FunctionDeclaration(decl) = element else {
            continue;
        };
        if !is_bound_library_function(decl, bindings) {
            continue;
        }
        // Present by the `is_bound_library_function` check above.
        let bound = bindings.get(decl.name.original()).unwrap();
        let resolved = match &bound.binding {
            PouBinding::Intrinsic { name } => {
                let func_id = opcode::builtin::intrinsic_func_id(name).ok_or_else(|| {
                    Diagnostic::problem(
                        Problem::LibraryManifestInvalid,
                        Label::file(
                            bound.manifest_file.clone(),
                            format!(
                                "library `{}` binds `{}` to unknown intrinsic `{name}`",
                                bound.library,
                                decl.name.original()
                            ),
                        ),
                    )
                })?;
                ResolvedBinding::Intrinsic {
                    func_id,
                    param_op_types: input_param_op_types(decl),
                }
            }
            PouBinding::DeclareOnly => ResolvedBinding::DeclareOnly {
                library: bound.library.clone(),
            },
        };
        map.insert(decl.name.original().to_uppercase(), resolved);
    }
    Ok(ResolvedBindings { map })
}

/// The op type of each `VAR_INPUT` parameter, in declaration order.
fn input_param_op_types(decl: &FunctionDeclaration) -> Vec<OpType> {
    decl.variables
        .iter()
        .filter(|v| v.var_type == VariableType::Input)
        .map(|v| match &v.initializer {
            InitialValueAssignmentKind::Simple(simple) => resolve_type_name(&simple.type_name.name)
                .map(|info| (info.op_width, info.signedness))
                .unwrap_or(DEFAULT_OP_TYPE),
            _ => DEFAULT_OP_TYPE,
        })
        .collect()
}

/// Compiles a call to a bound library POU, if the name is bound.
///
/// Returns `None` when the name carries no binding, so the caller can fall
/// through to the remaining call-lowering paths.
pub(crate) fn compile_bound_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
) -> Option<Result<(), Diagnostic>> {
    let resolved = ctx.library_bindings.get(func.name.original())?.clone();
    Some(match resolved {
        ResolvedBinding::Intrinsic {
            func_id,
            param_op_types,
        } => compile_intrinsic_call(emitter, ctx, func, func_id, &param_op_types),
        ResolvedBinding::DeclareOnly { library } => Err(Diagnostic::problem(
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
        )),
    })
}

/// Lowers an intrinsic-bound call: each argument compiles at the declared
/// parameter's op type, then `BUILTIN func_id` (`REQ-CL-codegen-001`).
fn compile_intrinsic_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    func_id: u16,
    param_op_types: &[OpType],
) -> Result<(), Diagnostic> {
    // Named arguments are already rewritten to positional by the analyzer
    // (xform_named_to_positional_args), matching the user-function path.
    let args: Vec<&Expr> = func
        .param_assignment
        .iter()
        .filter_map(|p| match p {
            ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
            _ => None,
        })
        .collect();

    // The analyzer type-checked the call against the library's declaration,
    // which is the same declaration the parameter types came from.
    if args.len() != param_op_types.len() {
        return Err(Diagnostic::todo_with_span(
            func.name.span(),
            file!(),
            line!(),
        ));
    }

    for (arg, param_op_type) in args.iter().zip(param_op_types) {
        compile_expr(emitter, ctx, arg, *param_op_type)?;
    }
    emitter.emit_builtin(func_id);
    Ok(())
}
