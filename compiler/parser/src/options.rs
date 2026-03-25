//! Options affecting parsing.
//!
//!
//!

#[derive(Debug, Default, Clone, Copy)]
pub struct ParseOptions {
    pub allow_c_style_comments: bool,
    pub allow_iec_61131_3_2013: bool,
    pub allow_missing_semicolon: bool,
    pub allow_top_level_var_global: bool,
    pub allow_constant_type_params: bool,
    pub allow_time_as_function_name: bool,
}
