//! Provides definitions specific to configuration elements.
//!
//! See section 2 (especially 2.7).
use time::Duration;

use crate::{
    common::{HasVariables, VarDecl},
    core::Id,
};

use crate::fold::Fold;
use crate::visitor::Visitor;
use dsl_macro_derive::Recurse;

/// Resource assigns tasks to a particular CPU.
///
/// See section 2.7.1.
#[derive(Debug, PartialEq, Recurse)]
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
/// See section 2.7.1.
#[derive(Debug, PartialEq, Recurse)]
pub struct ProgramConfiguration {
    pub name: Id,
    pub task_name: Option<Id>,
    pub type_name: Id,
}

/// Configuration declaration,
///
/// See section 2.7.1.
#[derive(Debug, PartialEq, Recurse)]
pub struct ConfigurationDeclaration {
    pub name: Id,
    pub global_var: Vec<VarDecl>,
    pub resource_decl: Vec<ResourceDeclaration>,
}

impl HasVariables for ConfigurationDeclaration {
    fn variables(&self) -> &Vec<VarDecl> {
        &self.global_var
    }
}

/// Task configuration.
///
/// See section 2.7.2.
#[derive(Debug, PartialEq, Recurse)]
pub struct TaskConfiguration {
    pub name: Id,
    #[recurse(ignore)]
    pub priority: u32,
    // TODO this might not be optional
    #[recurse(ignore)]
    pub interval: Option<Duration>,
}
