//! Options affecting parsing.
//!
//!
//!

#[derive(Debug, Default, Clone, Copy)]
pub struct ParseOptions {
    pub allow_c_style_comments: bool,
    pub allow_iec_61131_3_2013: bool,
}
