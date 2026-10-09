//! Tests that codegen converts nothing the analyzer did not record: a
//! program analyzed as usual, with its recorded conversions then removed,
//! is an internal error (P9998) where a conversion was needed, rather than
//! compiled with a conversion codegen chose itself (ADR-0056).

use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::{Expr, ExprKind};
use ironplc_parser::options::{CompilerOptions, Dialect};
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::common::{compile_analyzed, parse};

/// Removes every recorded conversion, leaving its operand in its place.
struct RemoveConversions;

impl Fold<()> for RemoveConversions {
    fn fold_expr(&mut self, node: Expr) -> Result<Expr, ()> {
        match node.kind {
            ExprKind::ImplicitConversion(inner) => self.fold_expr(*inner),
            _ => node.recurse_fold(self),
        }
    }
}

/// The problem code of compiling `x := <value>` for an `x` of type `target`
/// with every recorded conversion removed, or `None` when it compiles.
fn code_without_conversions(target: &str, value: &str) -> Option<String> {
    let source = format!(
        "PROGRAM main
         VAR x : {target}; t : TIME; lt : LTIME; da : DATE; lda : LDATE; d : DINT;
             l : LINT; r : REAL; END_VAR
         x := {value};
         END_PROGRAM"
    );
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    let (library, context) = parse(&source, &options);
    let library = RemoveConversions.fold_library(library).unwrap();
    compile_analyzed(&library, &context, &options)
        .err()
        .map(|diagnostic| diagnostic.code)
}

#[spec_test(REQ_IC_codegen_013)]
#[rstest]
#[case::short_time_of_long_form("LTIME", "lt + t")]
#[case::short_date_of_long_form("LTIME", "SUB_LDATE_LDATE(lda, da)")]
#[case::dint_scaling_long_time("LTIME", "lt * d")]
#[case::real_scaling_long_time("LTIME", "MUL_LTIME(lt, r)")]
#[case::lint_scaling_short_time("TIME", "t * l")]
fn compile_when_time_operand_conversion_removed_then_internal_error(
    #[case] target: &str,
    #[case] value: &str,
) {
    assert_eq!(
        code_without_conversions(target, value).as_deref(),
        Some("P9998")
    );
}

#[test]
fn compile_when_time_operands_need_no_conversion_then_compiles() {
    assert_eq!(code_without_conversions("LTIME", "lt + lt"), None);
}
