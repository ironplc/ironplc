//! Semantic rule that a bit or partial access selects a part of a variable
//! that exists.
//!
//! A variable's declared type fixes what parts it has: a `BYTE` has eight
//! bits and one byte, a `WORD` sixteen bits and two bytes. An index past the
//! last part names storage the variable does not have, and a slice wider than
//! the variable cannot be taken from it at all.
//!
//! Both spellings of bit access are checked -- `x.3` and the IEC
//! 61131-3:2013 form `x.%X3` -- along with the byte, word, dword and lword
//! slices (`x.%B1`). The `%` forms require `--allow-partial-access-syntax`.
//!
//! This bounds an *index into* a variable. What values that variable can
//! hold is a different question, and not this rule's.
//!
//! See section B.1.4.2.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK FB1
//!    VAR
//!       myWord : WORD;
//!       myBool : BOOL;
//!       myByte : BYTE;
//!    END_VAR
//!    myBool := myWord.0;     (* first of 16 bits *)
//!    myBool := myWord.15;    (* last of 16 bits *)
//!    myByte := myWord.%B1;   (* second of 2 bytes *)
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK FB1
//!    VAR
//!       myByte : BYTE;
//!       myBool : BOOL;
//!       myWord : WORD;
//!    END_VAR
//!    myBool := myByte.8;     (* a BYTE has bits 0..7 *)
//!    myWord := myByte.%W0;   (* a WORD does not fit in a BYTE *)
//!    myByte := myWord.%B2;   (* a WORD has bytes 0..1 *)
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::ScopeTracker,
    variable_type,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleBitAndPartialAccessRange {
            context,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleBitAndPartialAccessRange<'a> {
    context: &'a SemanticContext,
    /// Where the traversal is, to look variables up in the symbol
    /// environment.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleBitAndPartialAccessRange<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleBitAndPartialAccessRange<'_> {
    fn check_partial_access(&mut self, node: &PartialAccessVariable) {
        let accessed_type =
            match variable_type::of(&node.variable, self.context, &self.scope.current()) {
                Some(t) => t,
                None => return,
            };

        let base_bytes = match accessed_type.size_in_bytes() {
            Some(bytes) => bytes as u128,
            None => return,
        };

        let access_bytes: u128 = match node.size {
            PartialAccessSize::Byte => 1,
            PartialAccessSize::Word => 2,
            PartialAccessSize::DWord => 4,
            PartialAccessSize::LWord => 8,
        };

        if access_bytes > base_bytes {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::BitAccessOutOfRange,
                    Label::span(
                        node.index.span(),
                        format!(
                            "Partial access {}0 requires at least {} bytes but type has only {} bytes",
                            node.size.prefix(),
                            access_bytes,
                            base_bytes,
                        ),
                    ),
                )
                .with_context("access_bytes", &access_bytes.to_string())
                .with_context("base_bytes", &base_bytes.to_string()),
            );
            return;
        }

        let max_index = base_bytes / access_bytes - 1;
        let index = node.index.value;
        if index > max_index {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::BitAccessOutOfRange,
                    Label::span(
                        node.index.span(),
                        format!(
                            "Partial access index {} is out of range. Valid range is 0..{} for type",
                            index, max_index,
                        ),
                    ),
                )
                .with_context("index", &index.to_string())
                .with_context("max_index", &max_index.to_string()),
            );
        }
    }

    fn check_bit_access(&mut self, node: &BitAccessVariable) {
        // Resolve the type of the variable being bit-accessed
        let accessed_type =
            match variable_type::of(&node.variable, self.context, &self.scope.current()) {
                Some(t) => t,
                None => return,
            };

        let bit_width = match accessed_type.size_in_bytes() {
            Some(bytes) => bytes as u128 * 8,
            None => return,
        };

        let index = node.index.value;
        if index >= bit_width {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::BitAccessOutOfRange,
                    Label::span(
                        node.index.span(),
                        format!(
                            "Bit index {} is out of range. Valid range is 0..{} for type",
                            index,
                            bit_width - 1,
                        ),
                    ),
                )
                .with_context("index", &index.to_string())
                .with_context("max_bit", &(bit_width - 1).to_string()),
            );
        }
    }
}

impl Visitor<Infallible> for RuleBitAndPartialAccessRange<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
        node.recurse_visit(self)
    }

    fn visit_self_ref_variable(&mut self, node: &SelfRefVariable) -> Result<(), Infallible> {
        // Report rather than skip: a bit access through THIS^/SUPER^ cannot
        // be range-checked until member resolution exists, and staying
        // silent here would keep this rule quietly passing such a program
        // once the construct is otherwise supported. See issue #1406.
        self.diagnostics.push(Diagnostic::not_implemented(Label::span(
            node.span(),
            format!(
                "{} is recognized but its members are not yet resolved, so bit and partial access through it is not range-checked",
                node.kind.spelling()
            ),
        )));
        Ok(())
    }

    fn visit_bit_access_variable(&mut self, node: &BitAccessVariable) -> Result<(), Infallible> {
        self.check_bit_access(node);
        node.recurse_visit(self)
    }

    fn visit_partial_access_variable(
        &mut self,
        node: &PartialAccessVariable,
    ) -> Result<(), Infallible> {
        self.check_partial_access(node);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::parse_and_resolve_types_with_options;
    use ironplc_parser::options::CompilerOptions;
    use rstest::rstest;
    use spec_test_macro::spec_test;

    use super::*;

    /// No problems: the access is in range.
    const OK: &[Problem] = &[];
    /// The one problem an out-of-range access reports.
    const OUT_OF_RANGE: &[Problem] = &[Problem::BitAccessOutOfRange];

    /// The problem codes this rule reports for `program` under `opts`.
    fn problems_with(program: &str, opts: &CompilerOptions) -> Vec<String> {
        let (library, context) = parse_and_resolve_types_with_options(program, opts);
        match apply(&library, &context, opts) {
            Ok(()) => vec![],
            Err(diagnostics) => diagnostics.into_iter().map(|d| d.code).collect(),
        }
    }

    fn codes(problems: &[Problem]) -> Vec<&str> {
        problems.iter().map(|p| p.code()).collect()
    }

    fn assert_bit_access_ok(program: &str) {
        let codes = problems_with(program, &CompilerOptions::default());
        assert!(codes.is_empty(), "{codes:?}");
    }

    fn assert_bit_access_err(program: &str) {
        assert_eq!(
            problems_with(program, &CompilerOptions::default()),
            codes(OUT_OF_RANGE)
        );
    }

    // --- Bit access boundary tests across all bit-sized types ---
    //
    // For each type: the highest valid bit index is OK, and one past the
    // highest is an error. BYTE additionally covers bit 0 as the low bound.

    #[rstest]
    // BYTE (8 bits): valid range 0..7
    #[case::byte_bit_0("BYTE", 0, OK)]
    #[case::byte_bit_7("BYTE", 7, OK)]
    #[case::byte_bit_8("BYTE", 8, OUT_OF_RANGE)]
    // WORD (16 bits): valid range 0..15
    #[case::word_bit_15("WORD", 15, OK)]
    #[case::word_bit_16("WORD", 16, OUT_OF_RANGE)]
    // DWORD (32 bits): valid range 0..31
    #[case::dword_bit_31("DWORD", 31, OK)]
    #[case::dword_bit_32("DWORD", 32, OUT_OF_RANGE)]
    // LWORD (64 bits): valid range 0..63
    #[case::lword_bit_63("LWORD", 63, OK)]
    #[case::lword_bit_64("LWORD", 64, OUT_OF_RANGE)]
    // SINT (8 bits): valid range 0..7
    #[case::sint_bit_7("SINT", 7, OK)]
    #[case::sint_bit_8("SINT", 8, OUT_OF_RANGE)]
    // INT (16 bits): valid range 0..15
    #[case::int_bit_15("INT", 15, OK)]
    #[case::int_bit_16("INT", 16, OUT_OF_RANGE)]
    // DINT (32 bits): valid range 0..31
    #[case::dint_bit_31("DINT", 31, OK)]
    #[case::dint_bit_32("DINT", 32, OUT_OF_RANGE)]
    // LINT (64 bits): valid range 0..63
    #[case::lint_bit_63("LINT", 63, OK)]
    #[case::lint_bit_64("LINT", 64, OUT_OF_RANGE)]
    // USINT (8 bits): valid range 0..7
    #[case::usint_bit_7("USINT", 7, OK)]
    #[case::usint_bit_8("USINT", 8, OUT_OF_RANGE)]
    // UINT (16 bits): valid range 0..15
    #[case::uint_bit_15("UINT", 15, OK)]
    #[case::uint_bit_16("UINT", 16, OUT_OF_RANGE)]
    // UDINT (32 bits): valid range 0..31
    #[case::udint_bit_31("UDINT", 31, OK)]
    #[case::udint_bit_32("UDINT", 32, OUT_OF_RANGE)]
    // ULINT (64 bits): valid range 0..63
    #[case::ulint_bit_63("ULINT", 63, OK)]
    #[case::ulint_bit_64("ULINT", 64, OUT_OF_RANGE)]
    fn apply_when_bit_index_at_boundary_then_ok_or_err(
        #[case] type_name: &str,
        #[case] bit: u32,
        #[case] expected: &[Problem],
    ) {
        let program = format!(
            "FUNCTION_BLOCK FB1
VAR
    x : {type_name};
    y : BOOL;
END_VAR
    y := x.{bit};
END_FUNCTION_BLOCK"
        );
        assert_eq!(
            problems_with(&program, &CompilerOptions::default()),
            codes(expected)
        );
    }

    // --- Bit access on assignment target ---

    #[test]
    fn apply_when_bit_access_target_in_range_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    x : WORD;
    y : BOOL;
END_VAR
    x.0 := y;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_bit_access_target_out_of_range_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    x : BYTE;
    y : BOOL;
END_VAR
    x.8 := y;
END_FUNCTION_BLOCK",
        );
    }

    // --- Struct field bit access ---

    #[test]
    fn apply_when_struct_field_bit_in_range_then_ok() {
        assert_bit_access_ok(
            "TYPE
    MyStruct : STRUCT
        field1 : BYTE;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB1
VAR
    s : MyStruct;
    y : BOOL;
END_VAR
    y := s.field1.7;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_struct_field_bit_out_of_range_then_err() {
        assert_bit_access_err(
            "TYPE
    MyStruct : STRUCT
        field1 : BYTE;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB1
VAR
    s : MyStruct;
    y : BOOL;
END_VAR
    y := s.field1.8;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_struct_word_field_bit_in_range_then_ok() {
        assert_bit_access_ok(
            "TYPE
    MyStruct : STRUCT
        field1 : WORD;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB1
VAR
    s : MyStruct;
    y : BOOL;
END_VAR
    y := s.field1.15;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_struct_word_field_bit_out_of_range_then_err() {
        assert_bit_access_err(
            "TYPE
    MyStruct : STRUCT
        field1 : WORD;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB1
VAR
    s : MyStruct;
    y : BOOL;
END_VAR
    y := s.field1.16;
END_FUNCTION_BLOCK",
        );
    }

    // --- Array element bit access ---

    #[test]
    fn apply_when_array_element_bit_in_range_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    arr : ARRAY [0..3] OF BYTE;
    y : BOOL;
END_VAR
    y := arr[0].7;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_array_element_bit_out_of_range_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    arr : ARRAY [0..3] OF BYTE;
    y : BOOL;
END_VAR
    y := arr[0].8;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_array_word_element_bit_in_range_then_ok() {
        assert_bit_access_ok(
            "FUNCTION_BLOCK FB1
VAR
    arr : ARRAY [0..3] OF WORD;
    y : BOOL;
END_VAR
    y := arr[1].15;
END_FUNCTION_BLOCK",
        );
    }

    #[test]
    fn apply_when_array_word_element_bit_out_of_range_then_err() {
        assert_bit_access_err(
            "FUNCTION_BLOCK FB1
VAR
    arr : ARRAY [0..3] OF WORD;
    y : BOOL;
END_VAR
    y := arr[1].16;
END_FUNCTION_BLOCK",
        );
    }

    // --- Bit access in FUNCTION (not FUNCTION_BLOCK) ---

    #[test]
    fn apply_when_function_dint_bit_access_then_ok() {
        assert_bit_access_ok(
            "FUNCTION FOO : INT
VAR_INPUT
    A : DINT;
END_VAR
    IF A.0 THEN
        FOO := 1;
    END_IF;
END_FUNCTION

PROGRAM test_bit_func
VAR
    result : INT;
END_VAR
    result := FOO(A := 5);
END_PROGRAM",
        );
    }

    // --- Declarations outside the POU body ---
    //
    // The rule resolves a name against every scope that is open, not just
    // the enclosing POU's own variables, so a global and a method local are
    // both checkable.

    #[test]
    fn apply_when_global_bit_out_of_range_then_err() {
        assert_bit_access_err(
            "PROGRAM main
VAR
    y : BOOL;
END_VAR
    y := g.8;
END_PROGRAM

CONFIGURATION config
VAR_GLOBAL
    g : BYTE;
END_VAR
RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM inst WITH plc_task : main;
END_RESOURCE
END_CONFIGURATION",
        );
    }

    #[test]
    fn apply_when_method_local_bit_out_of_range_then_err() {
        let opts = CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        };
        let program = "FUNCTION_BLOCK FB_Motor
VAR
    y : BOOL;
END_VAR
METHOD Start
VAR
    local : BYTE;
END_VAR
    y := local.8;
END_METHOD
END_FUNCTION_BLOCK";
        assert_eq!(problems_with(program, &opts), codes(OUT_OF_RANGE));
    }

    // --- Partial access: byte, word, dword and lword slices ---
    //
    // The `%` selectors need `allow_partial_access_syntax`, so these build
    // their own options rather than using the helpers above.

    /// The problem codes this rule reports for `program` with partial-access
    /// syntax enabled.
    fn partial_access_problems(program: &str) -> Vec<String> {
        let opts = CompilerOptions {
            allow_partial_access_syntax: true,
            ..CompilerOptions::default()
        };
        problems_with(program, &opts)
    }

    fn partial_access_program(declared_type: &str, target_type: &str, selector: &str) -> String {
        format!(
            "FUNCTION_BLOCK FB1
VAR
    x : {declared_type};
    y : {target_type};
END_VAR
    y := x.{selector};
END_FUNCTION_BLOCK"
        )
    }

    /// REQ-PAB-analyzer-122: the valid index range is
    /// `0..(base_bytes / slice_bytes - 1)`.
    #[spec_test(REQ_PAB_analyzer_122)]
    #[rstest]
    // A WORD holds two bytes, so byte 0 and byte 1 exist and byte 2 does not.
    #[case::word_byte_0("WORD", "BYTE", "%B0", OK)]
    #[case::word_byte_1("WORD", "BYTE", "%B1", OK)]
    #[case::word_byte_2("WORD", "BYTE", "%B2", OUT_OF_RANGE)]
    // A DWORD holds four bytes and two words.
    #[case::dword_byte_3("DWORD", "BYTE", "%B3", OK)]
    #[case::dword_byte_4("DWORD", "BYTE", "%B4", OUT_OF_RANGE)]
    #[case::dword_word_1("DWORD", "WORD", "%W1", OK)]
    #[case::dword_word_2("DWORD", "WORD", "%W2", OUT_OF_RANGE)]
    fn apply_when_partial_access_index_at_boundary_then_ok_or_err(
        #[case] declared_type: &str,
        #[case] target_type: &str,
        #[case] selector: &str,
        #[case] expected: &[Problem],
    ) {
        let program = partial_access_program(declared_type, target_type, selector);

        assert_eq!(partial_access_problems(&program), codes(expected));
    }

    /// REQ-PAB-analyzer-121: a slice wider than the variable is rejected.
    #[spec_test(REQ_PAB_analyzer_121)]
    #[rstest]
    // A slice wider than the variable cannot be taken from it, whatever the
    // index -- a distinct failure from an index past the last slice.
    #[case::word_from_byte("BYTE", "WORD", "%W0")]
    #[case::dword_from_word("WORD", "DWORD", "%D0")]
    #[case::lword_from_dword("DWORD", "LWORD", "%L0")]
    fn apply_when_partial_access_wider_than_variable_then_err(
        #[case] declared_type: &str,
        #[case] target_type: &str,
        #[case] selector: &str,
    ) {
        let program = partial_access_program(declared_type, target_type, selector);

        assert_eq!(partial_access_problems(&program), codes(OUT_OF_RANGE));
    }

    // ---------------------------------------------------------------------
    // REQ-PAB-analyzer-030: the bit-range analyzer applies to .%Xn identically to .n.
    // See specs/design/partial-access-bit-syntax.md.
    // ---------------------------------------------------------------------

    /// REQ-PAB-analyzer-030: `b.%X8` on a BYTE is rejected (bit 8 out of range).
    #[spec_test(REQ_PAB_analyzer_030)]
    fn analyzer_spec_req_pab_030_dot_percent_x_bit_out_of_range_is_rejected() {
        let opts = CompilerOptions {
            allow_partial_access_syntax: true,
            ..CompilerOptions::default()
        };
        let program = "FUNCTION_BLOCK FB1
VAR
    b : BYTE;
    y : BOOL;
END_VAR
    y := b.%X8;
END_FUNCTION_BLOCK";
        assert_eq!(problems_with(program, &opts), codes(OUT_OF_RANGE));
    }
}
