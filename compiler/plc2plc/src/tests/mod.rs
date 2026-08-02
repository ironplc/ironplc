//! Tests of the renderer, split into focused modules to keep merge
//! conflicts local. Shared imports and helpers live in `common`; each
//! feature area has its own file. Adding tests for a new feature area =
//! a new file here plus one `mod` line.

mod common;

mod case;
mod constant_initializers;
mod corpus;
mod declarations;
mod enums;
mod mixed_vars;
mod partial_access;
mod reference_to;
mod short_circuit;
mod struct_init_expressions;
mod time_and_sizeof;
