//! Single integration-test binary for `ironplc-vm`.
//!
//! Each former top-level test file under `tests/` is a submodule of this
//! binary. Consolidating into one binary cuts link time and `target/` size:
//! instead of linking the whole dependency graph (and llvm-cov instrumentation)
//! once per file, we link it once.
//!
//! Wired up as a single test target via `[[test]]` in `vm/Cargo.toml` so
//! `main.rs` is the crate root — that lets `mod foo;` resolve to `it/foo.rs`
//! without `#[path]` attributes on every declaration.

mod common;

mod debug_engine;
mod execute_add_i32;
mod execute_arith_f32;
mod execute_arith_f64;
mod execute_array_ops;
mod execute_bitwise;
mod execute_bool;
mod execute_bool_literal;
mod execute_builtin_abs_i32;
mod execute_builtin_abs_i64;
mod execute_builtin_expt_i32;
mod execute_call_ret;
mod execute_cmp_i32;
mod execute_data_region_oob;
mod execute_div_i32;
mod execute_dup_swap;
mod execute_fb_ops;
mod execute_fb_tof;
mod execute_fb_ton;
mod execute_fb_tp;
mod execute_if;
mod execute_indirect;
mod execute_loops;
mod execute_mod_i32;
mod execute_mul_i32;
mod execute_neg_i32;
mod execute_stack_overflow;
mod execute_string_ops;
mod execute_sub_i32;
mod load_max_call_depth;
mod profiling;
mod proptest_robustness;
mod scenarios;
mod steel_thread;
