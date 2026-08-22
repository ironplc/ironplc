//! Tests of the parser, split into focused modules to keep merge
//! conflicts local. Shared imports and helpers live in `common`; each
//! feature area has its own file. Adding tests for a new feature area =
//! a new file here plus one `mod` line.

mod common;

mod arrays;
mod case;
mod comments_and_errors;
mod constant_initializers;
mod corpus;
mod dialect_flags;
mod duration;
mod enums;
mod fb_inheritance;
mod function_calls;
mod literals;
mod methods;
mod partial_access;
mod pointer_to;
mod pragmas;
mod reference_to;
mod short_circuit;
mod struct_init_expressions;
mod tasks;
mod this_super;
mod time_functions;
mod types_and_returns;
mod var_declarations;
