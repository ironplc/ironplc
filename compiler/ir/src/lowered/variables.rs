//! The variables and POUs of a lowered program.
//!
//! Each table allocates the ids of its entries, so an id always names an
//! entry, and a place rooted at a variable takes its type from the table
//! rather than from its caller.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::call::Param;
use super::ids::{FieldIdx, PouId, Stamp, TypeId, VarId};
use super::invariant::broken;
use super::standard_block::StandardFunctionBlock;
use super::stmt::Stmt;
use super::types::{DirectAddress, Nullability, Retention, Section, TypeKind, TypeTable};
use crate::execution::DebugName;

/// What a variable belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Global,
    Pou(PouId),
}

/// What a variable's declaration says of it besides its name, owner and
/// type: the section that declares it, its direct address and its
/// retention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declared {
    pub section: Section,
    pub location: Option<DirectAddress>,
    pub retention: Retention,
}

impl Declared {
    /// A variable of `section` with no direct address and no retention, such
    /// as a temporary lowering makes or the instance parameter of a body.
    pub fn in_section(section: Section) -> Self {
        Self {
            section,
            location: None,
            retention: Retention::None,
        }
    }
}

/// A variable of the lowered program that is not a field: a global, a
/// parameter, a local, a `VAR_TEMP`, or the instance parameter of a body.
#[derive(Clone, Debug)]
pub struct Variable {
    name: DebugName,
    owner: Owner,
    declared: Declared,
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

    pub fn section(&self) -> Section {
        self.declared.section
    }

    /// The direct address its declaration gives it, if any.
    pub fn location(&self) -> Option<&DirectAddress> {
        self.declared.location.as_ref()
    }

    pub fn retention(&self) -> Retention {
        self.declared.retention
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

    /// Adds a variable named `name`, owned by `owner`, declared as
    /// `declared` says, of the type `ty`, which must be in `types`
    /// (REQ-LOW-lowering-026).
    pub(crate) fn push(
        &mut self,
        types: &TypeTable,
        name: DebugName,
        owner: Owner,
        declared: Declared,
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
        self.variables.push(Variable {
            name,
            owner,
            declared,
            ty,
        });
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
    /// string of its encoding, a copy of an array or a structure, or, for a
    /// `VAR_IN_OUT` or a `VAR_OUTPUT`, a non-null reference, the place its
    /// argument passes (REQ-LOW-lowering-033, REQ-LOW-lowering-072,
    /// REQ-LOW-lowering-092). `None` when one is none of these.
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
/// type, a string of its encoding, or a copy of an array, a structure or a
/// function block instance.
pub(crate) fn by_value(types: &TypeTable, ty: TypeId) -> Option<Param> {
    match types.kind(ty)? {
        TypeKind::String(shape) => Some(Param::Str(shape.encoding)),
        TypeKind::Array { .. } | TypeKind::Structure { .. } | TypeKind::FunctionBlock { .. } => {
            Some(Param::Copy(ty))
        }
        TypeKind::Bool
        | TypeKind::Integer { .. }
        | TypeKind::BitString { .. }
        | TypeKind::Real { .. }
        | TypeKind::Temporal { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Subrange { .. }
        | TypeKind::Program { .. }
        | TypeKind::Reference { .. } => types.operation_type(ty).map(Param::Value),
    }
}

/// What a POU is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PouKind {
    Program,
    Function,
    FunctionBlock,
    /// A method of the function block whose body
    /// [`PouTable::method_of`] gives.
    Method,
}

/// What a POU's declaration gives it besides its name and signature: what
/// it is, the instance parameter its body works on, its locals, what each of
/// its declarations declares, and the variable that holds its result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PouParts {
    pub kind: PouKind,
    /// The instance parameter of a program, function block or method body;
    /// `None` for a function, which has none.
    pub instance: Option<VarId>,
    /// The locals of a function or method, and the `VAR_TEMP` of any body,
    /// in declaration order. Its parameters and result are its signature's.
    pub locals: Vec<VarId>,
    /// What each of its declarations declares, in the order it declares
    /// them (REQ-LOW-lowering-197).
    pub declarations: Vec<Declaration>,
    /// The variable that holds a function's or method's result: its
    /// signature's result or result parameter, or, for a method whose result
    /// is an array or a structure, the local that holds it. `None` for any
    /// other POU.
    pub result: Option<VarId>,
}

/// One declaration of a body: what its name declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Declaration {
    /// A field of the body's instance.
    Field(FieldIdx),
    /// A variable of the body's own, or a global a PLCopen program declares
    /// in its interface.
    Variable(VarId),
    /// The global a `VAR_EXTERNAL` names.
    External(VarId),
}

/// The statements a POU runs: the stores at its head that give its
/// variables their starting values (REQ-LOW-lowering-195, 201), then its
/// own.
#[derive(Clone, Debug)]
struct Body {
    statements: Vec<Stmt>,
    /// How many of `statements`, at their head, are the prologue.
    prologue: usize,
}

/// One POU: its name, its signature when it is a function or method a call
/// reaches, for a method the body of the function block it belongs to, and,
/// once the whole program is lowered, its parts and its body, or the
/// standard function block it stands for.
#[derive(Clone, Debug)]
struct PouEntry {
    name: DebugName,
    function: Option<FunctionSignature>,
    method_of: Option<PouId>,
    parts: Option<PouParts>,
    body: Option<Body>,
    standard_block: Option<StandardFunctionBlock>,
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
            parts: None,
            body: None,
            standard_block: None,
        });
        id
    }

    /// Every POU, with its id, in the order they were added.
    pub fn ids(&self) -> impl Iterator<Item = PouId> + '_ {
        (0..self.pous.len()).map(|index| PouId::new(self.stamp, index as u32))
    }

    /// The parts of the POU `id`, or `None` if they are not declared.
    pub fn parts(&self, id: PouId) -> Option<&PouParts> {
        self.entry(id).and_then(|entry| entry.parts.as_ref())
    }

    /// The body of the POU `id`, its prologue and then its own statements,
    /// or `None` if it is not set.
    pub fn body(&self, id: PouId) -> Option<&[Stmt]> {
        self.entry(id)
            .and_then(|entry| entry.body.as_ref())
            .map(|body| body.statements.as_slice())
    }

    /// The prologue of the POU `id`, the stores at the head of its body
    /// that give its variables their starting values, or `None` if its body
    /// is not set.
    pub fn prologue(&self, id: PouId) -> Option<&[Stmt]> {
        let body = self.entry(id)?.body.as_ref()?;
        body.statements.get(..body.prologue)
    }

    /// The statements of the body of the POU `id` after its prologue, or
    /// `None` if its body is not set.
    pub fn statements(&self, id: PouId) -> Option<&[Stmt]> {
        let body = self.entry(id)?.body.as_ref()?;
        body.statements.get(body.prologue..)
    }

    /// The standard function block the POU `id` stands for, which has no
    /// parts or body; `None` for any other POU.
    pub fn standard_block(&self, id: PouId) -> Option<StandardFunctionBlock> {
        self.entry(id).and_then(|entry| entry.standard_block)
    }

    /// Declares the parts of the POU `id`, or a P9998 at `span` if this
    /// table did not allocate `id` or declares its parts already. The
    /// [`ProgramBuilder`](super::ProgramBuilder) checks the parts first.
    pub(crate) fn declare_parts(
        &mut self,
        id: PouId,
        parts: PouParts,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        let entry = self.entry_mut("ProgramBuilder::declare_parts", id, span)?;
        if entry.parts.is_some() || entry.standard_block.is_some() {
            return Err(broken(
                "ProgramBuilder::declare_parts",
                span,
                format!("the parts of {id} are declared already"),
            ));
        }
        entry.parts = Some(parts);
        Ok(())
    }

    /// Sets the body of the POU `id`, `prologue` and then `statements`, or a
    /// P9998 at `span` if this table did not allocate `id` or its body is set
    /// already.
    pub(crate) fn set_body(
        &mut self,
        id: PouId,
        prologue: Vec<Stmt>,
        statements: Vec<Stmt>,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        let entry = self.entry_mut("ProgramBuilder::set_body", id, span)?;
        if entry.body.is_some() || entry.standard_block.is_some() {
            return Err(broken(
                "ProgramBuilder::set_body",
                span,
                format!("the body of {id} is set already"),
            ));
        }
        let length = prologue.len();
        let mut body = prologue;
        body.extend(statements);
        entry.body = Some(Body {
            statements: body,
            prologue: length,
        });
        Ok(())
    }

    /// Declares the POU `id` the standard function block `block`, or a
    /// P9998 at `span` if this table did not allocate `id`, or `id` has
    /// parts, a body or a standard block already.
    pub(crate) fn declare_standard_block(
        &mut self,
        id: PouId,
        block: StandardFunctionBlock,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        const CONSTRUCTOR: &str = "ProgramBuilder::declare_standard_block";
        let entry = self.entry_mut(CONSTRUCTOR, id, span)?;
        if entry.parts.is_some() || entry.body.is_some() || entry.standard_block.is_some() {
            return Err(broken(
                CONSTRUCTOR,
                span,
                format!("{id} is declared already"),
            ));
        }
        entry.standard_block = Some(block);
        Ok(())
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
        let entry = self.entry_mut(CONSTRUCTOR, id, span)?;
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

    /// The entry of `id`, or a P9998 at `span`, naming `constructor`, if this
    /// table did not allocate `id`.
    fn entry_mut(
        &mut self,
        constructor: &str,
        id: PouId,
        span: &SourceSpan,
    ) -> Result<&mut PouEntry, Diagnostic> {
        let entry = match id.stamp() == self.stamp {
            true => self.pous.get_mut(id.index() as usize),
            false => None,
        };
        entry.ok_or_else(|| {
            broken(
                constructor,
                span,
                format!("{id} is not a POU of this program"),
            )
        })
    }

    fn entry(&self, id: PouId) -> Option<&PouEntry> {
        if id.stamp() != self.stamp {
            return None;
        }
        self.pous.get(id.index() as usize)
    }
}
