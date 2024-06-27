//! Preprocessor for IEC 61131-3 language elements. The preprocessor transforms
//! the input text into a form that can be easily parsed.
//!
//! The preprocessor:
//! * removes OSCAT comments
//!
//! Comments are replaced by whitespace so that language elements retain their
//! original position (this means that source locations remain correct even
//! after comments are removed).

pub fn preprocess(source: &str) -> String {
    let source = source.to_string();
    remove_oscat_comment(source)
}

/// Removes the OSCAT ranged comment. This is not valid IEC 61131, but there
/// are enough of these that it is worthwhile.
pub fn remove_oscat_comment(source: String) -> String {
    let len_key = 21; // The length of "(*@KEY@:DESCRIPTION*)"
    if let Some(start) = source.find("(*@KEY@:DESCRIPTION*)") {
        if let Some(end) = source.find("(*@KEY@:END_DESCRIPTION*)") {
            if start < end {
                let prelude = &source[0..start + len_key];
                let epilog = &source[end..source.len()];

                let mut output = String::with_capacity(source.len());
                output.push_str(prelude);

                // Replace the comment internally character-by-character
                // so that we retain the exact same positions
                for c in source[start + len_key..end].chars() {
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

        let output = preprocess(program);
        assert_eq!(program, output.as_str());
    }

    #[test]
    fn apply_when_oscat_comment_then_removes() {
        let program = "
TYPE
    (*@KEY@:DESCRIPTION*)
    any text
    (*@KEY@:END_DESCRIPTION*)
END_TYPE";

        // The whitespace here matters in order to keep the same
        // character positions
        let expected = "
TYPE
    (*@KEY@:DESCRIPTION*)
            
    (*@KEY@:END_DESCRIPTION*)
END_TYPE";

        let output = preprocess(program);
        assert_eq!(expected, output.as_str());
    }

    #[test]
    fn apply_when_open_oscat_comment_then_ok() {
        // Handle an open OSCAT key comment as a regular comment
        let program = "
        TYPE
            (*@KEY@:DESCRIPTION*)
        END_TYPE";

        let res = preprocess(program);
        assert!(res.len() > 0);
    }
}
