use crate::ast::*;
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
    pub name: String,
    pub qualifier: Option<ActionQualifier>,
    pub indicators: Vec<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Element {
    Action {
        name: String,
        body: FunctionBlockBody,
    },

    Transition {
        name: Option<String>,
        priority: Option<u32>,
        from: Vec<String>,
        to: Vec<String>,
        condition: ExprKind,
    },

    Step {
        name: String,
        action_associations: Vec<ActionAssociation>,
    },

    InitialStep {
        name: String,
        action_associations: Vec<ActionAssociation>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub struct Network {
    pub initial_step: Element,
    pub elements: Vec<Element>
}