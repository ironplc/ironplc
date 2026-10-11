//! The variables and POUs of a lowered program.
//!
//! Each table allocates the ids of its entries, so an id always names an
//! entry, and a place rooted at a variable takes its type from the table
//! rather than from its caller.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::ids::{PouId, TypeId, VarId};
use super::invariant::broken;
use super::types::TypeTable;
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

/// Every variable of a lowered program, by [`VarId`].
#[derive(Clone, Debug, Default)]
pub struct VariableTable {
    variables: Vec<Variable>,
}

impl VariableTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a variable named `name`, owned by `owner`, of the type `ty`,
    /// which must be in `types` (REQ-LOW-lowering-026).
    pub fn push(
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
        let id = VarId::new(self.variables.len() as u32);
        self.variables.push(Variable { name, owner, ty });
        Ok(id)
    }

    /// The variable `id` names, or `None` if this table did not allocate
    /// `id`.
    pub fn get(&self, id: VarId) -> Option<&Variable> {
        self.variables.get(id.index() as usize)
    }

    /// Every variable, with its id, in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = (VarId, &Variable)> {
        self.variables
            .iter()
            .enumerate()
            .map(|(index, variable)| (VarId::new(index as u32), variable))
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

/// Every POU of a lowered program, by [`PouId`].
#[derive(Clone, Debug, Default)]
pub struct PouTable {
    names: Vec<DebugName>,
}

impl PouTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the POU named `name` and returns the id that names it.
    pub fn push(&mut self, name: DebugName) -> PouId {
        let id = PouId::new(self.names.len() as u32);
        self.names.push(name);
        id
    }

    /// The name of the POU `id`, or `None` if this table did not allocate
    /// `id`.
    pub fn name(&self, id: PouId) -> Option<&DebugName> {
        self.names.get(id.index() as usize)
    }
}
