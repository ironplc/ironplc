//! Provides definitions specific to sequential function chart (SFC) elements.
//!
//! See section 2 (especially 2.6).
use core::fmt;

use crate::common::*;
use crate::core::Id;
use crate::textual::*;
use crate::time::*;

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
    SD(ActionTimeKind),
    // Delayed and stored
    DS(ActionTimeKind),
    // Stored and time limited
    SL(ActionTimeKind),
    // Pulse (rising edge)
    PR(ActionTimeKind),
    // Pulse (falling edge)
    PF(ActionTimeKind),
}

impl fmt::Display for ActionQualifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl ActionQualifier {
    pub fn recurse_visit<V: Visitor<E> + ?Sized, E>(&self, v: &mut V) -> Result<V::Value, E> {
        match self {
            ActionQualifier::N => Ok(V::Value::default()),
            ActionQualifier::R => Ok(V::Value::default()),
            ActionQualifier::S => Ok(V::Value::default()),
            ActionQualifier::L => Ok(V::Value::default()),
            ActionQualifier::D => Ok(V::Value::default()),
            ActionQualifier::P => Ok(V::Value::default()),
            ActionQualifier::SD(node) => v.visit_action_time_kind(node),
            ActionQualifier::DS(node) => v.visit_action_time_kind(node),
            ActionQualifier::SL(node) => v.visit_action_time_kind(node),
            ActionQualifier::PR(node) => v.visit_action_time_kind(node),
            ActionQualifier::PF(node) => v.visit_action_time_kind(node),
        }
    }

    pub fn recurse_fold<F: Fold<E> + ?Sized, E>(self, f: &mut F) -> Result<Self, E> {
        match self {
            ActionQualifier::N => Ok(ActionQualifier::N),
            ActionQualifier::R => Ok(ActionQualifier::R),
            ActionQualifier::S => Ok(ActionQualifier::S),
            ActionQualifier::L => Ok(ActionQualifier::L),
            ActionQualifier::D => Ok(ActionQualifier::D),
            ActionQualifier::P => Ok(ActionQualifier::P),
            ActionQualifier::SD(node) => Ok(ActionQualifier::SD(f.fold_action_time_kind(node)?)),
            ActionQualifier::DS(node) => Ok(ActionQualifier::DS(f.fold_action_time_kind(node)?)),
            ActionQualifier::SL(node) => Ok(ActionQualifier::SL(f.fold_action_time_kind(node)?)),
            ActionQualifier::PR(node) => Ok(ActionQualifier::PR(f.fold_action_time_kind(node)?)),
            ActionQualifier::PF(node) => Ok(ActionQualifier::PF(f.fold_action_time_kind(node)?)),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Recurse)]
pub enum ActionTimeKind {
    Duration(DurationLiteral),
    VariableName(Id),
}

#[derive(Debug, PartialEq, Clone)]
pub enum TimedQualifier {
    L,
    D,
    SD,
    DS,
    SL,
}

impl fmt::Display for TimedQualifier {
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
