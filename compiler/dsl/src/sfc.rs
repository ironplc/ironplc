//! Provides definitions specific to sequential function chart (SFC) elements.
//!
//! See section 2 (especially 2.6).
use core::fmt;

use crate::common::*;
use crate::core::Id;
use crate::textual::*;

use crate::fold::Fold;
use crate::visitor::Visitor;
use dsl_macro_derive::Recurse;

/// Sequential function chart.
///
/// See section 2.6.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Sfc {
    pub networks: Vec<Network>,
}

/// Grouping of related items that represent and a complete SFC.
///
/// See section 2.6.2.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Network {
    pub initial_step: Step,
    pub elements: Vec<ElementKind>,
}

/// Grouping for SFC keyword-defined elements.
///
/// See section 2.6.2.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub enum ElementKind {
    Step(Step),
    Transition(Transition),
    Action(Action),
}

impl ElementKind {
    pub fn action(name: &str, body: Vec<StmtKind>) -> ElementKind {
        ElementKind::Action(Action {
            name: Id::from(name),
            body: FunctionBlockBodyKind::Statements(Statements { body }),
        })
    }

    pub fn transition(from: &str, to: &str, condition: ExprKind) -> ElementKind {
        ElementKind::Transition(Transition {
            name: None,
            priority: None,
            from: vec![Id::from(from)],
            to: vec![Id::from(to)],
            condition,
        })
    }

    pub fn step(name: Id, action_associations: Vec<ActionAssociation>) -> ElementKind {
        ElementKind::Step(Step {
            name,
            action_associations,
        })
    }
}

/// Step item for a SFC.
///
/// See section 2.6.2.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Step {
    pub name: Id,
    pub action_associations: Vec<ActionAssociation>,
}

/// Transition item for a SFC.
///
/// See section 2.6.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Transition {
    pub name: Option<Id>,
    #[recurse(ignore)]
    pub priority: Option<u32>,
    pub from: Vec<Id>,
    pub to: Vec<Id>,
    pub condition: ExprKind,
}

/// Action item for a SFC.
///
/// See section 2.6.4. Action
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Action {
    pub name: Id,
    pub body: FunctionBlockBodyKind,
}

/// Action qualifiers defined for each step/action association.
///
/// See section 2.6.4.4.
#[derive(Debug, PartialEq, Clone)]
pub enum ActionQualifier {
    /// Non-stored
    N,
    /// Overriding Reset
    R,
    /// Set (stored)
    S,
    /// Time limited
    L,
    /// Time delayed
    D,
    /// Pulse
    P,
    // Stored and time delayed
    SD,
    // Delayed and stored
    DS,
    // Stored and time limited
    SL,
    // Pulse (rising edge)
    PR,
    // Pulse (falling edge)
    PF,
}

impl fmt::Display for ActionQualifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Associated actions with steps.
///
/// See section 2.6.5.2.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct ActionAssociation {
    pub name: Id,
    #[recurse(ignore)]
    pub qualifier: Option<ActionQualifier>,
    pub indicators: Vec<Id>,
}

impl ActionAssociation {
    pub fn new(name: &str, qualifier: Option<ActionQualifier>) -> ActionAssociation {
        ActionAssociation {
            name: Id::from(name),
            qualifier,
            indicators: vec![],
        }
    }
}
