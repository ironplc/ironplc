//! `init`: the statements that run once, before the first round, and give
//! every global and every field of every program instance the value it
//! starts with (`specs/design/lowered-program.md`, "Initial values").
//!
//! The analyzer completes each declaration's initializer
//! ([`crate::initial_value`]): every field and element is listed, and every
//! scalar is a literal of its type. `init` stores that value through the
//! place that holds it, in declaration order: a scalar with `Assign`, a
//! string with `AssignStr`, and an array, a structure or a function block
//! instance element by element and field by field (REQ-LOW-lowering-194).
//! An instance's own initializer, `t : MyFb := (x := 7)`, is part of the
//! completed value, so it needs nothing more.
//!
//! `init` stores every value, cleared or not: which stores a backend can
//! leave out, because its storage starts cleared, is the backend's to say.
//!
//! The globals come first, in the execution model's order, then the fields
//! of each program instance, as codegen's `init` visits them. A program's
//! `VAR_TEMP` starts again at the head of each scan, and a compiler-provided
//! global has no initializer, so neither is in `init`.

use std::collections::HashMap;

use ironplc_dsl::common::{VarDecl, VariableType};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{SymbolicVariableKind, Variable};
use ironplc_dsl::type_id::TypeId as AstTypeId;
use ironplc_ir::execution::{ExecutionModel, InstanceOf};
use ironplc_ir::lowered::{
    ExecutionBindings, Expr, ExprKind, Owner, Place, Projection, ScalarType, Signedness, Stmt,
    StrExpr, TypeKind, VarId,
};

use crate::initial_value::{self, BlockMembers, FieldValue, InitialValue, ReferenceValue, Scalar};
use crate::variables::BodyFrame;
use crate::{Body, Lowering};

/// What lowering made of `init`: the statements of each value it lowered,
/// in order, and where each declaration is whose value it does not express
/// yet.
#[derive(Clone, Debug, Default)]
pub(crate) struct Init {
    pub(crate) statements: Vec<Stmt>,
    pub(crate) unsupported: Vec<SourceSpan>,
}

/// What reading a completed initializer needs: the representation of every
/// type, and the fields of every function block.
struct Values {
    types: HashMap<AstTypeId, ironplc_analyzer::SemanticType>,
    blocks: BlockMembers,
}

/// A store lowering made, `None` when it cannot express it yet, or a P9998.
type Made = Result<Option<()>, Diagnostic>;

impl<'a> Lowering<'a> {
    /// The statements of `init` for the globals and program instances of
    /// `execution`, which `bindings` binds to the program's variables.
    pub(crate) fn lower_init(
        &mut self,
        execution: &ExecutionModel,
        bindings: &ExecutionBindings,
    ) -> Result<Init, Diagnostic> {
        let types = self.analysis.context().types();
        let values = Values {
            types: types
                .iter_ids()
                .map(|(id, attributes)| (id, attributes.representation.clone()))
                .collect(),
            blocks: initial_value::block_members(types),
        };
        let mut init = Init::default();
        for (id, _) in execution.globals() {
            let (Some(var), Some(decl)) =
                (bindings.global(id), self.analysis.global_declaration(id))
            else {
                continue;
            };
            let place = self.program.var(var, &decl.identifier.span())?;
            self.store_declaration(None, place, decl, &values, &mut init)?;
        }
        for (position, instance) in execution.instances().enumerate() {
            let InstanceOf::Program(id) = &instance.program else {
                continue;
            };
            let (Some(var), Some(program)) = (
                bindings.instance(position),
                self.analysis.program_declaration(*id),
            ) else {
                continue;
            };
            let pou = self.body_pou(Body::Program(program));
            let Some(Some(frame)) = self.bodies.get(&pou).cloned() else {
                continue;
            };
            for decl in &program.variables {
                self.store_program_declaration(&frame, var, decl, &values, &mut init)?;
            }
        }
        Ok(init)
    }

    /// Adds the stores of the value `decl`, a declaration of the program
    /// whose body is `frame`, starts with to `init`: at its field of the
    /// program instance `instance` for a field, and at the global for a
    /// global the program declares in its interface.
    fn store_program_declaration(
        &mut self,
        frame: &BodyFrame,
        instance: VarId,
        decl: &VarDecl,
        values: &Values,
        init: &mut Init,
    ) -> Result<(), Diagnostic> {
        let Some(name) = decl.identifier.symbolic_id() else {
            return Ok(());
        };
        let span = name.span();
        let place = match decl.var_type {
            VariableType::Var | VariableType::Input | VariableType::Output => {
                let this = self.program.var(instance, &span)?;
                self.field_named(this, name, &span)?
            }
            VariableType::Global => self.named_place(frame, name, &span)?,
            // A `VAR_IN_OUT` is bound by a call, a `VAR_TEMP` starts again
            // in each scan, a `VAR_EXTERNAL` is its global's and an access
            // path names another variable.
            VariableType::InOut
            | VariableType::VarTemp
            | VariableType::External
            | VariableType::Access => return Ok(()),
        };
        match place {
            Some(place) => {
                self.store_declaration(Some((frame, instance)), place, decl, values, init)
            }
            None => {
                init.unsupported.push(span);
                Ok(())
            }
        }
    }

    /// Adds the stores of the value `decl` starts with, at `place`, to
    /// `init`, or `decl`'s span to what it does not express yet. A
    /// reference to a variable the declaration names is to one of the body
    /// of `frame` for a field of a program instance, and to a global
    /// otherwise.
    fn store_declaration(
        &mut self,
        frame: Option<(&BodyFrame, VarId)>,
        place: Place,
        decl: &VarDecl,
        values: &Values,
        init: &mut Init,
    ) -> Result<(), Diagnostic> {
        let span = decl.identifier.span();
        let value = initial_value::read(decl, &values.types, &values.blocks)?;
        let mut statements = Vec::new();
        match self.store_value(frame, place, &value, &span, &mut statements)? {
            Some(()) => init.statements.extend(statements),
            None => init.unsupported.push(span),
        }
        Ok(())
    }

    /// Adds the stores of `value` at `place` to `statements`.
    fn store_value(
        &mut self,
        frame: Option<(&BodyFrame, VarId)>,
        place: Place,
        value: &InitialValue,
        span: &SourceSpan,
        statements: &mut Vec<Stmt>,
    ) -> Made {
        match value {
            InitialValue::Scalar(scalar) => {
                let Some(operated) = self.program.types().operation_type(place.ty()) else {
                    return Ok(None);
                };
                let Some(expr) = scalar_expr(scalar.value, operated, span)? else {
                    return Ok(None);
                };
                statements.push(self.assign_value(place, expr, span.clone())?);
            }
            InitialValue::String(string) => {
                let Some(TypeKind::String(shape)) = self.program.types().kind(place.ty()) else {
                    return Ok(None);
                };
                let literal = StrExpr::literal(string.chars.clone(), shape.encoding, span.clone())?;
                statements.push(self.program.assign_str(place, literal, span.clone())?);
            }
            InitialValue::Reference(reference) => {
                let value = match reference {
                    ReferenceValue::Null => Expr::null(span.clone()),
                    ReferenceValue::To(variable) => {
                        let Some(target) = self.reference_target(frame, variable)? else {
                            return Ok(None);
                        };
                        Expr::ref_to(target, span.clone())
                    }
                };
                statements.push(self.program.assign(place, value, span.clone())?);
            }
            InitialValue::Array(elements) => {
                let Some(TypeKind::Array { dimensions, .. }) =
                    self.program.types().kind(place.ty()).cloned()
                else {
                    return Ok(None);
                };
                let Some(elements) = subscripted(&dimensions, elements) else {
                    return Ok(None);
                };
                for (subscripts, element) in elements {
                    let subscripts = subscripts
                        .into_iter()
                        .map(|subscript| {
                            Expr::integer(
                                subscript.into(),
                                ScalarType::I32(Signedness::Signed),
                                span.clone(),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let at = self.program.index(place.clone(), subscripts, span)?;
                    if self
                        .store_value(frame, at, element, span, statements)?
                        .is_none()
                    {
                        return Ok(None);
                    }
                }
            }
            InitialValue::Structure(fields) => {
                for FieldValue { name, value } in fields {
                    let Some(field) = self.field_named(place.clone(), name, span)? else {
                        return Ok(None);
                    };
                    if self
                        .store_value(frame, field, value, span, statements)?
                        .is_none()
                    {
                        return Ok(None);
                    }
                }
            }
            // A member initializer that is an expression reads the places
            // its body names, through the body's instance parameter, which
            // `init` does not have; lowering does not rebase them yet.
            InitialValue::Expression(_) => return Ok(None),
        }
        Ok(Some(()))
    }

    /// The field `name` of the record at `place`, matched without regard to
    /// case as IEC identifiers are; `None` when it has none of that name.
    fn field_named(
        &self,
        place: Place,
        name: &Id,
        span: &SourceSpan,
    ) -> Result<Option<Place>, Diagnostic> {
        let wanted = name.to_string().to_lowercase();
        let Some(position) = self.program.types().fields(place.ty()).and_then(|fields| {
            fields
                .iter()
                .position(|field| field.name.to_string().to_lowercase() == wanted)
        }) else {
            return Ok(None);
        };
        let Some(field) = self.program.field_idx(place.ty(), position) else {
            return Ok(None);
        };
        self.program.field(place, field, span).map(Some)
    }

    /// The place `REF(variable)` refers to: a global, or, in the
    /// initializer of a field of the program instance `instance`, what the
    /// body of `frame` names, reached through `instance` rather than the
    /// body's instance parameter; `None` for any other place.
    fn reference_target(
        &mut self,
        frame: Option<(&BodyFrame, VarId)>,
        variable: &Variable,
    ) -> Result<Option<Place>, Diagnostic> {
        let Some((frame, instance)) = frame else {
            return match variable {
                Variable::Symbolic(SymbolicVariableKind::Named(named)) => {
                    self.global_place(&named.name, &named.name.span())
                }
                Variable::Symbolic(_) | Variable::Direct(_) => Ok(None),
            };
        };
        let Some(place) = self.variable_place(frame, variable)? else {
            return Ok(None);
        };
        let span = variable.span();
        let owner = self
            .program
            .variables()
            .get(place.root())
            .map(|var| var.owner());
        let this = frame.instance.as_ref().map(|instance| instance.this);
        if owner == Some(Owner::Global) {
            return Ok(Some(place));
        }
        let ([Projection::Deref, path @ ..], true) = (place.path(), this == Some(place.root()))
        else {
            return Ok(None);
        };
        let mut rebased = self.program.var(instance, &span)?;
        for projection in path {
            rebased = match projection {
                Projection::Field(field) => self.program.field(rebased, *field, &span)?,
                Projection::Deref => self.program.deref(rebased, &span)?,
                Projection::Index(subscripts) => {
                    // A subscript that reads a place would read it through
                    // the instance parameter too.
                    let constant = subscripts
                        .iter()
                        .all(|subscript| matches!(subscript.kind(), ExprKind::Const(_)));
                    if !constant {
                        return Ok(None);
                    }
                    self.program.index(rebased, subscripts.clone(), &span)?
                }
            };
        }
        Ok(Some(rebased))
    }
}

/// The constant `value` at `operated`, the operation type of the place it
/// is stored in; `None` when the type does not hold such a value.
fn scalar_expr(
    value: Scalar,
    operated: ScalarType,
    span: &SourceSpan,
) -> Result<Option<Expr>, Diagnostic> {
    let span = span.clone();
    Ok(match (value, operated) {
        (Scalar::Bool(value), ScalarType::Bool) => Some(Expr::boolean(value, span)),
        (Scalar::Integer(value), ScalarType::I32(_) | ScalarType::I64(_)) => {
            Some(Expr::integer(value, operated, span)?)
        }
        (Scalar::Real32(value), ScalarType::F32) => Some(Expr::real(value.into(), operated, span)?),
        (Scalar::Real64(value), ScalarType::F64) => Some(Expr::real(value, operated, span)?),
        _ => None,
    })
}

/// Each element of an array whose dimensions are `dimensions`, with its
/// subscripts; `None` when `elements` are not one for each. The elements are
/// in storage order, the last subscript varying fastest, as
/// [`InitialValue::Array`] lists them; an element of an array of arrays is
/// itself an array, whose own elements its value lists.
fn subscripted<'v>(
    dimensions: &[(i32, i32)],
    elements: &'v [InitialValue],
) -> Option<Vec<(Vec<i32>, &'v InitialValue)>> {
    let mut subscripts: Vec<Vec<i32>> = vec![Vec::new()];
    for (lower, upper) in dimensions {
        subscripts = subscripts
            .into_iter()
            .flat_map(|outer| {
                (*lower..=*upper).map(move |subscript| {
                    let mut inner = outer.clone();
                    inner.push(subscript);
                    inner
                })
            })
            .collect();
    }
    (subscripts.len() == elements.len()).then(|| subscripts.into_iter().zip(elements).collect())
}
