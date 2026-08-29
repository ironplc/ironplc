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

/// Spec-conformance requirements generated from `specs/design/runtime-execution-model.md`.
/// Referenced by `#[spec_test(REQ_RT_vm_NNN)]`. See `vm/build.rs`. Every item the
/// build script emits carries its own targeted `#[allow(dead_code)]`, so no
/// module-level blanket allow is needed.
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}

/// Meta-test: every `REQ-RT-vm-NNN` requirement in
/// `specs/design/runtime-execution-model.md` has a `#[spec_test(...)]`
/// somewhere in this crate's `src/` or `tests/`. The build script populates
/// `UNTESTED` from the files it scans.
#[test]
fn all_spec_requirements_have_tests() {
    assert!(
        spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        spec_requirements::UNTESTED
    );
}

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
mod execute_builtin_trunc_mod_f32;
mod execute_builtin_trunc_mod_f64;
mod execute_call_ret;
mod execute_cmp_i32;
mod execute_copy_region;
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
