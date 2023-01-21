//! Provides definitions specific to sequential function chart (SFC) elements.
//!
//! See section 2.
use crate::common::*;
use crate::core::Id;
use crate::textual::*;

/// 2.6.4.4 Action qualifiers defined for each step/action association.
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
    // TODO there are more
}

impl ActionQualifier {
    pub fn from_char(l: char) -> ActionQualifier {
        match l {
            'N' => ActionQualifier::N,
            'R' => ActionQualifier::R,
            'S' => ActionQualifier::S,
            'L' => ActionQualifier::L,
            'D' => ActionQualifier::D,
            'P' => ActionQualifier::P,
            // TODO error message
            _ => panic!(),
        }
    }
}

/// 2.6.5.2 Associated actions with steps.
#[derive(Debug, PartialEq, Clone)]
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

/// 2.6.2 Steps
#[derive(Debug, PartialEq, Clone)]
pub struct Step {
    pub name: Id,
    pub action_associations: Vec<ActionAssociation>,
}

/// 2.6.3 Transition
#[derive(Debug, PartialEq, Clone)]
pub struct Transition {
    pub name: Option<Id>,
    pub priority: Option<u32>,
    pub from: Vec<Id>,
    pub to: Vec<Id>,
    pub condition: ExprKind,
}

/// 2.6.4 Action
#[derive(Debug, PartialEq, Clone)]
pub struct Action {
    pub name: Id,
    pub body: FunctionBlockBody,
}

/// Grouping for SFC keyword-defined elements.
#[derive(Debug, PartialEq, Clone)]
pub enum Element {
    Action(Action),
    Transition(Transition),
    Step(Step),
    InitialStep(Step),
}

impl Element {
    pub fn action(name: &str, body: Vec<StmtKind>) -> Element {
        Element::Action(Action {
            name: Id::from(name),
            body: FunctionBlockBody::Statements(Statements { body }),
        })
    }

    pub fn transition(from: &str, to: &str, condition: ExprKind) -> Element {
        Element::Transition(Transition {
            name: None,
            priority: None,
            from: vec![Id::from(from)],
            to: vec![Id::from(to)],
            condition,
        })
    }

    pub fn step(name: Id, action_associations: Vec<ActionAssociation>) -> Element {
        Element::Step(Step {
            name,
            action_associations,
        })
    }

    pub fn initial_step(name: &str, action_associations: Vec<ActionAssociation>) -> Element {
        Element::InitialStep(Step {
            name: Id::from(name),
            action_associations,
        })
    }
}

/// Grouping of related items that represent and a complete SFC.
#[derive(Debug, PartialEq, Clone)]
pub struct Network {
    pub initial_step: Element,
    pub elements: Vec<Element>,
}
