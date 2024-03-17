#![allow(clippy::type_complexity)]

//! The compiler as individual stages (to enable testing).

use ironplc_dsl::{core::FileId, diagnostic::Diagnostic};

use crate::{
    compilation_set::{CompilationSet, CompilationSource},
    ironplc_dsl::common::Library,
    result::SemanticResult,
    rule_decl_struct_element_unique_names, rule_decl_subrange_limits,
    rule_enumeration_values_unique, rule_function_block_invocation, rule_pous_no_cycles,
    rule_program_task_definition_exists, rule_unsupported_stdlib_type,
    rule_use_declared_enumerated_value, rule_use_declared_symbolic_var,
    rule_var_decl_const_initialized, rule_var_decl_const_not_fb,
    rule_var_decl_global_const_requires_external_const, xform_assign_file_id,
    xform_resolve_late_bound_data_decl, xform_resolve_late_bound_type_initializer,
};

/// Parse create a library (set of elements) if the text is valid.
///
/// Returns `Ok(Library)` if parsing succeeded.
/// Returns `Err(Diagnostic)` if parsing did not succeed.
pub fn parse(source: &str, file_id: &FileId) -> Result<Library, Diagnostic> {
    let library = ironplc_parser::parse_program(source, file_id)?;

    // The parser does not know about the concept of files, so we apply the file
    // ID as a post transformation.
    xform_assign_file_id::apply(library, file_id)
}

/// Analyze runs semantic analysis on the set of files as a self-contained and complete unit.
///
/// Returns `Ok(Library)` if analysis succeeded (containing a possibly new library) that is
/// the merge of the inputs.
/// Returns `Err(Diagnostic)` if analysis did not succeed.
pub fn analyze(compilation_set: &CompilationSet) -> Result<(), Vec<Diagnostic>> {
    let library = resolve_types(compilation_set)?;
    semantic(&library)
}

pub(crate) fn resolve_types(compilation_set: &CompilationSet) -> Result<Library, Vec<Diagnostic>> {
    // We want to analyze this as a complete set, so we need to join the items together
    // into a single library. Extend owns the item so after this we are free to modify
    let mut library = Library::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for x in &compilation_set.sources {
        match x {
            CompilationSource::Library(lib) => {
                library = library.extend(lib.clone());
            }
            CompilationSource::Text(txt) => match parse(&txt.0, &txt.1) {
                Ok(lib) => library = library.extend(lib),
                Err(err) => diagnostics.push(err),
            },
            CompilationSource::TextRef(txt) => match parse(txt.0, &txt.1) {
                Ok(lib) => library = library.extend(lib),
                Err(err) => diagnostics.push(err),
            },
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let xforms: Vec<fn(Library) -> Result<Library, Vec<Diagnostic>>> = vec![
        xform_resolve_late_bound_data_decl::apply,
        xform_resolve_late_bound_type_initializer::apply,
    ];

    for xform in xforms {
        library = xform(library)?
    }

    Ok(library)
}

/// Semantic implements semantic analysis (stage 3).
///
/// Returns `Ok(())` if the library is free of semantic errors.
/// Returns `Err(String)` if the library contains a semantic error.
pub(crate) fn semantic(library: &Library) -> SemanticResult {
    let functions: Vec<fn(&Library) -> SemanticResult> = vec![
        rule_decl_struct_element_unique_names::apply,
        rule_decl_subrange_limits::apply,
        rule_enumeration_values_unique::apply,
        rule_function_block_invocation::apply,
        rule_pous_no_cycles::apply,
        rule_program_task_definition_exists::apply,
        rule_use_declared_enumerated_value::apply,
        rule_use_declared_symbolic_var::apply,
        rule_unsupported_stdlib_type::apply,
        rule_var_decl_const_initialized::apply,
        rule_var_decl_const_not_fb::apply,
        rule_var_decl_global_const_requires_external_const::apply,
    ];

    let mut all_diagnostics = vec![];
    for func in functions {
        match func(library) {
            Ok(_) => {
                // Nothing to do here
            }
            Err(diagnostics) => {
                all_diagnostics.extend(diagnostics);
            }
        }
    }

    if !all_diagnostics.is_empty() {
        return Err(all_diagnostics);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::stages::analyze;
    use crate::stages::CompilationSet;
    use crate::test_helpers;

    use super::parse;

    use ironplc_dsl::common::*;
    use ironplc_dsl::configuration::*;
    use ironplc_dsl::core::FileId;
    use ironplc_dsl::core::Id;
    use ironplc_dsl::core::SourceLoc;
    use ironplc_dsl::sfc::*;
    use ironplc_dsl::textual::*;
    use test_helpers::*;

    use time::Duration;

    use crate::stages::CompilationSource;

    impl<'a> CompilationSet<'a> {
        fn of_source(str: &String) -> Self {
            Self {
                sources: vec![CompilationSource::Text((
                    str.to_string(),
                    FileId::default(),
                ))],
                references: vec![],
            }
        }
    }

    #[test]
    fn analyze_when_first_steps_then_result_is_ok() {
        let src = read_resource("first_steps.st");
        let res = analyze(&CompilationSet::of_source(&src));
        assert!(res.is_ok());
    }

    #[test]
    fn analyze_when_first_steps_syntax_error_then_result_is_err() {
        let src = read_resource("first_steps_syntax_error.st");
        let res = analyze(&CompilationSet::of_source(&src));
        assert!(res.is_err())
    }

    #[test]
    fn analyze_when_first_steps_semantic_error_then_result_is_err() {
        let src = read_resource("first_steps_semantic_error.st");
        let res = analyze(&CompilationSet::of_source(&src));
        assert!(res.is_err())
    }

    #[test]
    fn parse_when_first_steps_data_type_decl_then_builds_structure() {
        let src = read_resource("first_steps_data_type_decl.st");
        let expected = new_library(LibraryElementKind::DataTypeDeclaration(
            DataTypeDeclarationKind::Enumeration(EnumerationDeclaration {
                type_name: Id::from("LOGLEVEL"),
                spec_init: EnumeratedSpecificationInit::values_and_default(
                    vec!["CRITICAL", "WARNING", "INFO", "DEBUG"],
                    "INFO",
                ),
            }),
        ));
        assert_eq!(parse(src.as_str(), &FileId::default()).unwrap(), expected)
    }

    #[test]
    fn analyze_2() {
        let src = read_resource("main.st");
        let res = analyze(&CompilationSet::of_source(&src));
        assert!(res.is_ok());
    }

    #[test]
    fn parse_when_first_steps_function_block_logger_then_test_apply_when_names_correct_then_passes()
    {
        let src = read_resource("first_steps_function_block_logger.st");
        let expected = new_library(LibraryElementKind::FunctionBlockDeclaration(
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
                body: FunctionBlockBodyKind::stmts(vec![
                    StmtKind::if_then(
                        ExprKind::compare(
                            CompareOp::And,
                            ExprKind::named_variable("TRIG"),
                            ExprKind::unary(UnaryOp::Not, ExprKind::named_variable("TRIG0")),
                        ),
                        vec![],
                    ),
                    StmtKind::assignment(
                        Variable::named("TRIG0"),
                        ExprKind::named_variable("TRIG"),
                    ),
                ]),
                position: SourceLoc::default(),
            },
        ));

        let res = ironplc_parser::parse_program(src.as_str(), &FileId::default()).unwrap();
        assert_eq!(res, expected);
        format!("{:?}", res.clone());
    }

    #[test]
    fn parse_when_first_steps_function_block_counter_sfc_then_builds_structure() {
        let src = read_resource("first_steps_function_block_counter_sfc.st");
        let expected = new_library(LibraryElementKind::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("CounterSFC"),
                variables: vec![
                    VarDecl::simple("Reset", "BOOL").with_type(VariableType::Input),
                    VarDecl::simple("OUT", "INT").with_type(VariableType::Output),
                    VarDecl::simple("Cnt", "INT"),
                    VarDecl {
                        identifier: VariableIdentifier::new_symbol("ResetCounterValue"),
                        var_type: VariableType::External,
                        qualifier: DeclarationQualifier::Constant,
                        initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                            type_name: Id::from("INT"),
                            initial_value: None,
                        }),
                        position: SourceLoc::default(),
                    },
                ],
                body: FunctionBlockBodyKind::sfc(vec![Network {
                    initial_step: Step {
                        name: Id::from("Start"),
                        action_associations: vec![],
                    },
                    elements: vec![
                        ElementKind::transition(
                            "Start",
                            "ResetCounter",
                            ExprKind::named_variable("Reset"),
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
                            vec![StmtKind::simple_assignment("Cnt", "ResetCounterValue")],
                        ),
                        ElementKind::action(
                            "RESETCOUNTER_INLINE2",
                            vec![StmtKind::simple_assignment("OUT", "Cnt")],
                        ),
                        ElementKind::transition(
                            "ResetCounter",
                            "Start",
                            ExprKind::unary(UnaryOp::Not, ExprKind::named_variable("Reset")),
                        ),
                        ElementKind::transition(
                            "Start",
                            "Count",
                            ExprKind::unary(UnaryOp::Not, ExprKind::named_variable("Reset")),
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
                                Variable::named("Cnt"),
                                ExprKind::binary(
                                    Operator::Add,
                                    ExprKind::named_variable("Cnt"),
                                    ExprKind::integer_literal("1"),
                                ),
                            )],
                        ),
                        ElementKind::action(
                            "COUNT_INLINE4",
                            vec![StmtKind::simple_assignment("OUT", "Cnt")],
                        ),
                        ElementKind::transition(
                            "Count",
                            "Start",
                            ExprKind::named_variable("Reset"),
                        ),
                    ],
                }]),
                position: SourceLoc::default(),
            },
        ));
        let res = ironplc_parser::parse_program(src.as_str(), &FileId::default()).unwrap();
        assert_eq!(res, expected);
        format!("{:?}", res.clone());
    }

    #[test]
    fn parse_when_first_steps_function_block_counter_fbd_then_builds_structure() {
        let src = read_resource("first_steps_function_block_counter_fbd.st");
        let expected = new_library(LibraryElementKind::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("CounterFBD"),
                variables: vec![
                    VarDecl::simple("Reset", "BOOL").with_type(VariableType::Input),
                    VarDecl::simple("OUT", "INT").with_type(VariableType::Output),
                    VarDecl::simple("Cnt", "INT"),
                    VarDecl {
                        identifier: VariableIdentifier::new_symbol("ResetCounterValue"),
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
                body: FunctionBlockBodyKind::stmts(vec![
                    StmtKind::simple_assignment("Cnt", "_TMP_SEL7_OUT"),
                    StmtKind::simple_assignment("OUT", "Cnt"),
                ]),
                position: SourceLoc::default(),
            },
        ));
        let res = ironplc_parser::parse_program(src.as_str(), &FileId::default()).unwrap();
        assert_eq!(res, expected);
        format!("{:?}", res.clone());
    }

    #[test]
    fn parse_when_first_steps_func_avg_val_then_builds_structure() {
        let src = read_resource("first_steps_func_avg_val.st");
        let expected = new_library(LibraryElementKind::FunctionDeclaration(
            FunctionDeclaration {
                name: Id::from("AverageVal"),
                return_type: Id::from("REAL"),
                variables: vec![
                    VarDecl::simple("Cnt1", "INT").with_type(VariableType::Input),
                    VarDecl::simple("Cnt2", "INT").with_type(VariableType::Input),
                    VarDecl::simple("Cnt3", "INT").with_type(VariableType::Input),
                    VarDecl::simple("Cnt4", "INT").with_type(VariableType::Input),
                    VarDecl::simple("Cnt5", "INT").with_type(VariableType::Input),
                    VarDecl {
                        identifier: VariableIdentifier::new_symbol("InputsNumber"),
                        var_type: VariableType::Var,
                        qualifier: DeclarationQualifier::Unspecified,
                        initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                            type_name: Id::from("REAL"),
                            initial_value: Some(ConstantKind::RealLiteral(RealLiteral {
                                value: 5.1,
                                data_type: None,
                            })),
                        }),
                        position: SourceLoc::default(),
                    },
                ],
                body: vec![StmtKind::assignment(
                    Variable::named("AverageVal"),
                    ExprKind::binary(
                        Operator::Div,
                        ExprKind::Function(Function {
                            name: Id::from("INT_TO_REAL"),
                            param_assignment: vec![ParamAssignmentKind::positional(
                                ExprKind::binary(
                                    Operator::Add,
                                    ExprKind::binary(
                                        Operator::Add,
                                        ExprKind::binary(
                                            Operator::Add,
                                            ExprKind::binary(
                                                Operator::Add,
                                                ExprKind::named_variable("Cnt1"),
                                                ExprKind::named_variable("Cnt2"),
                                            ),
                                            ExprKind::named_variable("Cnt3"),
                                        ),
                                        ExprKind::named_variable("Cnt4"),
                                    ),
                                    ExprKind::named_variable("Cnt5"),
                                ),
                            )],
                        }),
                        ExprKind::named_variable("InputsNumber"),
                    ),
                )],
            },
        ));
        let program = ironplc_parser::parse_program(src.as_str(), &FileId::default()).unwrap();
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
        let expected = new_library(LibraryElementKind::ProgramDeclaration(ProgramDeclaration {
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
            body: FunctionBlockBodyKind::stmts(vec![
                StmtKind::fb_call_mapped("CounterST0", vec![("Reset", "Reset")]),
                StmtKind::structured_assignment("Cnt1", "CounterST0", "OUT"),
                StmtKind::fb_assign(
                    "AverageVal",
                    vec!["Cnt1", "Cnt2", "Cnt3", "Cnt4", "Cnt5"],
                    "_TMP_AverageVal17_OUT",
                ),
                StmtKind::simple_assignment("AVCnt", "_TMP_AverageVal17_OUT"),
                StmtKind::fb_call_mapped("CounterFBD0", vec![("Reset", "Reset")]),
                StmtKind::structured_assignment("Cnt2", "CounterFBD0", "OUT"),
                StmtKind::fb_call_mapped("CounterSFC0", vec![("Reset", "Reset")]),
                StmtKind::structured_assignment("Cnt3", "CounterSFC0", "OUT"),
                StmtKind::fb_call_mapped("CounterIL0", vec![("Reset", "Reset")]),
                StmtKind::structured_assignment("Cnt4", "CounterIL0", "OUT"),
                StmtKind::fb_call_mapped("CounterLD0", vec![("Reset", "Reset")]),
                StmtKind::structured_assignment("Cnt5", "CounterLD0", "Out"),
            ]),
        }));
        assert_eq!(
            ironplc_parser::parse_program(src.as_str(), &FileId::default()).unwrap(),
            expected
        )
    }

    #[test]
    fn parse_when_first_steps_configuration_then_builds_structure() {
        let src = read_resource("first_steps_configuration.st");
        let expected = new_library(LibraryElementKind::ConfigurationDeclaration(
            ConfigurationDeclaration {
                name: Id::from("config"),
                global_var: vec![VarDecl {
                    identifier: VariableIdentifier::new_symbol("ResetCounterValue"),
                    var_type: VariableType::Global,
                    qualifier: DeclarationQualifier::Constant,
                    initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                        type_name: Id::from("INT"),
                        initial_value: Some(ConstantKind::integer_literal("17").unwrap()),
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
        let res = ironplc_parser::parse_program(src.as_str(), &FileId::default()).unwrap();
        assert_eq!(res, expected);
        format!("{:?}", res.clone());
    }

    #[test]
    fn analyze_when_split_across_multiple_files_then_ok() {
        let program1 = "
TYPE
LOGLEVEL : (CRITICAL) := CRITICAL;
END_TYPE";

        let program2 = "
FUNCTION_BLOCK LOGGER
VAR_EXTERNAL CONSTANT
ResetCounterValue : LOGLEVEL;
END_VAR

END_FUNCTION_BLOCK";

        let mut compilation_set = CompilationSet::new();
        compilation_set.push(CompilationSource::Text((
            program1.to_owned(),
            FileId::default(),
        )));
        compilation_set.push(CompilationSource::Text((
            program2.to_owned(),
            FileId::default(),
        )));

        let result = analyze(&compilation_set);
        assert!(result.is_ok())
    }
}
