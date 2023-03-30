#![allow(clippy::type_complexity)]

//! The compiler as individual stages (to enable testing).

use ironplc_dsl::{core::FileId, diagnostic::Diagnostic};

use crate::{
    ironplc_dsl::common::Library, rule_decl_struct_element_unique_names, rule_decl_subrange_limits,
    rule_enumeration_values_unique, rule_function_block_invocation, rule_pous_no_cycles,
    rule_program_task_definition_exists, rule_use_declared_enumerated_value,
    rule_use_declared_symbolic_var, rule_var_decl_const_initialized, rule_var_decl_const_not_fb,
    rule_var_decl_global_const_requires_external_const, xform_resolve_late_bound_data_decl,
    xform_resolve_late_bound_type_initializer,
};

pub fn analyze(contents: &str, file_id: &FileId) -> Result<(), Diagnostic> {
    let library = parse(contents, file_id)?;
    semantic(&library)
}

/// Parse combines lexical and sematic analysis (stages 1 & 2).
///
/// The stage takes text and returns a library of domain specific
/// objects including resolution of all ambiguous syntax.
///
/// Returns `Ok(Library)` if parsing succeeded.
/// Returns `Err(String)` if parsing did not succeed.
pub fn parse(source: &str, file_id: &FileId) -> Result<Library, Diagnostic> {
    let library = ironplc_parser::parse_program(source, file_id)?;

    // Resolve the late bound type declarations, replacing with
    // the type-specific declarations. This just simplifies
    // code generation because we know the type of every declaration
    // exactly
    let library = xform_resolve_late_bound_data_decl::apply(library)?;
    xform_resolve_late_bound_type_initializer::apply(library)
}

/// Semantic implements semantic analysis (stage 3).
///
/// Returns `Ok(())` if the library is free of semantic errors.
/// Returns `Err(String)` if the library contains a semantic error.
pub fn semantic(library: &Library) -> Result<(), Diagnostic> {
    let functions: Vec<fn(&Library) -> Result<(), Diagnostic>> = vec![
        rule_use_declared_symbolic_var::apply,
        rule_use_declared_enumerated_value::apply,
        rule_function_block_invocation::apply,
        rule_var_decl_const_initialized::apply,
        rule_var_decl_const_not_fb::apply,
        rule_enumeration_values_unique::apply,
        rule_program_task_definition_exists::apply,
        rule_pous_no_cycles::apply,
        rule_var_decl_global_const_requires_external_const::apply,
        rule_decl_subrange_limits::apply,
        rule_decl_struct_element_unique_names::apply,
    ];

    for func in functions {
        func(library)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::stages::analyze;
    use crate::test_helpers;

    use super::parse;

    use ironplc_dsl::common::*;
    use ironplc_dsl::core::Id;
    use ironplc_dsl::core::SourceLoc;
    use ironplc_dsl::sfc::*;
    use ironplc_dsl::textual::*;
    use test_helpers::*;

    use time::Duration;

    #[test]
    fn analyze_when_first_steps_then_result_is_ok() {
        let src = read_resource("first_steps.st");
        let res = analyze(&src, &PathBuf::default());
        assert!(res.is_ok())
    }

    #[test]
    fn analyze_when_first_steps_syntax_error_then_result_is_err() {
        let src = read_resource("first_steps_syntax_error.st");
        let res = analyze(&src, &PathBuf::default());
        assert!(res.is_err())
    }

    #[test]
    fn analyze_when_first_steps_semantic_error_then_result_is_err() {
        let src = read_resource("first_steps_semantic_error.st");
        let res = analyze(&src, &PathBuf::default());
        assert!(res.is_err())
    }

    #[test]
    fn parse_when_first_steps_data_type_decl_then_builds_structure() {
        let src = read_resource("first_steps_data_type_decl.st");
        let expected = new_library(LibraryElement::DataTypeDeclaration(
            DataTypeDeclarationKind::Enumeration(EnumerationDeclaration {
                type_name: Id::from("LOGLEVEL"),
                spec_init: EnumeratedSpecificationInit::values_and_default(
                    vec!["CRITICAL", "WARNING", "INFO", "DEBUG"],
                    "INFO",
                ),
            }),
        ));
        assert_eq!(parse(src.as_str(), &PathBuf::default()).unwrap(), expected)
    }

    #[test]
    fn parse_when_first_steps_function_block_logger_then_test_apply_when_names_correct_then_passes()
    {
        let src = read_resource("first_steps_function_block_logger.st");
        let expected = new_library(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("LOGGER"),
                variables: vec![
                    VarDecl::simple("TRIG", "BOOL").with_type(VariableType::Input),
                    VarDecl::string(
                        "MSG",
                        VariableType::Input,
                        DeclarationQualifier::Unspecified,
                        SourceLoc::default(),
                    ),
                    VarDecl::enumerated("LEVEL", "LOGLEVEL", "INFO").with_type(VariableType::Input),
                    VarDecl::simple("TRIG0", "BOOL"),
                ],
                body: FunctionBlockBody::stmts(vec![
                    StmtKind::if_then(
                        ExprKind::compare(
                            CompareOp::And,
                            ExprKind::symbolic_variable("TRIG"),
                            ExprKind::unary(UnaryOp::Not, ExprKind::symbolic_variable("TRIG0")),
                        ),
                        vec![],
                    ),
                    StmtKind::assignment(
                        Variable::symbolic("TRIG0"),
                        ExprKind::symbolic_variable("TRIG"),
                    ),
                ]),
                position: SourceLoc::default(),
            },
        ));
        assert_eq!(
            ironplc_parser::parse_program(src.as_str(), &PathBuf::default()).unwrap(),
            expected
        )
    }

    #[test]
    fn parse_when_first_steps_function_block_counter_sfc_then_builds_structure() {
        let src = read_resource("first_steps_function_block_counter_sfc.st");
        let expected = new_library(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("CounterSFC"),
                variables: vec![
                    VarDecl::simple("Reset", "BOOL").with_type(VariableType::Input),
                    VarDecl::simple("OUT", "INT").with_type(VariableType::Output),
                    VarDecl::simple("Cnt", "INT"),
                    VarDecl {
                        name: Id::from("ResetCounterValue"),
                        var_type: VariableType::External,
                        qualifier: DeclarationQualifier::Constant,
                        initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                            type_name: Id::from("INT"),
                            initial_value: None,
                        }),
                        position: SourceLoc::default(),
                    },
                ],
                body: FunctionBlockBody::sfc(vec![Network {
                    initial_step: Step {
                        name: Id::from("Start"),
                        action_associations: vec![],
                    },
                    elements: vec![
                        ElementKind::transition(
                            "Start",
                            "ResetCounter",
                            ExprKind::symbolic_variable("Reset"),
                        ),
                        ElementKind::step(
                            Id::from("ResetCounter"),
                            vec![
                                ActionAssociation::new(
                                    "RESETCOUNTER_INLINE1",
                                    Some(ActionQualifier::N),
                                ),
                                ActionAssociation::new(
                                    "RESETCOUNTER_INLINE2",
                                    Some(ActionQualifier::N),
                                ),
                            ],
                        ),
                        ElementKind::action(
                            "RESETCOUNTER_INLINE1",
                            vec![StmtKind::simple_assignment(
                                "Cnt",
                                vec!["ResetCounterValue"],
                            )],
                        ),
                        ElementKind::action(
                            "RESETCOUNTER_INLINE2",
                            vec![StmtKind::simple_assignment("OUT", vec!["Cnt"])],
                        ),
                        ElementKind::transition(
                            "ResetCounter",
                            "Start",
                            ExprKind::unary(UnaryOp::Not, ExprKind::symbolic_variable("Reset")),
                        ),
                        ElementKind::transition(
                            "Start",
                            "Count",
                            ExprKind::unary(UnaryOp::Not, ExprKind::symbolic_variable("Reset")),
                        ),
                        ElementKind::step(
                            Id::from("Count"),
                            vec![
                                ActionAssociation::new("COUNT_INLINE3", Some(ActionQualifier::N)),
                                ActionAssociation::new("COUNT_INLINE4", Some(ActionQualifier::N)),
                            ],
                        ),
                        ElementKind::action(
                            "COUNT_INLINE3",
                            vec![StmtKind::assignment(
                                Variable::symbolic("Cnt"),
                                ExprKind::binary(
                                    Operator::Add,
                                    ExprKind::symbolic_variable("Cnt"),
                                    ExprKind::integer_literal("1"),
                                ),
                            )],
                        ),
                        ElementKind::action(
                            "COUNT_INLINE4",
                            vec![StmtKind::simple_assignment("OUT", vec!["Cnt"])],
                        ),
                        ElementKind::transition(
                            "Count",
                            "Start",
                            ExprKind::symbolic_variable("Reset"),
                        ),
                    ],
                }]),
                position: SourceLoc::default(),
            },
        ));
        assert_eq!(
            ironplc_parser::parse_program(src.as_str(), &PathBuf::default()).unwrap(),
            expected
        )
    }

    #[test]
    fn parse_when_first_steps_function_block_counter_fbd_then_builds_structure() {
        let src = read_resource("first_steps_function_block_counter_fbd.st");
        let expected = new_library(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("CounterFBD"),
                variables: vec![
                    VarDecl::simple("Reset", "BOOL").with_type(VariableType::Input),
                    VarDecl::simple("OUT", "INT").with_type(VariableType::Output),
                    VarDecl::simple("Cnt", "INT"),
                    VarDecl {
                        name: Id::from("ResetCounterValue"),
                        var_type: VariableType::External,
                        qualifier: DeclarationQualifier::Constant,
                        initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                            type_name: Id::from("INT"),
                            initial_value: None,
                        }),
                        position: SourceLoc::default(),
                    },
                    VarDecl::simple("_TMP_ADD4_OUT", "INT"),
                    VarDecl::simple("_TMP_SEL7_OUT", "INT"),
                ],
                body: FunctionBlockBody::stmts(vec![
                    StmtKind::simple_assignment("Cnt", vec!["_TMP_SEL7_OUT"]),
                    StmtKind::simple_assignment("OUT", vec!["Cnt"]),
                ]),
                position: SourceLoc::default(),
            },
        ));
        assert_eq!(
            ironplc_parser::parse_program(src.as_str(), &PathBuf::default()).unwrap(),
            expected
        )
    }

    #[test]
    fn parse_when_first_steps_func_avg_val_then_builds_structure() {
        let src = read_resource("first_steps_func_avg_val.st");
        let expected = new_library(LibraryElement::FunctionDeclaration(FunctionDeclaration {
            name: Id::from("AverageVal"),
            return_type: Id::from("REAL"),
            variables: vec![
                VarDecl::simple("Cnt1", "INT").with_type(VariableType::Input),
                VarDecl::simple("Cnt2", "INT").with_type(VariableType::Input),
                VarDecl::simple("Cnt3", "INT").with_type(VariableType::Input),
                VarDecl::simple("Cnt4", "INT").with_type(VariableType::Input),
                VarDecl::simple("Cnt5", "INT").with_type(VariableType::Input),
                VarDecl {
                    name: Id::from("InputsNumber"),
                    var_type: VariableType::Var,
                    qualifier: DeclarationQualifier::Unspecified,
                    initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                        type_name: Id::from("REAL"),
                        initial_value: Some(Constant::RealLiteral(Float {
                            value: 5.1,
                            data_type: None,
                        })),
                    }),
                    position: SourceLoc::default(),
                },
            ],
            body: vec![StmtKind::assignment(
                Variable::symbolic("AverageVal"),
                ExprKind::binary(
                    Operator::Div,
                    ExprKind::Function {
                        name: Id::from("INT_TO_REAL"),
                        param_assignment: vec![ParamAssignmentKind::positional(ExprKind::binary(
                            Operator::Add,
                            ExprKind::binary(
                                Operator::Add,
                                ExprKind::binary(
                                    Operator::Add,
                                    ExprKind::binary(
                                        Operator::Add,
                                        ExprKind::symbolic_variable("Cnt1"),
                                        ExprKind::symbolic_variable("Cnt2"),
                                    ),
                                    ExprKind::symbolic_variable("Cnt3"),
                                ),
                                ExprKind::symbolic_variable("Cnt4"),
                            ),
                            ExprKind::symbolic_variable("Cnt5"),
                        ))],
                    },
                    ExprKind::symbolic_variable("InputsNumber"),
                ),
            )],
        }));
        let program = ironplc_parser::parse_program(src.as_str(), &PathBuf::default()).unwrap();
        assert_eq!(program, expected)
    }

    // TODO add this as a test
    //#[test]
    //fn first_steps_function_block_counter_ld() {
    //    let src = read_resource("first_steps_function_block_counter_ld.st");
    //    let expected = Ok(vec![
    //    ]);
    //    assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    //}

    #[test]
    fn parse_when_first_steps_program_declaration_then_builds_structure() {
        let src = read_resource("first_steps_program.st");
        let expected = new_library(LibraryElement::ProgramDeclaration(ProgramDeclaration {
            type_name: Id::from("plc_prg"),
            variables: vec![
                VarDecl::simple("Reset", "BOOL").with_type(VariableType::Input),
                VarDecl::simple("Cnt1", "INT").with_type(VariableType::Output),
                VarDecl::simple("Cnt2", "INT").with_type(VariableType::Output),
                VarDecl::simple("Cnt3", "INT").with_type(VariableType::Output),
                VarDecl::simple("Cnt4", "INT").with_type(VariableType::Output),
                VarDecl::simple("Cnt5", "INT").with_type(VariableType::Output),
                VarDecl::late_bound("CounterST0", "CounterST"),
                VarDecl::late_bound("CounterFBD0", "CounterFBD"),
                VarDecl::late_bound("CounterSFC0", "CounterSFC"),
                VarDecl::late_bound("CounterIL0", "CounterIL"),
                VarDecl::late_bound("CounterLD0", "CounterLD"),
                VarDecl::simple("AVCnt", "REAL"),
                VarDecl::simple("_TMP_AverageVal17_OUT", "REAL"),
            ],
            body: FunctionBlockBody::stmts(vec![
                StmtKind::fb_call_mapped("CounterST0", vec![("Reset", "Reset")]),
                StmtKind::simple_assignment("Cnt1", vec!["CounterST0", "OUT"]),
                StmtKind::fb_assign(
                    "AverageVal",
                    vec!["Cnt1", "Cnt2", "Cnt3", "Cnt4", "Cnt5"],
                    "_TMP_AverageVal17_OUT",
                ),
                StmtKind::simple_assignment("AVCnt", vec!["_TMP_AverageVal17_OUT"]),
                StmtKind::fb_call_mapped("CounterFBD0", vec![("Reset", "Reset")]),
                StmtKind::simple_assignment("Cnt2", vec!["CounterFBD0", "OUT"]),
                StmtKind::fb_call_mapped("CounterSFC0", vec![("Reset", "Reset")]),
                StmtKind::simple_assignment("Cnt3", vec!["CounterSFC0", "OUT"]),
                StmtKind::fb_call_mapped("CounterIL0", vec![("Reset", "Reset")]),
                StmtKind::simple_assignment("Cnt4", vec!["CounterIL0", "OUT"]),
                StmtKind::fb_call_mapped("CounterLD0", vec![("Reset", "Reset")]),
                StmtKind::simple_assignment("Cnt5", vec!["CounterLD0", "Out"]),
            ]),
        }));
        assert_eq!(
            ironplc_parser::parse_program(src.as_str(), &PathBuf::default()).unwrap(),
            expected
        )
    }

    #[test]
    fn parse_when_first_steps_configuration_then_builds_structure() {
        let src = read_resource("first_steps_configuration.st");
        let expected = new_library(LibraryElement::ConfigurationDeclaration(
            ConfigurationDeclaration {
                name: Id::from("config"),
                global_var: vec![VarDecl {
                    name: Id::from("ResetCounterValue"),
                    var_type: VariableType::Global,
                    qualifier: DeclarationQualifier::Constant,
                    initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                        type_name: Id::from("INT"),
                        initial_value: Some(Constant::integer_literal("17")),
                    }),
                    position: SourceLoc::default(),
                }],
                resource_decl: vec![ResourceDeclaration {
                    name: Id::from("resource1"),
                    resource: Id::from("PLC"),
                    global_vars: vec![],
                    tasks: vec![TaskConfiguration {
                        name: Id::from("plc_task"),
                        priority: 1,
                        interval: Option::Some(Duration::new(0, 100_000_000)),
                    }],
                    programs: vec![ProgramConfiguration {
                        name: Id::from("plc_task_instance"),
                        task_name: Option::Some(Id::from("plc_task")),
                        type_name: Id::from("plc_prg"),
                    }],
                }],
            },
        ));
        assert_eq!(
            ironplc_parser::parse_program(src.as_str(), &PathBuf::default()).unwrap(),
            expected
        )
    }
}
