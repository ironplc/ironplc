//! Preprocessor for IEC 61131-3 language elements. The preprocessor transforms
//! the input text into a form that can be easily parsed.
//!
//! The preprocessor:
//! * removes comments
//!
//! Comments are replaced by whitespace so that language elements retain their
//! original position (this means that source locations remain correct even
//! after comments are removed).

use dsl::{core::FileId, diagnostic::Label};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_problems::Problem;

pub fn preprocess(source: &str, file_id: &FileId) -> Result<String, Diagnostic> {
    let source = remove_oscat_comment(source.to_string());
    remove_standard_comment(&source, file_id)
}

/// Removes the OSCAT ranged comment. This is not valid IEC 61131, but there
/// are enough of these that it is worthwhile.
pub fn remove_oscat_comment(source: String) -> String {
    if let Some(start) = source.find("(*@KEY@:DESCRIPTION*)") {
        if let Some(end) = source.find("(*@KEY@:END_DESCRIPTION*)") {
            if start < end {
                let prelude = &source[0..start];
                let epilog = &source[end..source.len()];

                let mut output = String::with_capacity(source.len());
                output.push_str(prelude);

                // Replace the comment internally character-by-character
                // so that we retain the exact same positions
                for c in source[start..end].chars() {
                    if c == '\n' {
                        output.push('\n');
                    } else {
                        output.push(' ');
                    }
                }

                output.push_str(epilog);
                return output;
            }
        }
    }
    source
}

pub fn remove_standard_comment(source: &str, file_id: &FileId) -> Result<String, Diagnostic> {
    // True when currently in a comment block, otherwise false.
    let mut in_comment = false;
    // True when the prior character is a candidate for starting or ending a
    // comment block otherwise, false.
    let mut last_is_comment_candidate = false;

    let mut output = String::with_capacity(source.len());

    for char in source.chars() {
        if in_comment {
            if last_is_comment_candidate && char == ')' {
                // This is the end of a comment, update our simple state
                in_comment = false;
                last_is_comment_candidate = false;
            } else {
                last_is_comment_candidate = char == '*';
            }

            // We want to retain new line characters so that
            // line numbers remain the same.
            if char == '\n' {
                output.push('\n');
            } else {
                output.push(' ');
            }
        } else if last_is_comment_candidate && char == '*' {
            // We have started a comment - there is a character written
            // that was actually the start of a comment so replace it
            output.pop();
            output.push(' ');
            // Set our state as being in a comment
            in_comment = true;
            last_is_comment_candidate = false;
            output.push(' ');
        } else {
            // Just write the character
            last_is_comment_candidate = char == '(';
            output.push(char)
        }
    }

    // By the very end, we should no longer be in a comment. If we are, that's
    // an error
    if in_comment {
        return Err(Diagnostic::problem(
            Problem::OpenComment,
            Label::offset(
                file_id.clone(),
                source.len()..source.len(),
                "Expected '*)' - end of comment",
            ),
        ));
    }
    Ok(output.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_when_no_comment_then_ok() {
        let program = "
        TYPE
            CUSTOM_STRUCT : STRUCT 
                NAME: BOOL;
            END_STRUCT;
        END_TYPE";

        let output = preprocess(program, &FileId::default()).unwrap();
        assert_eq!(program, output.as_str());
    }

    #[test]
    fn apply_when_one_comment_then_removes_and_ok() {
        let program = "
        TYPE
        (* A comment *)
            CUSTOM_STRUCT : STRUCT 
                NAME: BOOL;
            END_STRUCT;
        END_TYPE";

        let expected = "
        TYPE
                       
            CUSTOM_STRUCT : STRUCT 
                NAME: BOOL;
            END_STRUCT;
        END_TYPE";

        let output = preprocess(program, &FileId::default()).unwrap();
        assert_eq!(expected, output.as_str());
    }

    #[test]
    fn apply_when_back_to_back_then_removes_and_ok() {
        let program = "
        TYPE
        (* A comment *)(* A comment *)
        END_TYPE";

        let expected = "
        TYPE
                                      
        END_TYPE";

        let output = preprocess(program, &FileId::default()).unwrap();
        assert_eq!(expected, output.as_str());
    }

    #[test]
    fn apply_when_not_closed_then_error() {
        let program = "
        TYPE
        (* A comment
            CUSTOM_STRUCT : STRUCT 
                NAME: BOOL;
            END_STRUCT;
        END_TYPE";

        assert!(preprocess(program, &FileId::default()).is_err());
    }
}
