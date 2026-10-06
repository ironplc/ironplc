//! Recording the conversion of an argument to the type of its parameter.
//!
//! A call to a user-defined function passes each input by value at the
//! operation width of its parameter, and an argument of another width is
//! converted to it: `f(d)` with a `LINT` parameter widens the `DINT`. This
//! module records that conversion on the argument, as an
//! [`ExprKind::ImplicitConversion`].
//!
//! It records exactly what the code generator did, including two choices a
//! later change may correct:
//!
//! * a parameter whose type is not elementary (an alias, a subrange, an
//!   enumeration) is passed as a `DINT`, the default slot type, so an argument
//!   of another width is converted to `DINT`;
//! * an untyped literal is operated at its default type (`DINT` or `REAL`)
//!   and then converted, so the `1.5` of `f(1.5)` with an `LREAL` parameter is
//!   a `REAL` converted to `LREAL`.
//!
//! A standard function compiles its arguments as its operation requires and
//! is not recorded here, and neither is a `VAR_IN_OUT` or `REF_TO`
//! parameter, which is passed by reference, or a string, which is copied.

use ironplc_dsl::common::{ElementaryTypeName, TypeName};
use ironplc_dsl::textual::{Expr, ExprType, Function, ParamAssignmentKind};
use ironplc_dsl::type_id::TypeId;

use super::ImplicitConversions;
use crate::intermediates::conversion_target::wrap;
use crate::intermediates::numeric_operation::{
    literal_default_type, operation_width_of, OperationWidth,
};
use crate::semantic_type::{SemanticFunctionParameter, SemanticType};
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

    fn record_argument(&self, arg: &mut Expr, param: &SemanticFunctionParameter) {
        if param.is_inout || param.is_reference {
            return;
        }
        let Some((target, width)) = self.passed_as(&param.param_type) else {
            return;
        };
        match &arg.expr_type {
            Some(ExprType::Literal(generic)) => {
                let Some(default) = literal_default_type(generic) else {
                    return;
                };
                let default: TypeName = default.into();
                let Some((default_id, default_width)) = self.elementary(&default) else {
                    return;
                };
                if default_width == width {
                    arg.expr_type = Some(ExprType::Inferred(target));
                } else {
                    arg.expr_type = Some(ExprType::Inferred(default_id));
                    wrap(arg, target);
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
    /// operation width: its own type when it is elementary, and `DINT`, the
    /// default slot type, otherwise. `None` for a string, which is copied
    /// rather than passed.
    fn passed_as(&self, param_type: &TypeName) -> Option<(TypeId, OperationWidth)> {
        let is_string =
            |representation: &SemanticType| matches!(representation, SemanticType::String { .. });
        if self
            .context
            .types()
            .get(param_type)
            .is_some_and(|attributes| is_string(&attributes.representation))
        {
            return None;
        }
        match elementary_type(param_type) {
            Some(representation) if is_string(representation) => None,
            Some(_) => self.elementary(param_type),
            None => self.elementary(&ElementaryTypeName::DINT.into()),
        }
    }

    /// The id of the elementary type `name`, and its operation width.
    fn elementary(&self, name: &TypeName) -> Option<(TypeId, OperationWidth)> {
        let width = operation_width_of(elementary_type(name)?)?;
        Some((self.context.types().id_of(name)?, width))
    }
}
