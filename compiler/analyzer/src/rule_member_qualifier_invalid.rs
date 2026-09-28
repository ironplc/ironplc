//! Semantic rule that checks the qualifiers on function blocks and methods
//! (e.g. `METHOD PUBLIC FINAL Start`): which ones may be combined, in which
//! order, and where each is allowed.
//!
//! The grammar accepts qualifiers in any order and number, so that a
//! mistake is reported here with a message that says what is wrong instead
//! of as a syntax error at the second qualifier. The rules follow TwinCAT
//! 3.1.4024, checked in XAE (see `specs/design/beckhoff-twincat-dialect.md`
//! §1.5):
//!
//! * a qualifier may appear only once, and there is at most one access
//!   specifier;
//! * the access specifier comes first, before `FINAL`/`ABSTRACT`/`OVERRIDE`;
//! * `ABSTRACT` and `FINAL` exclude each other;
//! * a function block may not be `PRIVATE`, `PROTECTED` or `OVERRIDE`;
//! * an `ABSTRACT` method has no body and belongs to an `ABSTRACT` function
//!   block.
//!
//! Access specifiers and `FINAL` are metadata only (ADR-0041): calling a
//! `PRIVATE` method from outside, or extending a `FINAL` function block, is
//! not checked.
//!
//! Without `--allow-fb-inheritance` the rule does nothing: a method cannot
//! be declared, and a function block qualifier is already reported by
//! `rule_member_qualifier_allowed` (P4062).
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Motor
//! METHOD FINAL PUBLIC Start
//! END_METHOD
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::{FunctionBlockDeclaration, MethodDeclaration},
    diagnostic::{Diagnostic, Label},
    member_qualifier::{AccessSpecifier, MemberQualifier, MemberQualifierKind, MemberQualifiers},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};

pub fn apply(
    lib: &ironplc_dsl::common::Library,
    _context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    if !options.allow_fb_inheritance {
        return Ok(());
    }

    run_rule(
        RuleMemberQualifierInvalid {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleMemberQualifierInvalid {
    diagnostics: Vec<Diagnostic>,
}

impl RuleMemberQualifierInvalid {
    fn report(&mut self, qualifier: &MemberQualifier, owner: &str, message: String) {
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::MemberQualifierInvalid,
                Label::span(qualifier.span.clone(), message),
            )
            .with_context("qualifier", &qualifier.kind.keyword().to_string())
            .with_context("declaration", &owner.to_string()),
        );
    }

    /// The checks that apply to function blocks and methods alike. Each
    /// qualifier is compared with the ones before it, so a problem is
    /// reported once, at the qualifier that causes it.
    fn check_combination(&mut self, qualifiers: &MemberQualifiers, owner: &str) {
        let all: Vec<&MemberQualifier> = qualifiers.iter().collect();
        for (index, qualifier) in all.iter().enumerate() {
            let earlier = &all[..index];
            let keyword = qualifier.kind.keyword();
            if earlier.iter().any(|e| e.kind == qualifier.kind) {
                self.report(
                    qualifier,
                    owner,
                    format!("{keyword} appears more than once"),
                );
            } else if is_access(qualifier.kind) && earlier.iter().any(|e| is_access(e.kind)) {
                self.report(
                    qualifier,
                    owner,
                    format!("{keyword} is a second access specifier"),
                );
            } else if is_access(qualifier.kind) && !earlier.is_empty() {
                self.report(
                    qualifier,
                    owner,
                    format!("{keyword} must come before {}", earlier[0].kind.keyword()),
                );
            } else if let Some(other) = excluded_by(qualifier.kind)
                .and_then(|other| earlier.iter().find(|e| e.kind == other))
            {
                self.report(
                    qualifier,
                    owner,
                    format!("{keyword} cannot be combined with {}", other.kind.keyword()),
                );
            }
        }
    }

    fn check_function_block(&mut self, node: &FunctionBlockDeclaration) {
        let Some(oop) = &node.oop else {
            return;
        };
        let owner = node.name.to_string();
        self.check_combination(&oop.qualifiers, &owner);
        for qualifier in oop.qualifiers.iter() {
            if matches!(
                qualifier.kind,
                MemberQualifierKind::Access(AccessSpecifier::Private)
                    | MemberQualifierKind::Access(AccessSpecifier::Protected)
                    | MemberQualifierKind::Override
            ) {
                self.report(
                    qualifier,
                    &owner,
                    format!(
                        "{} is not allowed on a function block",
                        qualifier.kind.keyword()
                    ),
                );
            }
        }
    }

    fn check_method(
        &mut self,
        node: &MethodDeclaration,
        function_block: &FunctionBlockDeclaration,
    ) {
        let owner = node.name.to_string();
        self.check_combination(&node.qualifiers, &owner);
        for qualifier in node.qualifiers.iter() {
            if qualifier.kind != MemberQualifierKind::Abstract {
                continue;
            }
            if !function_block.is_abstract() {
                self.report(
                    qualifier,
                    &owner,
                    format!(
                        "ABSTRACT method in {}, which is not ABSTRACT",
                        function_block.name
                    ),
                );
            }
            if !node.body.is_empty() {
                self.report(qualifier, &owner, "ABSTRACT method has a body".to_string());
            }
        }
    }
}

fn is_access(kind: MemberQualifierKind) -> bool {
    matches!(kind, MemberQualifierKind::Access(_))
}

/// The qualifier that cannot appear together with `kind`.
fn excluded_by(kind: MemberQualifierKind) -> Option<MemberQualifierKind> {
    match kind {
        MemberQualifierKind::Abstract => Some(MemberQualifierKind::Final),
        MemberQualifierKind::Final => Some(MemberQualifierKind::Abstract),
        _ => None,
    }
}

impl DiagnosticVisitor for RuleMemberQualifierInvalid {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleMemberQualifierInvalid {
    type Value = ();

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.check_function_block(node);
        for method in &node.methods {
            self.check_method(method, node);
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn opts_flag() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    /// Runs the rule and returns the label message of each diagnostic.
    fn messages(source: &str) -> Vec<String> {
        let opts = opts_flag();
        let (library, context) = crate::test_helpers::resolve_fresh_with(source, &opts);
        match apply(&library, &context, &opts) {
            Ok(()) => vec![],
            Err(errors) => errors
                .into_iter()
                .inspect(|e| assert_eq!(e.code, Problem::MemberQualifierInvalid.code()))
                .map(|e| e.primary.message)
                .collect(),
        }
    }

    fn fb(header: &str) -> String {
        format!("{header}\nVAR\n    x : INT;\nEND_VAR\nEND_FUNCTION_BLOCK")
    }

    fn method_in(fb_header: &str, method: &str) -> String {
        format!("{fb_header}\nVAR\n    x : INT;\nEND_VAR\n{method}\nEND_FUNCTION_BLOCK")
    }

    // Function block cases from the TwinCAT 4024 table (design doc §1.5).
    #[rstest]
    #[case::public("FUNCTION_BLOCK PUBLIC FB_Motor")]
    #[case::internal("FUNCTION_BLOCK INTERNAL FB_Motor")]
    #[case::final_only("FUNCTION_BLOCK FINAL FB_Motor")]
    #[case::public_final("FUNCTION_BLOCK PUBLIC FINAL FB_Motor")]
    #[case::public_abstract("FUNCTION_BLOCK PUBLIC ABSTRACT FB_Motor")]
    fn apply_when_fb_qualifiers_valid_then_ok(#[case] header: &str) {
        assert_eq!(messages(&fb(header)), Vec::<String>::new());
    }

    #[rstest]
    #[case::private(
        "FUNCTION_BLOCK PRIVATE FB_Motor",
        "PRIVATE is not allowed on a function block"
    )]
    #[case::protected(
        "FUNCTION_BLOCK PROTECTED FB_Motor",
        "PROTECTED is not allowed on a function block"
    )]
    #[case::override_(
        "FUNCTION_BLOCK OVERRIDE FB_Motor",
        "OVERRIDE is not allowed on a function block"
    )]
    #[case::abstract_final(
        "FUNCTION_BLOCK ABSTRACT FINAL FB_Motor",
        "FINAL cannot be combined with ABSTRACT"
    )]
    #[case::final_abstract(
        "FUNCTION_BLOCK FINAL ABSTRACT FB_Motor",
        "ABSTRACT cannot be combined with FINAL"
    )]
    #[case::final_public(
        "FUNCTION_BLOCK FINAL PUBLIC FB_Motor",
        "PUBLIC must come before FINAL"
    )]
    fn apply_when_fb_qualifiers_invalid_then_error(#[case] header: &str, #[case] expected: &str) {
        assert_eq!(messages(&fb(header)), vec![expected.to_string()]);
    }

    // Method cases from the TwinCAT 4024 table (design doc §1.5).
    #[rstest]
    #[case::internal(
        "FUNCTION_BLOCK FB_Motor",
        "METHOD INTERNAL M\n    x := 1;\nEND_METHOD"
    )]
    #[case::public_final(
        "FUNCTION_BLOCK FB_Motor",
        "METHOD PUBLIC FINAL M : BOOL\n    M := TRUE;\nEND_METHOD"
    )]
    #[case::public_abstract_in_abstract_fb(
        "FUNCTION_BLOCK ABSTRACT FB_Motor",
        "METHOD PUBLIC ABSTRACT M : BOOL\nEND_METHOD"
    )]
    // XAE rejects `METHOD OVERRIDE` that redeclares a base method, for a
    // reason that is not known. OVERRIDE is Edition 3 syntax, so it is
    // accepted here.
    #[case::override_(
        "FUNCTION_BLOCK FB_Motor",
        "METHOD PUBLIC OVERRIDE M\n    x := 1;\nEND_METHOD"
    )]
    // A body with only an empty statement is empty.
    #[case::abstract_with_empty_statement(
        "FUNCTION_BLOCK ABSTRACT FB_Motor",
        "METHOD ABSTRACT M\n;\nEND_METHOD"
    )]
    fn apply_when_method_qualifiers_valid_then_ok(#[case] fb_header: &str, #[case] method: &str) {
        assert_eq!(
            messages(&method_in(fb_header, method)),
            Vec::<String>::new()
        );
    }

    #[rstest]
    #[case::final_public(
        "FUNCTION_BLOCK FB_Motor",
        "METHOD FINAL PUBLIC M\n    x := 1;\nEND_METHOD",
        "PUBLIC must come before FINAL"
    )]
    #[case::abstract_public(
        "FUNCTION_BLOCK ABSTRACT FB_Motor",
        "METHOD ABSTRACT PUBLIC M\nEND_METHOD",
        "PUBLIC must come before ABSTRACT"
    )]
    #[case::final_abstract(
        "FUNCTION_BLOCK ABSTRACT FB_Motor",
        "METHOD FINAL ABSTRACT M\nEND_METHOD",
        "ABSTRACT cannot be combined with FINAL"
    )]
    #[case::public_private(
        "FUNCTION_BLOCK FB_Motor",
        "METHOD PUBLIC PRIVATE M\n    x := 1;\nEND_METHOD",
        "PRIVATE is a second access specifier"
    )]
    #[case::public_public(
        "FUNCTION_BLOCK FB_Motor",
        "METHOD PUBLIC PUBLIC M\n    x := 1;\nEND_METHOD",
        "PUBLIC appears more than once"
    )]
    #[case::abstract_in_concrete_fb(
        "FUNCTION_BLOCK FB_Motor",
        "METHOD PUBLIC ABSTRACT M\nEND_METHOD",
        "ABSTRACT method in FB_Motor, which is not ABSTRACT"
    )]
    #[case::abstract_with_body(
        "FUNCTION_BLOCK ABSTRACT FB_Motor",
        "METHOD PUBLIC ABSTRACT M\n    x := 1;\nEND_METHOD",
        "ABSTRACT method has a body"
    )]
    fn apply_when_method_qualifiers_invalid_then_error(
        #[case] fb_header: &str,
        #[case] method: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            messages(&method_in(fb_header, method)),
            vec![expected.to_string()]
        );
    }

    // Both method-position problems at once are both reported.
    #[test]
    fn apply_when_abstract_method_with_body_in_concrete_fb_then_two_errors() {
        let source = method_in(
            "FUNCTION_BLOCK FB_Motor",
            "METHOD ABSTRACT M\n    x := 1;\nEND_METHOD",
        );
        assert_eq!(messages(&source).len(), 2);
    }

    #[test]
    fn apply_when_no_qualifiers_then_ok() {
        let source = method_in(
            "FUNCTION_BLOCK FB_Motor",
            "METHOD M\n    x := 1;\nEND_METHOD",
        );
        assert_eq!(messages(&source), Vec::<String>::new());
    }

    // Without the flag, P4062 already reports the qualifier.
    rule_ok!(
        apply_when_flag_disabled_then_ok,
        "
FUNCTION_BLOCK PRIVATE FB_Motor
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK"
    );
}
