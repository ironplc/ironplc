//! Provides definitions specific to configuration elements.
//!
//! See section 2 (especially 2.7).
use time::Duration;

use crate::{
    common::{
        AddressAssignment, ConstantKind, EnumeratedValue, HasVariables, InitialValueAssignmentKind,
        VarDecl,
    },
    core::Id,
    textual::SymbolicVariableKind,
};

use crate::fold::Fold;
use crate::visitor::Visitor;
use dsl_macro_derive::Recurse;

/// Resource assigns tasks to a particular CPU.
///
/// See section 2.7.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct ResourceDeclaration {
    /// Symbolic name for a CPU
    pub name: Id,
    /// The identifier for a CPU
    pub resource: Id,
    /// Global variables in the scope of the resource.
    ///
    /// Global variables are not in scope for other resources.
    pub global_vars: Vec<VarDecl>,
    /// Defines the configuration of programs on this resource.
    pub tasks: Vec<TaskConfiguration>,
    /// Defines runnable programs.
    ///
    /// A runnable program can be associated with a task configuration
    /// by name.
    pub programs: Vec<ProgramConfiguration>,
}

impl HasVariables for ResourceDeclaration {
    fn variables(&self) -> &Vec<VarDecl> {
        &self.global_vars
    }
}

/// Program configurations.
///
/// The function block task, sources and sinks are declared using variants in the specification.
/// But when used, we really need to treat them separately, so we split them up in the object model.
///
/// See section 2.7.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct ProgramConfiguration {
    pub name: Id,
    pub task_name: Option<Id>,
    pub type_name: Id,
    pub fb_tasks: Vec<FunctionBlockTask>,
    pub sources: Vec<ProgramConnectionSource>,
    pub sinks: Vec<ProgramConnectionSink>,
}

/// Configuration declaration.
///
/// See section 2.7.2.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct ConfigurationDeclaration {
    pub name: Id,
    pub global_var: Vec<VarDecl>,
    pub resource_decl: Vec<ResourceDeclaration>,
    pub fb_inits: Vec<FunctionBlockInit>,
    pub located_var_inits: Vec<LocatedVarInit>,
}

impl HasVariables for ConfigurationDeclaration {
    fn variables(&self) -> &Vec<VarDecl> {
        &self.global_var
    }
}

/// See section 2.7.2.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct AccessDeclaration {
    pub identifier: Id,
    pub path: AccessPathKind,
    pub type_name: Id,
    #[recurse(ignore)]
    pub direction: Option<Direction>,
}

/// See section 2.7.2.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub enum AccessPathKind {
    Direct(DirectAccessPath),
    Symbolic(SymbolicAccessPath),
}

/// See section 2.7.2.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct DirectAccessPath {
    pub resource_name: Option<Id>,
    pub variable: AddressAssignment,
}

/// See section 2.7.2.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct SymbolicAccessPath {
    pub resource_name: Option<Id>,
    pub program_name: Option<Id>,
    pub fb_name: Vec<Id>,
    pub variable: SymbolicVariableKind,
}

/// The direction indicates whether communication services can use the value.
///
/// See section 2.7.2.
#[derive(Clone, Debug, PartialEq)]
pub enum Direction {
    ReadWrite,
    ReadOnly,
}

/// Task configuration.
///
/// See section 2.7.2.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct TaskConfiguration {
    pub name: Id,
    #[recurse(ignore)]
    pub priority: u32,
    // TODO this might not be optional
    #[recurse(ignore)]
    pub interval: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct FunctionBlockTask {
    pub fb_name: Id,
    pub task_name: Id,
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct ProgramConnectionSource {
    pub dst: SymbolicVariableKind,
    pub src: ProgramConnectionSourceKind,
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub enum ProgramConnectionSourceKind {
    Constant(ConstantKind),
    EnumeratedValue(EnumeratedValue),
    GlobalVarReference(GlobalVarReference),
    DirectVariable(AddressAssignment),
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct ProgramConnectionSink {
    pub src: SymbolicVariableKind,
    pub dst: ProgramConnectionSinkKind,
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub enum ProgramConnectionSinkKind {
    GlobalVarReference(GlobalVarReference),
    DirectVariable(AddressAssignment),
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct GlobalVarReference {
    pub resource_name: Option<Id>,
    pub global_var_name: Id,
    pub structure_element_name: Option<Id>,
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct FunctionBlockInit {
    // TODO
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct LocatedVarInit {
    // TODO
}
