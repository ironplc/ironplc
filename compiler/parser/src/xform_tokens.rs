use dsl::core::FileId;

use crate::options::CompilerOptions;
use crate::token::{Token, TokenType};

fn semicolon_like(tok: &Token) -> Token {
    Token {
        token_type: TokenType::Semicolon,
        span: tok.span.clone(),
        line: tok.line,
        col: tok.col,
        text: "".to_owned(),
    }
}

/// Adds a semicolon after keyword statements to terminate the statement, and
/// fills in a missing statement for a completely empty `CASE` branch.
///
/// IEC 61131-3 requires a semicolon after each statement but many programs
/// do not have a semicolon after named keywords. This function inserts the
/// semicolon token after keyword statements (when the semicolon does not
/// exist) so that the token stream is valid.
///
/// It also handles a `CASE` branch with no statements at all (a label that
/// falls straight through to the next label, `ELSE`, or `END_CASE` --
/// confirmed against real TwinCAT output). Strict IEC 61131-3 only allows
/// this via an explicit empty statement (`5: ;`); this inserts that `;`
/// when a branch is otherwise completely empty, turning `5:` into the
/// already-legal `5: ;` before the grammar ever sees it.
///
/// Both fixups are only applied when `options.allow_missing_semicolon` is set.
pub fn insert_keyword_statement_terminators(
    input: Vec<Token>,
    _file_id: &FileId,
    options: &CompilerOptions,
) -> Vec<Token> {
    if !options.allow_missing_semicolon {
        return input;
    }

    let mut output = Vec::new();

    let mut in_end_statement = false;

    // Tracks CASE...END_CASE nesting and, within it, whether we're still
    // waiting for the first statement of the current branch (right after a
    // case label's `:`, or after `ELSE`). While waiting, tokens that could
    // still be part of either an empty branch's case list (the next label)
    // or a real statement -- digits, identifiers, `,`, `..`, `#`, sign --
    // are buffered rather than emitted immediately, since we can't tell
    // them apart until we see whichever comes first: an unambiguous
    // statement token (`:=`, `(`, `.`, `[`, `^`, or a statement keyword) or
    // the next branch terminator (another `:`, `ELSE`, `END_CASE`).
    // Reaching a terminator while still waiting means the branch was empty.
    let mut case_depth: u32 = 0;
    let mut awaiting_case_branch_statement = false;
    let mut case_branch_buffer: Vec<Token> = Vec::new();

    for tok in input {
        match tok.token_type {
            TokenType::Case => {
                if awaiting_case_branch_statement {
                    output.append(&mut case_branch_buffer);
                    awaiting_case_branch_statement = false;
                }
                case_depth += 1;
                output.push(tok);
                continue;
            }
            TokenType::Colon if case_depth > 0 => {
                if awaiting_case_branch_statement {
                    output.push(semicolon_like(&tok));
                    output.append(&mut case_branch_buffer);
                }
                awaiting_case_branch_statement = true;
                output.push(tok);
                continue;
            }
            TokenType::EndCase if case_depth > 0 => {
                if awaiting_case_branch_statement {
                    output.push(semicolon_like(&tok));
                    output.append(&mut case_branch_buffer);
                    awaiting_case_branch_statement = false;
                }
                case_depth -= 1;
            }
            TokenType::Else if awaiting_case_branch_statement => {
                output.push(semicolon_like(&tok));
                output.append(&mut case_branch_buffer);
                awaiting_case_branch_statement = false;
            }
            TokenType::Semicolon if awaiting_case_branch_statement => {
                // The branch already has an explicit empty statement
                // (`5: ;`) -- nothing to insert, just stop waiting.
                output.append(&mut case_branch_buffer);
                awaiting_case_branch_statement = false;
            }
            TokenType::Assignment
            | TokenType::LeftParen
            | TokenType::LeftBracket
            | TokenType::Caret
            | TokenType::Period
            | TokenType::If
            | TokenType::For
            | TokenType::While
            | TokenType::Repeat
            | TokenType::Return
            | TokenType::Exit
                if awaiting_case_branch_statement =>
            {
                // Unambiguous start of a real statement.
                output.append(&mut case_branch_buffer);
                awaiting_case_branch_statement = false;
            }
            _ if awaiting_case_branch_statement => {
                case_branch_buffer.push(tok);
                continue;
            }
            _ => {}
        }

        // Insert a semicolon after a keyword statement terminator
        // (END_IF, END_CASE, ...) when the source omitted it.
        if !in_end_statement
            && matches!(
                tok.token_type,
                TokenType::EndIf
                    | TokenType::EndStruct
                    | TokenType::EndWhile
                    | TokenType::EndFor
                    | TokenType::EndCase
                    | TokenType::EndRepeat
            )
        {
            in_end_statement = true;
        } else if in_end_statement && tok.token_type == TokenType::Semicolon {
            // The source already has the semicolon — no insertion needed.
            in_end_statement = false;
        } else if in_end_statement
            && tok.token_type != TokenType::Comment
            && tok.token_type != TokenType::Whitespace
        {
            // TODO remove the span and line/col
            output.push(Token {
                token_type: TokenType::Semicolon,
                span: tok.span.clone(),
                line: tok.line,
                col: tok.col,
                text: "".to_owned(),
            });
            in_end_statement = false;
        }

        output.push(tok);
    }

    output
}
