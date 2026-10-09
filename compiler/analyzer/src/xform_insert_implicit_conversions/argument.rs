//! Recording the conversion of an argument to the type of its parameter.
//!
//! A call to a user-defined function passes each input by value at the
//! operation width of its parameter, and an argument of another width is
//! converted to it: `f(d)` with a `LINT` parameter widens the `DINT`. This
//! module records that conversion on the argument, as an
//! [`ExprKind::ImplicitConversion`].
//!
//! A parameter is passed at its declared type, whatever kind of declaration
//! declares it: a subrange at its base type and an alias at the type it
//! names, so the `DINT` of `f(d)` with a parameter of an alias of `LREAL` is
//! converted to `LREAL`.
//!
//! An untyped literal takes the type of its parameter, as it takes the type
//! of an assignment's target, so the parameter receives the value the program
//! wrote: the `0.1` of `f(0.1)` with an `LREAL` parameter is an `LREAL`. The
//! code generator used to compile it at its default type (`REAL`) and convert
//! it, which rounded `0.1` to a `REAL` and made `1.0E300` infinite, and
//! failed on `f(5000000000)` with an `LINT` parameter.
//!
//! A standard function compiles its arguments as its operation requires and
//! is not recorded here, and neither is a `VAR_IN_OUT` or `REF_TO`
//! parameter, which is passed by reference, or a string, which is copied.
//!
//! An input of a function block call is stored in a field of the block, and
//! an argument of a method call in a parameter of the method. Each is
//! converted to the declared type of its field or parameter, as an assigned
//! value is to its target's (see `assignment.rs`).

use ironplc_dsl::common::TypeName;
use ironplc_dsl::textual::{Expr, ExprType, FbCall, Function, MethodCall, ParamAssignmentKind};
use ironplc_dsl::type_id::TypeId;

use super::declared::elementary_of;
use super::ImplicitConversions;
use crate::intermediates::conversion_target::wrap;
use crate::intermediates::numeric_operation::{
    literal_default_type, operation_width_of, OperationWidth,
};
use crate::semantic_type::SemanticFunctionParameter;
use crate::type_environment::elementary_type;

impl ImplicitConversions<'_> {
    /// Records the conversion of each argument of `func`, a call to a
    /// user-defined function, to the type its parameter is passed as.
    pub(super) fn record_argument_conversions(&self, func: &mut Function) {
        let Some(signature) = self.context.functions().get(&func.name) else {
            return;
        };
        if signature.intrinsic.is_some() {
            return;
        }
        // The named-argument pass made every input positional, in the order
        // of the input parameters.
        let params: Vec<SemanticFunctionParameter> = signature.input_parameters().collect();
        let args = func
            .param_assignment
            .iter_mut()
            .filter_map(|arg| match arg {
                ParamAssignmentKind::PositionalInput(input) => Some(&mut input.expr),
                ParamAssignmentKind::NamedInput(_) | ParamAssignmentKind::Output(_) => None,
            });
        for (arg, param) in args.zip(&params) {
            self.record_argument(arg, param);
        }
    }

    /// Records the conversion of each input of `node` to the type of the
    /// field it is stored in.
    pub(super) fn record_fb_call_inputs(&self, node: &mut FbCall) {
        for (input, at) in self.fb_call_inputs(node) {
            if let Some(at) = at {
                self.record_conversion_to(input, at);
            }
        }
    }

    /// Records the conversion of each argument of `call` to the type its
    /// parameter is passed as.
    pub(super) fn record_method_arguments(&self, call: &mut MethodCall) {
        for (arg, at) in self.method_arguments(call) {
            if let Some(at) = at {
                self.record_conversion_to(arg, at);
            }
        }
    }

    fn record_argument(&self, arg: &mut Expr, param: &SemanticFunctionParameter) {
        if param.is_inout || param.is_reference {
            return;
        }
        let Some((target, width)) = self.passed_as(&param.param_type) else {
            return;
        };
        if self.is_integer_result_in_other_context(arg, target) {
            self.settle_integer_result(arg, target);
            return;
        }
        match &arg.expr_type {
            Some(ExprType::Literal(generic)) => {
                let Some(default) = literal_default_type(generic) else {
                    return;
                };
                let default: TypeName = default.into();
                let Some((default_id, default_width)) = self.elementary(&default) else {
                    return;
                };
                if is_real(default_width) && !is_real(width) {
                    // A real literal for an integer parameter, which the
                    // argument type rule rejects: it is no value of the
                    // parameter's type, so it is converted to one.
                    arg.expr_type = Some(ExprType::Inferred(default_id));
                    wrap(arg, target);
                } else {
                    arg.expr_type = Some(ExprType::Inferred(target));
                }
            }
            Some(ExprType::Concrete(own) | ExprType::Inferred(own)) => {
                let own = self
                    .context
                    .types()
                    .get_by_id(*own)
                    .and_then(|attributes| operation_width_of(&attributes.representation));
                if own.is_some_and(|own| own != width) {
                    wrap(arg, target);
                }
            }
            Some(ExprType::Null) | None => {}
        }
    }

    /// The type a parameter declared as `param_type` is passed as, and its
    /// operation width: the elementary type it is operated as, its own, the
    /// base type of a subrange, or the type an alias names. `None` for a
    /// parameter that is not a single numeric value: a string, which is
    /// copied rather than passed, or an enumeration.
    fn passed_as(&self, param_type: &TypeName) -> Option<(TypeId, OperationWidth)> {
        let types = self.context.types();
        let representation = match elementary_type(param_type) {
            Some(representation) => representation,
            None => &types.get(param_type)?.representation,
        };
        let id = elementary_of(types, representation)?;
        let width = operation_width_of(&types.get_by_id(id)?.representation)?;
        Some((id, width))
    }

    /// The id of the elementary type `name`, and its operation width.
    fn elementary(&self, name: &TypeName) -> Option<(TypeId, OperationWidth)> {
        let width = operation_width_of(elementary_type(name)?)?;
        Some((self.context.types().id_of(name)?, width))
    }
}

/// Returns `true` for the width of a real type.
fn is_real(width: OperationWidth) -> bool {
    match width {
        OperationWidth::F32 | OperationWidth::F64 => true,
        OperationWidth::W32 | OperationWidth::W64 => false,
    }
}
