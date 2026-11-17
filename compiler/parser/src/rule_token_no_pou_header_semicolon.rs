//! Validation rule: reject a `;` that closes a `PROGRAM`, `FUNCTION`,
//! `FUNCTION_BLOCK` or `METHOD` header before its first VAR block unless the
//! `allow_pou_header_semicolon` flag is set.
//!
//! IEC 61131-3 (Annex B) puts no terminator after a POU header: the VAR
//! blocks follow the name (and the return type, `EXTENDS` and `IMPLEMENTS`)
//! directly. TwinCAT accepts `FUNCTION_BLOCK FB EXTENDS FB_Base;` and treats
//! the `;` as if it were absent, so the `;` is an extension, accepted under
//! `--allow-pou-header-semicolon`. Which dialects enable it is defined by the
//! dialect mapping in `options.rs`, not restated here.
//!
//! The grammar accepts the `;` unconditionally (see `pou_header_semicolon()`),
//! so this token-stream check is what enforces the flag. The first `;` after
//! the POU keyword is the header's `;` when the next significant token opens
//! a VAR block. A `;` followed by anything else is the standard empty
//! statement at the start of the body and is always allowed.
//!
//! Inside `CONFIGURATION` ... `END_CONFIGURATION`, `PROGRAM` starts a program
//! configuration (`PROGRAM inst : Prog;`), which a `VAR_ACCESS` or
//! `VAR_CONFIG` block may follow, so `PROGRAM` there is not a POU header.
//!
//! The set of tokens skipped here matches the grammar's whitespace rule
//! `_ = (whitespace() / comment() / pragma())*`, so a `;` separated from the
//! VAR keyword by a comment is found the same way as one written next to it.

use dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

use crate::{
    options::CompilerOptions,
    token::{Token, TokenType},
};

fn is_trivia(t: &TokenType) -> bool {
    matches!(
        t,
        TokenType::Whitespace | TokenType::Newline | TokenType::Comment | TokenType::Pragma
    )
}

/// Matches the grammar's `var_block_keyword()`.
fn is_var_block_keyword(t: &TokenType) -> bool {
    matches!(
        t,
        TokenType::Var
            | TokenType::VarInput
            | TokenType::VarOutput
            | TokenType::VarInOut
            | TokenType::VarTemp
            | TokenType::VarExternal
            | TokenType::VarAccess
            | TokenType::VarGlobal
            | TokenType::VarConfig
    )
}

pub fn apply(tokens: &[Token], options: &CompilerOptions) -> Result<(), Vec<Diagnostic>> {
    if options.allow_pou_header_semicolon {
        return Ok(());
    }

    let significant: Vec<&Token> = tokens
        .iter()
        .filter(|t| !is_trivia(&t.token_type))
        .collect();

    let mut errors = vec![];
    let mut in_configuration = false;
    let mut in_header = false;
    for (i, tok) in significant.iter().enumerate() {
        match tok.token_type {
            TokenType::Configuration => in_configuration = true,
            TokenType::EndConfiguration => in_configuration = false,
            TokenType::Program if !in_configuration => in_header = true,
            TokenType::Function | TokenType::FunctionBlock | TokenType::Method => in_header = true,
            TokenType::Semicolon if in_header => {
                in_header = false;
                let next = significant.get(i + 1);
                if next.is_some_and(|n| is_var_block_keyword(&n.token_type)) {
                    errors.push(Diagnostic::problem(
                        Problem::PouHeaderSemicolonNotAllowed,
                        Label::span(tok.span.clone(), "Semicolon after POU header"),
                    ));
                }
            }
            ref t if is_var_block_keyword(t) => in_header = false,
            _ => {}
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod test {
    use crate::test_rule_helpers::token;
    use ironplc_problems::Problem;

    use crate::{
        options::CompilerOptions,
        token::{Token, TokenType},
    };

    /// `<keyword> X ; (* c *) VAR_INPUT`
    fn header_semicolon_tokens(keyword: TokenType) -> Vec<Token> {
        vec![
            token(keyword, "KEYWORD"),
            token(TokenType::Whitespace, " "),
            token(TokenType::Identifier, "X"),
            token(TokenType::Semicolon, ";"),
            token(TokenType::Whitespace, " "),
            token(TokenType::Comment, "(* c *)"),
            token(TokenType::Newline, "\n"),
            token(TokenType::VarInput, "VAR_INPUT"),
        ]
    }

    token_rule_err!(
        apply_when_function_block_header_semicolon_and_flag_off_then_error,
        header_semicolon_tokens(TokenType::FunctionBlock),
        [Problem::PouHeaderSemicolonNotAllowed]
    );

    token_rule_err!(
        apply_when_program_header_semicolon_and_flag_off_then_error,
        header_semicolon_tokens(TokenType::Program),
        [Problem::PouHeaderSemicolonNotAllowed]
    );

    token_rule_err!(
        apply_when_function_header_semicolon_and_flag_off_then_error,
        header_semicolon_tokens(TokenType::Function),
        [Problem::PouHeaderSemicolonNotAllowed]
    );

    token_rule_err!(
        apply_when_method_header_semicolon_and_flag_off_then_error,
        header_semicolon_tokens(TokenType::Method),
        [Problem::PouHeaderSemicolonNotAllowed]
    );

    token_rule_ok!(
        apply_when_header_semicolon_and_flag_on_then_ok,
        header_semicolon_tokens(TokenType::FunctionBlock),
        CompilerOptions {
            allow_pou_header_semicolon: true,
            ..CompilerOptions::default()
        }
    );

    // `PROGRAM P ; x := 1;` -- the `;` is the standard empty statement.
    token_rule_ok!(
        apply_when_header_semicolon_before_statement_then_ok,
        vec![
            token(TokenType::Program, "PROGRAM"),
            token(TokenType::Identifier, "P"),
            token(TokenType::Semicolon, ";"),
            token(TokenType::Identifier, "x"),
            token(TokenType::Assignment, ":="),
            token(TokenType::Digits, "1"),
            token(TokenType::Semicolon, ";"),
        ]
    );

    // `FUNCTION_BLOCK FB VAR END_VAR ; VAR_INPUT` -- the header ended at the
    // first VAR block, so a later `;` is not the header's.
    token_rule_ok!(
        apply_when_semicolon_after_var_block_then_ok,
        vec![
            token(TokenType::FunctionBlock, "FUNCTION_BLOCK"),
            token(TokenType::Identifier, "FB"),
            token(TokenType::Var, "VAR"),
            token(TokenType::Identifier, "a"),
            token(TokenType::Colon, ":"),
            token(TokenType::Identifier, "INT"),
            token(TokenType::Semicolon, ";"),
            token(TokenType::EndVar, "END_VAR"),
            token(TokenType::Semicolon, ";"),
            token(TokenType::VarInput, "VAR_INPUT"),
        ]
    );

    // `CONFIGURATION C ... PROGRAM inst : P; VAR_ACCESS` -- a program
    // configuration, not a POU header.
    token_rule_ok!(
        apply_when_program_configuration_before_var_access_then_ok,
        vec![
            token(TokenType::Configuration, "CONFIGURATION"),
            token(TokenType::Identifier, "C"),
            token(TokenType::Program, "PROGRAM"),
            token(TokenType::Identifier, "inst"),
            token(TokenType::Colon, ":"),
            token(TokenType::Identifier, "P"),
            token(TokenType::Semicolon, ";"),
            token(TokenType::VarAccess, "VAR_ACCESS"),
        ]
    );

    // After `END_CONFIGURATION`, `PROGRAM` starts a POU header again.
    token_rule_err!(
        apply_when_header_semicolon_after_configuration_then_error,
        {
            let mut tokens = vec![
                token(TokenType::Configuration, "CONFIGURATION"),
                token(TokenType::EndConfiguration, "END_CONFIGURATION"),
            ];
            tokens.extend(header_semicolon_tokens(TokenType::Program));
            tokens
        },
        [Problem::PouHeaderSemicolonNotAllowed]
    );
}
