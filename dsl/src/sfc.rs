//! Provides definitions specific to sequential function chart elements.
//!
//!  
use crate::ast::*;
use crate::core::Id;
use crate::dsl::*;

#[derive(Debug, PartialEq, Clone)]
pub enum ActionQualifier {
    N,
    R,
    S,
    P,
}

impl ActionQualifier {
    pub fn from_char(l: char) -> ActionQualifier {
        match l {
            'N' => return ActionQualifier::N,
            'R' => return ActionQualifier::R,
            'S' => return ActionQualifier::S,
            'P' => return ActionQualifier::P,
            // TODO error message
            _ => panic!(),
        }
    }
}

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
            qualifier: qualifier,
            indicators: vec![],
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Element {
    Action {
        name: Id,
        body: FunctionBlockBody,
    },

    Transition {
        name: Option<Id>,
        priority: Option<u32>,
        from: Vec<Id>,
        to: Vec<Id>,
        condition: ExprKind,
    },

    Step {
        name: Id,
        action_associations: Vec<ActionAssociation>,
    },

    InitialStep {
        name: Id,
        action_associations: Vec<ActionAssociation>,
    },
}

impl Element {
    pub fn action(name: &str, body: Vec<StmtKind>) -> Element {
        Element::Action {
            name: Id::from(name),
            body: FunctionBlockBody::Statements(Statements { body: body }),
        }
    }

    pub fn transition(from: &str, to: &str, condition: ExprKind) -> Element {
        Element::Transition {
            name: None,
            priority: None,
            from: vec![Id::from(from)],
            to: vec![Id::from(to)],
            condition: condition,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Network {
    pub initial_step: Element,
    pub elements: Vec<Element>,
}
