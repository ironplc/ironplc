//! Starting values: `init`, the statements that run once, before the first
//! round, and give every global and every field of every program instance
//! the value it starts with, and each body's prologue, which gives each of
//! its variables that starts again on every call its value
//! (`specs/design/lowered-program.md`, "Initial values").
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
//!
//! A function's or method's prologue stores the value of each `VAR`,
//! `VAR_TEMP` and `VAR_OUTPUT` the analyzer flags as starting again on every
//! call, in declaration order, then of its result, as codegen's prologue
//! does (REQ-LOW-lowering-195). A program's or function block's prologue
//! stores the value of each of its `VAR_TEMP`, which the analyzer flags the
//! same way, in declaration order (REQ-LOW-lowering-201).

use std::collections::HashMap;

use ironplc_dsl::common::{VarDecl, VariableType};
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{SymbolicVariableKind, Variable};
use ironplc_dsl::type_id::TypeId as AstTypeId;
use ironplc_ir::execution::{ExecutionModel, InstanceOf};
use ironplc_ir::lowered::{
    ExecutionBindings, Expr, Owner, Place, ScalarType, Signedness, Stmt, StrExpr, TypeKind, VarId,
};

use crate::execution_model::Declarations;
use crate::initial_value::{self, BlockMembers, FieldValue, InitialValue, ReferenceValue, Scalar};
use crate::variables::BodyFrame;
use crate::{Body, Lowered, Lowering, Unlowered};

/// What lowering made of the starting values of `init` or of a prologue:
/// the statements of each value it lowered, in order, and a P9999 at each
/// declaration whose value it does not express yet.
#[derive(Clone, Debug, Default)]
pub(crate) struct Stores {
    pub(crate) statements: Vec<Stmt>,
    pub(crate) unsupported: Vec<Diagnostic>,
}

impl Stores {
    /// The statements, when every value lowered.
    pub(crate) fn complete(&self) -> Option<&[Stmt]> {
        self.unsupported
            .is_empty()
            .then_some(self.statements.as_slice())
    }
}

/// What reading a completed initializer needs: the representation of every
/// type, and the fields of every function block.
pub(crate) struct Values {
    types: HashMap<AstTypeId, ironplc_analyzer::SemanticType>,
    blocks: BlockMembers,
}

impl Values {
    pub(crate) fn of(types: &ironplc_analyzer::TypeEnvironment) -> Self {
        Self {
            types: types
                .iter_ids()
                .map(|(id, attributes)| (id, attributes.representation.clone()))
                .collect(),
            blocks: initial_value::block_members(types),
        }
    }
}

/// Where the places a starting value names are reached from.
#[derive(Clone, Copy)]
enum Reach<'f> {
    /// A global's initializer, which names other globals.
    Global,
    /// The initializer of a field of a program instance, in `init`, which
    /// runs in no body: the frame of the program's body held by the
    /// variable that holds the instance ([`BodyFrame::held_by`]), so that
    /// the program's variables are reached through the instance itself.
    Init(&'f BodyFrame<'f>),
    /// The initializer of a variable of the body of the frame, which runs
    /// in that body.
    Body(&'f BodyFrame<'f>),
}

impl<'a> Lowering<'a> {
    /// The statements of `init` for the globals and program instances of
    /// `execution`, whose ids name `declarations` and which `bindings` binds
    /// to the program's variables.
    pub(crate) fn lower_init(
        &mut self,
        execution: &ExecutionModel,
        declarations: &Declarations<'a>,
        bindings: &ExecutionBindings,
        values: &Values,
    ) -> Result<Stores, Diagnostic> {
        let mut init = Stores::default();
        for (id, _) in execution.globals() {
            let (Some(var), Some(decl)) = (bindings.global(id), declarations.global(id)) else {
                continue;
            };
            let place = self.program.var(var, &decl.identifier.span())?;
            self.store_declaration(Reach::Global, place, decl, values, &mut init)?;
        }
        for (position, instance) in execution.instances().enumerate() {
            let InstanceOf::Program(id) = &instance.program else {
                continue;
            };
            let (Some(var), Some(program)) =
                (bindings.instance(position), declarations.program(*id))
            else {
                continue;
            };
            let pou = self.body_pou(Body::Program(program));
            let Some(Ok(frame)) = self.bodies.get(&pou).cloned() else {
                continue;
            };
            for decl in &program.variables {
                self.store_program_declaration(&frame, var, decl, values, &mut init)?;
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
        init: &mut Stores,
    ) -> Result<(), Diagnostic> {
        let Some(name) = decl.identifier.symbolic_id() else {
            return Ok(());
        };
        let span = name.span();
        match decl.var_type {
            VariableType::Var
            | VariableType::Input
            | VariableType::Output
            | VariableType::Global => {}
            // A `VAR_IN_OUT` is bound by a call, a `VAR_TEMP` starts again
            // in each scan, a `VAR_EXTERNAL` is its global's and an access
            // path names another variable.
            VariableType::InOut
            | VariableType::VarTemp
            | VariableType::External
            | VariableType::Access => return Ok(()),
        }
        let held = frame.held_by(self.program.var(instance, &span)?);
        match self.named_place(&held, name, &span) {
            Ok(place) => self.store_declaration(Reach::Init(&held), place, decl, values, init),
            Err(unlowered) => {
                init.unsupported.push(unlowered.not_yet_at(span)?);
                Ok(())
            }
        }
    }

    /// The prologue of `body`, whose frame is `frame`: the store of the
    /// value each of its declarations that starts again on every call starts
    /// with, in declaration order, then a function's or method's result's
    /// (REQ-LOW-lowering-195, REQ-LOW-lowering-201).
    pub(crate) fn lower_prologue(
        &mut self,
        body: Body<'a>,
        frame: &BodyFrame,
        values: &Values,
    ) -> Result<Stores, Diagnostic> {
        let mut prologue = Stores::default();
        let (variables, result) = match body {
            Body::Function(function) => (&function.variables, function.result.variable()),
            Body::Method { method, .. } => (&method.variables, method.result.variable()),
            Body::Program(program) => (&program.variables, None),
            Body::FunctionBlock(block) => (&block.variables, None),
        };
        let reset = variables.iter().filter(|decl| decl.reset_on_call);
        for decl in reset.chain(result) {
            let Some(name) = decl.identifier.symbolic_id() else {
                continue;
            };
            match self.named_place(frame, name, &name.span()) {
                Ok(place) => {
                    self.store_declaration(Reach::Body(frame), place, decl, values, &mut prologue)?
                }
                Err(unlowered) => prologue
                    .unsupported
                    .push(unlowered.not_yet_at(name.span())?),
            }
        }
        Ok(prologue)
    }

    /// Adds the stores of the value `decl` starts with, at `place`, to
    /// `stores`, or the P9999 at `decl` when it does not express the value
    /// yet. A variable the value names is reached as `reach` says.
    fn store_declaration(
        &mut self,
        reach: Reach<'_>,
        place: Place,
        decl: &VarDecl,
        values: &Values,
        stores: &mut Stores,
    ) -> Result<(), Diagnostic> {
        let span = decl.identifier.span();
        let value = initial_value::read(decl, &values.types, &values.blocks)?;
        let mut statements = Vec::new();
        match self.store_value(reach, place, &value, &span, &mut statements) {
            Ok(()) => stores.statements.extend(statements),
            Err(unlowered) => stores.unsupported.push(unlowered.not_yet_at(span)?),
        }
        Ok(())
    }

    /// Adds the stores of `value` at `place` to `statements`.
    fn store_value(
        &mut self,
        reach: Reach<'_>,
        place: Place,
        value: &InitialValue,
        span: &SourceSpan,
        statements: &mut Vec<Stmt>,
    ) -> Lowered<()> {
        match value {
            InitialValue::Scalar(scalar) => {
                let operated = self
                    .program
                    .types()
                    .operation_type(place.ty())
                    .ok_or(Unlowered::NotYet)?;
                let expr = scalar_expr(scalar.value, operated, span)?;
                statements.push(self.assign_value(place, expr, span.clone())?);
            }
            InitialValue::String(string) => {
                let Some(TypeKind::String(shape)) = self.program.types().kind(place.ty()) else {
                    return Err(Unlowered::NotYet);
                };
                let literal = StrExpr::literal(string.chars.clone(), shape.encoding, span.clone())?;
                statements.push(self.program.assign_str(place, literal, span.clone())?);
            }
            InitialValue::Reference(reference) => {
                let value = match reference {
                    ReferenceValue::Null => Expr::null(span.clone()),
                    ReferenceValue::To(variable) => {
                        let target = self.reference_target(reach, variable)?;
                        Expr::ref_to(target, span.clone())
                    }
                };
                statements.push(self.program.assign(place, value, span.clone())?);
            }
            InitialValue::Array(elements) => {
                let Some(TypeKind::Array { dimensions, .. }) =
                    self.program.types().kind(place.ty()).cloned()
                else {
                    return Err(Unlowered::NotYet);
                };
                let elements = subscripted(&dimensions, elements).ok_or(Unlowered::NotYet)?;
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
                    self.store_value(reach, at, element, span, statements)?;
                }
            }
            InitialValue::Structure(fields) => {
                for FieldValue { name, value } in fields {
                    let field = self.field_of(place.clone(), name, span)?;
                    self.store_value(reach, field, value, span, statements)?;
                }
            }
            // A member initializer that is an expression is lowered in the
            // body it runs in. One of a field of a program instance, in
            // `init`, reads the program's variables through the instance
            // itself, as `init` has no instance parameter. One of a global
            // is not lowered yet.
            InitialValue::Expression(expr) => {
                let frame = match reach {
                    Reach::Body(frame) | Reach::Init(frame) => frame,
                    Reach::Global => return Err(Unlowered::NotYet),
                };
                statements.extend(self.store_at(frame, place, expr, span.clone())?);
            }
        }
        Ok(())
    }

    /// The place `REF(variable)` refers to, reached as `reach` says: a
    /// global, what a body names, or, in `init`, what the program's body
    /// names, reached through the instance itself. `init` runs in no body,
    /// so a variable a body owns is no place it reaches.
    fn reference_target(&mut self, reach: Reach<'_>, variable: &Variable) -> Lowered<Place> {
        match reach {
            Reach::Global => match variable {
                Variable::Symbolic(SymbolicVariableKind::Named(named)) => {
                    self.global_place(&named.name, &named.name.span())
                }
                Variable::Symbolic(_) | Variable::Direct(_) => Err(Unlowered::NotYet),
            },
            Reach::Body(frame) => self.variable_place(frame, variable),
            Reach::Init(frame) => {
                let place = self.variable_place(frame, variable)?;
                let owner = self
                    .program
                    .variables()
                    .get(place.root())
                    .map(|var| var.owner());
                if owner != Some(Owner::Global) {
                    return Err(Unlowered::NotYet);
                }
                Ok(place)
            }
        }
    }
}

/// The constant `value` at `operated`, the operation type of the place it
/// is stored in. A value the type does not hold is not lowered.
fn scalar_expr(value: Scalar, operated: ScalarType, span: &SourceSpan) -> Lowered<Expr> {
    let span = span.clone();
    Ok(match (value, operated) {
        (Scalar::Bool(value), ScalarType::Bool) => Expr::boolean(value, span),
        (Scalar::Integer(value), ScalarType::I32(_) | ScalarType::I64(_)) => {
            Expr::integer(value, operated, span)?
        }
        (Scalar::Real32(value), ScalarType::F32) => Expr::real(value.into(), operated, span)?,
        (Scalar::Real64(value), ScalarType::F64) => Expr::real(value, operated, span)?,
        _ => return Err(Unlowered::NotYet),
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
