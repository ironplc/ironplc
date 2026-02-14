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

/// Removes OSCAT ranged comments. These are not valid IEC 61131-3 but are
/// common enough that it is worthwhile to handle them. The pattern is
/// `(*@KEY@:NAME*)` ... `(*@KEY@:END_NAME*)` for any uppercase name.
pub fn remove_oscat_comment(source: String) -> String {
    let prefix = "(*@KEY@:";
    let suffix = "*)";

    let Some(prefix_start) = source.find(prefix) else {
        return source;
    };

    let after_prefix = prefix_start + prefix.len();

    // Find the closing *) for the opening marker
    let Some(suffix_offset) = source[after_prefix..].find(suffix) else {
        return source;
    };

    let name = &source[after_prefix..after_prefix + suffix_offset];

    // The name must not start with END_ (that would be a closing marker)
    if name.starts_with("END_") {
        return source;
    }

    let open_end = after_prefix + suffix_offset + suffix.len();

    // Build the expected closing marker: (*@KEY@:END_NAME*)
    let close_marker = format!("{}END_{}{}", prefix, name, suffix);

    let Some(close_start) = source[open_end..].find(&close_marker) else {
        return source;
    };
    let close_start = open_end + close_start;

    let mut output = String::with_capacity(source.len());
    output.push_str(&source[..open_end]);

    // Replace the comment internally character-by-character
    // so that we retain the exact same positions
    for c in source[open_end..close_start].chars() {
        if c == '\n' {
            output.push('\n');
        } else {
            output.push(' ');
        }
    }

    output.push_str(&source[close_start..]);
    output
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
        assert!(!res.is_empty());
    }

    #[test]
    fn apply_when_oscat_worksheet_comment_then_removes() {
        let program = "
TYPE
    (*@KEY@:WORKSHEET*)
    any text
    (*@KEY@:END_WORKSHEET*)
END_TYPE";

        // The whitespace here matters in order to keep the same
        // character positions. The blank line has 12 spaces replacing
        // "    any text".
        let expected = concat!(
            "\nTYPE\n",
            "    (*@KEY@:WORKSHEET*)\n",
            "            \n",
            "    (*@KEY@:END_WORKSHEET*)\n",
            "END_TYPE"
        );

        let output = preprocess(program);
        assert_eq!(expected, output.as_str());
    }
}
