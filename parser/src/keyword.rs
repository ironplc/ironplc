//! Language keywords.

use phf::{phf_set, Set};

static KEYWORDS: Set<&'static str> = phf_set! {
    "ACTION",
    "END_ACTION",
    "ARRAY",
    "OF",
    "AT",
    "CASE",
    "ELSE",
    "END_CASE",
    "CONFIGURATION",
    "END_CONFIGURATION",
    "CONSTANT",
    "EN",
    "ENO",
    "EXIT",
    "FALSE",
    "F_EDGE",
    "FOR",
    "TO",
    "BY",
    "DO",
    "END_FOR",
    "FUNCTION",
    "END_FUNCTION",
    "FUNCTION_BLOCK",
    "END_FUNCTION_BLOCK",
    "IF",
    "THEN",
    "ELSIF",
    "END_IF",
    "INITIAL_STEP",
    "END_STEP",
    "NOT",
    "MOD",
    "AND",
    "XOR",
    "OR",
    "PROGRAM",
    "END_PROGRAM",
    "R_EDGE",
    "READ_ONLY",
    "READ_WRITE",
    "REPEAT",
    "UNTIL",
    "END_REPEAT",
    "RESOURCE",
    "END_RESOURCE",
    "RETAIN",
    "NON_RETAIN",
    "RETURN",
    //"STEP",
    "STRUCT",
    "END_STRUCT",
    "TASK",
    "TRANSITION",
    "FROM",
    "END_TRANSITION",
    "TRUE",
    "VAR",
    "END_VAR",
    "VAR_INPUT",
    "VAR_OUTPUT",
    "VAR_IN_OUT",
    "VAR_TEMP",
    "VAR_EXTERNAL",
    "VAR_ACCESS",
    "VAR_CONFIG",
    "VAR_GLOBAL",
    "WHILE",
    "END_WHILE",
    "WITH",
    "PRIORITY",
    "STRING",
    "WSTRING"
};

pub fn get_keyword(input: &str) -> Option<&&str> {
    KEYWORDS.get_key(&input.to_uppercase())
}
