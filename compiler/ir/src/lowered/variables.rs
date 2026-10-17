//! The variables and POUs of a lowered program.
//!
//! Each table allocates the ids of its entries, so an id always names an
//! entry, and a place rooted at a variable takes its type from the table
//! rather than from its caller.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::call::Param;
use super::ids::{PouId, Stamp, TypeId, VarId};
use super::invariant::broken;
use super::types::{Nullability, TypeKind, TypeTable};
use crate::execution::DebugName;

/// What a variable belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Global,
    Pou(PouId),
}

/// A variable of the lowered program that is not a field: a global, a
/// parameter, a local, a `VAR_TEMP`, or the instance parameter of a body.
#[derive(Clone, Debug)]
pub struct Variable {
    name: DebugName,
    owner: Owner,
    ty: TypeId,
}

impl Variable {
    /// The name as declared, and the span of its declaration, for debug
    /// information and diagnostics.
    pub fn name(&self) -> &DebugName {
        &self.name
    }

    pub fn owner(&self) -> Owner {
        self.owner
    }

    pub fn ty(&self) -> TypeId {
        self.ty
    }
}

/// Every variable of a lowered program, by [`VarId`]. A
/// [`ProgramBuilder`](super::ProgramBuilder) owns it, and is the only way to
/// add to it.
#[derive(Clone, Debug)]
pub struct VariableTable {
    stamp: Stamp,
    variables: Vec<Variable>,
}

impl VariableTable {
    pub(crate) fn new(stamp: Stamp) -> Self {
        Self {
            stamp,
            variables: Vec::new(),
        }
    }

    /// Adds a variable named `name`, owned by `owner`, of the type `ty`,
    /// which must be in `types` (REQ-LOW-lowering-026).
    pub(crate) fn push(
        &mut self,
        types: &TypeTable,
        name: DebugName,
        owner: Owner,
        ty: TypeId,
    ) -> Result<VarId, Diagnostic> {
        if types.get(ty).is_none() {
            return Err(broken(
                "VariableTable::push",
                &name.span(),
                format!("{ty} is not in the type table"),
            ));
        }
        let id = VarId::new(self.stamp, self.variables.len() as u32);
        self.variables.push(Variable { name, owner, ty });
        Ok(id)
    }

    /// The variable `id` names, or `None` if this table did not allocate
    /// `id`.
    pub fn get(&self, id: VarId) -> Option<&Variable> {
        if id.stamp() != self.stamp {
            return None;
        }
        self.variables.get(id.index() as usize)
    }

    /// Every variable, with its id, in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = (VarId, &Variable)> {
        self.variables
            .iter()
            .enumerate()
            .map(|(index, variable)| (VarId::new(self.stamp, index as u32), variable))
    }

    /// The type of the variable `id`, or a P9998 at `span` if this table did
    /// not allocate `id`.
    pub(crate) fn type_of(&self, id: VarId, span: &SourceSpan) -> Result<TypeId, Diagnostic> {
        self.get(id).map(Variable::ty).ok_or_else(|| {
            broken(
                "Place::var",
                span,
                format!("{id} is not in the variable table"),
            )
        })
    }
}

/// The parameters a function takes, in order, and the variable that holds
/// its result. A call passes one argument for each parameter
/// (REQ-LOW-lowering-071).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionSignature {
    parameters: Vec<VarId>,
    result: Option<VarId>,
    /// The result parameter, the first of `parameters`, of a function
    /// whose result is a string (REQ-LOW-lowering-079).
    result_parameter: Option<VarId>,
}

impl FunctionSignature {
    pub(crate) fn new(parameters: Vec<VarId>, result: Option<VarId>) -> Self {
        Self {
            parameters,
            result,
            result_parameter: None,
        }
    }

    /// The signature of a function that returns its result through its
    /// first parameter, `parameters[0]`.
    pub(crate) fn returning_through(parameters: Vec<VarId>, result_parameter: VarId) -> Self {
        Self {
            parameters,
            result: None,
            result_parameter: Some(result_parameter),
        }
    }

    /// The parameters, in order: a method's instance parameter first
    /// (REQ-LOW-lowering-076), and a function's result parameter first
    /// (REQ-LOW-lowering-079).
    pub fn parameters(&self) -> &[VarId] {
        &self.parameters
    }

    /// The variable that holds the result, the value of a call; `None` for
    /// a method without a result, or a function that returns its result
    /// through a parameter, whose call is only a statement.
    pub fn result(&self) -> Option<VarId> {
        self.result
    }

    /// The result parameter of a function whose result is a string: a
    /// non-null reference to the temporary the caller passes, through which
    /// the function writes its result (REQ-LOW-lowering-079). `None` for any
    /// other function or method.
    pub fn result_parameter(&self) -> Option<VarId> {
        self.result_parameter
    }

    /// What each parameter takes, in order: a value at its operation type, a
    /// string of its encoding, or, for a `VAR_IN_OUT` or a `VAR_OUTPUT`, a
    /// non-null reference, the place its argument passes
    /// (REQ-LOW-lowering-033, REQ-LOW-lowering-072, REQ-LOW-lowering-092).
    /// `None` when one is none of these.
    pub(crate) fn params(
        &self,
        types: &TypeTable,
        variables: &VariableTable,
    ) -> Option<Vec<Param>> {
        self.parameters
            .iter()
            .map(|var| {
                let ty = variables.get(*var)?.ty();
                match types.reference(ty) {
                    Some((target, Nullability::NonNull)) => Some(Param::Ref(target)),
                    Some((_, Nullability::Nullable)) | None => by_value(types, ty),
                }
            })
            .collect()
    }
}

/// What a parameter of type `ty` takes by value: a value at its operation
/// type, or a string of its encoding. `None` for an aggregate, which a
/// parameter does not take yet.
pub(crate) fn by_value(types: &TypeTable, ty: TypeId) -> Option<Param> {
    if let Some(TypeKind::String(shape)) = types.kind(ty) {
        return Some(Param::Str(shape.encoding));
    }
    types.operation_type(ty).map(Param::Value)
}

/// One POU: its name, its signature when it is a function or method a call
/// reaches, and, for a method, the body of the function block it belongs to.
#[derive(Clone, Debug)]
struct PouEntry {
    name: DebugName,
    function: Option<FunctionSignature>,
    method_of: Option<PouId>,
}

/// Every POU of a lowered program, by [`PouId`]. A
/// [`ProgramBuilder`](super::ProgramBuilder) owns it, and is the only way to
/// add to it.
#[derive(Clone, Debug)]
pub struct PouTable {
    stamp: Stamp,
    pous: Vec<PouEntry>,
}

impl PouTable {
    pub(crate) fn new(stamp: Stamp) -> Self {
        Self {
            stamp,
            pous: Vec::new(),
        }
    }

    /// Adds the POU named `name`, a method of the function block whose body
    /// `method_of` is, if any, and returns the id that names it.
    pub(crate) fn push(&mut self, name: DebugName, method_of: Option<PouId>) -> PouId {
        let id = PouId::new(self.stamp, self.pous.len() as u32);
        self.pous.push(PouEntry {
            name,
            function: None,
            method_of,
        });
        id
    }

    /// The name of the POU `id`, or `None` if this table did not allocate
    /// `id`.
    pub fn name(&self, id: PouId) -> Option<&DebugName> {
        self.entry(id).map(|entry| &entry.name)
    }

    /// The signature of the function or method `id`, or `None` if it is not
    /// one this table declares.
    pub fn function(&self, id: PouId) -> Option<&FunctionSignature> {
        self.entry(id).and_then(|entry| entry.function.as_ref())
    }

    /// The body of the function block whose method `id` is, or `None` if
    /// `id` is not a method this table declares.
    pub fn method_of(&self, id: PouId) -> Option<PouId> {
        self.entry(id).and_then(|entry| entry.method_of)
    }

    /// Declares the POU `id` a function or method of `signature`, or a P9998
    /// at `span` if this table did not allocate `id` or declares it already.
    pub(crate) fn declare_function(
        &mut self,
        id: PouId,
        signature: FunctionSignature,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        const CONSTRUCTOR: &str = "ProgramBuilder::declare_function";
        let entry = match id.stamp() == self.stamp {
            true => self.pous.get_mut(id.index() as usize),
            false => None,
        };
        let Some(entry) = entry else {
            return Err(broken(
                CONSTRUCTOR,
                span,
                format!("{id} is not a POU of this program"),
            ));
        };
        if entry.function.is_some() {
            return Err(broken(
                CONSTRUCTOR,
                span,
                format!("{id} is declared already"),
            ));
        }
        entry.function = Some(signature);
        Ok(())
    }

    fn entry(&self, id: PouId) -> Option<&PouEntry> {
        if id.stamp() != self.stamp {
            return None;
        }
        self.pous.get(id.index() as usize)
    }
}
