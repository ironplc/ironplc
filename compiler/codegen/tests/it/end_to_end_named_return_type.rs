//! End-to-end tests for a function or method whose return type is an alias
//! or a subrange: its value is returned at the type the alias names or the
//! subrange's base type, not at the default slot type.

use ironplc_parser::options::CompilerOptions;
use spec_test_macro::spec_test;

e2e_f64_with!(
    #[spec_test(REQ_IC_codegen_016)]
    end_to_end_when_return_type_is_alias_of_lreal_then_returns_lreal,
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    },
    "TYPE Precise : LREAL; END_TYPE
     FUNCTION keep : Precise
     VAR_INPUT x : Precise; END_VAR
       keep := x;
     END_FUNCTION
     FUNCTION_BLOCK K
     METHOD Half : Precise
       Half := 1.25;
     END_METHOD
     END_FUNCTION_BLOCK
     PROGRAM main
     VAR k : K; x : Precise := 2.5; from_call : LREAL; from_method : LREAL; END_VAR
       from_call := keep(x);
       from_method := k.Half();
     END_PROGRAM",
    &[("from_call", 2.5), ("from_method", 1.25)],
);

e2e_i64!(
    #[spec_test(REQ_IC_codegen_016)]
    end_to_end_when_return_type_is_alias_or_subrange_of_lint_then_returns_lint,
    "TYPE Big : LINT (0..10000000000); Wide : LINT; END_TYPE
     FUNCTION next : Big
     VAR_INPUT x : Big; END_VAR
       next := x + 1;
     END_FUNCTION
     FUNCTION same : Wide
     VAR_INPUT x : Wide; END_VAR
       same := x;
     END_FUNCTION
     PROGRAM main
     VAR b : Big := 4294967297; w : Wide := 4294967297; from_subrange : LINT; from_alias : LINT; END_VAR
       from_subrange := next(b);
       from_alias := same(w);
     END_PROGRAM",
    &[("from_subrange", 4_294_967_298), ("from_alias", 4_294_967_297)],
);
