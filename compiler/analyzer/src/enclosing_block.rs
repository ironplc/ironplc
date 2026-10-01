//! The function block that encloses the code a pass is walking.
//!
//! Inside a function block's body, its methods and its property accessors,
//! `THIS^` names that block and `SUPER^` its `EXTENDS` base. Several passes
//! need that answer; each feeds this tracker from its
//! [`Visitor::enter_scope`]/[`Visitor::exit_scope`] hooks (or the [`Fold`]
//! equivalents) instead of working it out from its own scope stack.
//!
//! [`Visitor::enter_scope`]: ironplc_dsl::visitor::Visitor::enter_scope
//! [`Visitor::exit_scope`]: ironplc_dsl::visitor::Visitor::exit_scope
//! [`Fold`]: ironplc_dsl::fold::Fold

use ironplc_dsl::common::TypeName;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::SelfRefKind;

/// A function block and its `EXTENDS` base, if it has one.
#[derive(Clone, Debug, PartialEq)]
struct Block {
    name: TypeName,
    base: Option<TypeName>,
}

/// One entry per open scope: the function block that scope is inside, or
/// `None` for a scope outside any function block (a program, a function).
#[derive(Debug, Default)]
pub(crate) struct EnclosingBlock {
    scopes: Vec<Option<Block>>,
}

impl EnclosingBlock {
    /// Records the scope the traversal is entering. A method (or property
    /// accessor) is inside the block whose scope is open beneath it.
    pub(crate) fn enter(&mut self, node: &ScopeNode<'_>) {
        let block = match node {
            ScopeNode::FunctionBlock(node) => Some(Block {
                name: node.name.clone(),
                base: node.oop.as_ref().and_then(|oop| oop.base.clone()),
            }),
            ScopeNode::Method(_) => self.scopes.last().cloned().flatten(),
            ScopeNode::Function(_) | ScopeNode::Program(_) => None,
        };
        self.scopes.push(block);
    }

    /// Records that the traversal left the innermost open scope.
    pub(crate) fn exit(&mut self) {
        self.scopes.pop();
    }

    /// The function block the traversal is inside, if any.
    pub(crate) fn block(&self) -> Option<&TypeName> {
        self.current().map(|block| &block.name)
    }

    /// The type `THIS^` or `SUPER^` names here: the enclosing function
    /// block, or its base. `None` outside a function block, and for
    /// `SUPER^` in a block without a base.
    pub(crate) fn self_type(&self, kind: SelfRefKind) -> Option<&TypeName> {
        let block = self.current()?;
        match kind {
            SelfRefKind::This => Some(&block.name),
            SelfRefKind::Super => block.base.as_ref(),
        }
    }

    fn current(&self) -> Option<&Block> {
        self.scopes.last()?.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::EnclosingBlock;
    use ironplc_dsl::common::TypeName;
    use ironplc_dsl::core::FileId;
    use ironplc_dsl::scope::ScopeNode;
    use ironplc_dsl::textual::SelfRefKind;
    use ironplc_dsl::visitor::Visitor;
    use ironplc_parser::{options::CompilerOptions, parse_program};
    use std::convert::Infallible;

    /// What `THIS^` and `SUPER^` name in each scope of a library, in the
    /// order the scopes are entered.
    #[derive(Default)]
    struct Recorder {
        enclosing: EnclosingBlock,
        seen: Vec<(Option<String>, Option<String>)>,
    }

    impl Visitor<Infallible> for Recorder {
        type Value = ();

        fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
            self.enclosing.enter(&node);
            let name = |t: Option<&TypeName>| t.map(|t| t.to_string());
            self.seen.push((
                name(self.enclosing.self_type(SelfRefKind::This)),
                name(self.enclosing.self_type(SelfRefKind::Super)),
            ));
            Ok(())
        }

        fn exit_scope(&mut self) {
            self.enclosing.exit();
        }
    }

    fn record(program: &str) -> Vec<(Option<String>, Option<String>)> {
        let options = CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        };
        // Parsed only, so the scopes are entered in source order.
        let library = parse_program(program, &FileId::default(), &options).unwrap();
        let mut recorder = Recorder::default();
        let Ok(()) = recorder.walk(&library);
        recorder.seen
    }

    fn some(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn self_type_when_block_with_base_and_method_then_block_and_base() {
        let seen = record(
            "
FUNCTION_BLOCK FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
METHOD M
END_METHOD
END_FUNCTION_BLOCK",
        );
        assert_eq!(
            vec![
                (some("FB_Base"), None),
                (some("FB_Derived"), some("FB_Base")),
                (some("FB_Derived"), some("FB_Base")),
            ],
            seen
        );
    }

    #[test]
    fn self_type_when_program_or_function_then_none() {
        let seen = record(
            "
FUNCTION F : INT
F := 1;
END_FUNCTION

PROGRAM main
END_PROGRAM",
        );
        assert_eq!(vec![(None, None), (None, None)], seen);
    }

    #[test]
    fn self_type_when_block_left_then_next_unit_is_outside() {
        let seen = record(
            "
FUNCTION_BLOCK FB_A
METHOD M
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
END_PROGRAM",
        );
        assert_eq!(
            vec![(some("FB_A"), None), (some("FB_A"), None), (None, None)],
            seen
        );
    }
}
