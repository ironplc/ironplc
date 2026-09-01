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
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    variable_type::{self, DeclaredVariables},
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleBitAndPartialAccessRange {
            type_environment: context.types(),
            declarations: DeclaredVariables::new(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleBitAndPartialAccessRange<'a> {
    type_environment: &'a crate::type_environment::TypeEnvironment,
    /// The variable declarations visible in the POU being visited.
    declarations: DeclaredVariables,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleBitAndPartialAccessRange<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleBitAndPartialAccessRange<'_> {
    fn check_partial_access(&mut self, node: &PartialAccessVariable) {
        let resolved_type =
            match variable_type::of(&node.variable, &self.declarations, self.type_environment) {
                Some(t) => t,
                None => return,
            };

        let base_bytes = match resolved_type.size_in_bytes() {
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
        let resolved_type =
            match variable_type::of(&node.variable, &self.declarations, self.type_environment) {
                Some(t) => t,
                None => return,
            };

        let bit_width = match resolved_type.size_in_bytes() {
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

    fn visit_function_declaration(&mut self, node: &FunctionDeclaration) -> Result<(), Infallible> {
        self.declarations.enter_pou(&node.variables);
        let ret = node.recurse_visit(self);
        self.declarations.exit_pou();
        ret
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Infallible> {
        self.declarations.enter_pou(&node.variables);
        let ret = node.recurse_visit(self);
        self.declarations.exit_pou();
        ret
    }

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<(), Infallible> {
        self.declarations.enter_pou(&node.variables);
        let ret = node.recurse_visit(self);
        self.declarations.exit_pou();
        ret
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
    use crate::stages::analyze;
    use crate::test_helpers::parse_and_resolve_types_with_context;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::CompilerOptions, parse_program};
    use rstest::rstest;

    use super::*;

    fn assert_bit_access_ok(program: &str) {
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok(), "Expected OK but got: {:?}", result);
    }

    fn assert_bit_access_err(program: &str) {
        let library =
            parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
        let result = analyze(&[&library], &CompilerOptions::default());
        let (_library, context) = result.unwrap();
        assert!(
            context.has_diagnostics(),
            "Expected diagnostics but got none"
        );
    }

    // --- Bit access boundary tests across all bit-sized types ---
    //
    // For each type: the highest valid bit index is OK, and one past the
    // highest is an error. BYTE additionally covers bit 0 as the low bound.

    #[rstest]
    // BYTE (8 bits): valid range 0..7
    #[case::byte_bit_0("BYTE", 0, true)]
    #[case::byte_bit_7("BYTE", 7, true)]
    #[case::byte_bit_8("BYTE", 8, false)]
    // WORD (16 bits): valid range 0..15
    #[case::word_bit_15("WORD", 15, true)]
    #[case::word_bit_16("WORD", 16, false)]
    // DWORD (32 bits): valid range 0..31
    #[case::dword_bit_31("DWORD", 31, true)]
    #[case::dword_bit_32("DWORD", 32, false)]
    // LWORD (64 bits): valid range 0..63
    #[case::lword_bit_63("LWORD", 63, true)]
    #[case::lword_bit_64("LWORD", 64, false)]
    // SINT (8 bits): valid range 0..7
    #[case::sint_bit_7("SINT", 7, true)]
    #[case::sint_bit_8("SINT", 8, false)]
    // INT (16 bits): valid range 0..15
    #[case::int_bit_15("INT", 15, true)]
    #[case::int_bit_16("INT", 16, false)]
    // DINT (32 bits): valid range 0..31
    #[case::dint_bit_31("DINT", 31, true)]
    #[case::dint_bit_32("DINT", 32, false)]
    // LINT (64 bits): valid range 0..63
    #[case::lint_bit_63("LINT", 63, true)]
    #[case::lint_bit_64("LINT", 64, false)]
    // USINT (8 bits): valid range 0..7
    #[case::usint_bit_7("USINT", 7, true)]
    #[case::usint_bit_8("USINT", 8, false)]
    // UINT (16 bits): valid range 0..15
    #[case::uint_bit_15("UINT", 15, true)]
    #[case::uint_bit_16("UINT", 16, false)]
    // UDINT (32 bits): valid range 0..31
    #[case::udint_bit_31("UDINT", 31, true)]
    #[case::udint_bit_32("UDINT", 32, false)]
    // ULINT (64 bits): valid range 0..63
    #[case::ulint_bit_63("ULINT", 63, true)]
    #[case::ulint_bit_64("ULINT", 64, false)]
    fn apply_when_bit_index_at_boundary_then_ok_or_err(
        #[case] type_name: &str,
        #[case] bit: u32,
        #[case] expected_ok: bool,
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
        if expected_ok {
            assert_bit_access_ok(&program);
        } else {
            assert_bit_access_err(&program);
        }
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

    // --- Partial access: byte, word, dword and lword slices ---
    //
    // The `%` selectors need `allow_partial_access_syntax`, so these build
    // their own options rather than using the helpers above.

    /// Analyzes `program` with partial-access syntax enabled, returning
    /// whether this rule reported the access as out of range. Naming the
    /// problem keeps a diagnostic from some other rule from passing for one
    /// of ours.
    fn reports_out_of_range_with_partial_access(program: &str) -> bool {
        let opts = CompilerOptions {
            allow_partial_access_syntax: true,
            ..CompilerOptions::default()
        };
        let library = parse_program(program, &FileId::default(), &opts).unwrap();
        let (_library, context) = analyze(&[&library], &opts).unwrap();
        context
            .diagnostics()
            .iter()
            .any(|d| d.code == Problem::BitAccessOutOfRange.code())
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

    #[rstest]
    // A WORD holds two bytes, so byte 0 and byte 1 exist and byte 2 does not.
    #[case::word_byte_0("WORD", "BYTE", "%B0", true)]
    #[case::word_byte_1("WORD", "BYTE", "%B1", true)]
    #[case::word_byte_2("WORD", "BYTE", "%B2", false)]
    // A DWORD holds four bytes and two words.
    #[case::dword_byte_3("DWORD", "BYTE", "%B3", true)]
    #[case::dword_byte_4("DWORD", "BYTE", "%B4", false)]
    #[case::dword_word_1("DWORD", "WORD", "%W1", true)]
    #[case::dword_word_2("DWORD", "WORD", "%W2", false)]
    fn apply_when_partial_access_index_at_boundary_then_ok_or_err(
        #[case] declared_type: &str,
        #[case] target_type: &str,
        #[case] selector: &str,
        #[case] expected_ok: bool,
    ) {
        let program = partial_access_program(declared_type, target_type, selector);

        assert_eq!(
            !reports_out_of_range_with_partial_access(&program),
            expected_ok
        );
    }

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

        assert!(reports_out_of_range_with_partial_access(&program));
    }

    // ---------------------------------------------------------------------
    // REQ-PAB-030: the bit-range analyzer applies to .%Xn identically to .n.
    // See specs/design/partial-access-bit-syntax.md.
    // ---------------------------------------------------------------------

    /// REQ-PAB-030: `b.%X8` on a BYTE is rejected (bit 8 out of range).
    #[test]
    fn analyzer_spec_req_pab_030_dot_percent_x_bit_out_of_range_is_rejected() {
        use ironplc_parser::options::CompilerOptions;
        use ironplc_parser::parse_program;

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
        let library = parse_program(program, &FileId::default(), &opts).unwrap();
        let result = analyze(&[&library], &opts);
        let (_library, context) = result.unwrap();
        assert!(
            context.has_diagnostics(),
            "Expected BitAccessOutOfRange diagnostic but got none"
        );
    }
}
