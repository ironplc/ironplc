//! Parsing of the shared sample corpus (`.st` resources) and the
//! First Steps structural fixtures.

use super::common::*;

#[test]
fn parse_variable_declarations() {
    let res = parse_resource("var_decl.st");
    assert!(res.is_ok())
}

#[test]
fn parse_inout_variable_declarations() {
    let res = parse_resource("inout_var_decl.st");
    assert!(res.is_ok())
}

#[test]
fn parse_input_variable_declarations() {
    let res = parse_resource("input_var_decl.st");
    assert!(res.is_ok())
}

#[test]
fn parse_strings() {
    let res = parse_resource("strings.st");
    assert!(res.is_ok())
}

#[test]
fn parse_type_decl() {
    let res = parse_resource("type_decl.st");
    assert!(res.is_ok())
}

#[test]
fn parse_textual() {
    let res = parse_resource("textual.st");
    assert!(res.is_ok())
}

#[test]
fn parse_conditional() {
    let res = parse_resource("conditional.st");
    assert!(res.is_ok())
}

#[test]
fn parse_oscat() {
    // OSCAT files have a header that as far as I can tell is not valid
    // but it is common.
    let res = parse_resource("oscat.st");
    assert!(res.is_ok())
}

#[test]
fn parse_expressions() {
    let res = parse_resource("expressions.st");
    assert!(res.is_ok())
}

#[test]
fn parse_array() {
    let res = parse_resource("array.st");
    assert!(res.is_ok())
}

#[test]
fn parse_nested() {
    let res: Result<Library, Diagnostic> = parse_resource("nested.st");
    assert!(res.is_ok())
}

#[test]
fn parse_configuration() {
    let res: Result<Library, Diagnostic> = parse_resource("configuration.st");
    assert!(res.is_ok())
}

#[test]
fn parse_program_then_ok() {
    let res: Result<Library, Diagnostic> = parse_resource("program.st");
    assert!(res.is_ok())
}

#[test]
fn parse_if_then_ok() {
    let source = ironplc_test::read_shared_resource("if.st");
    let options = CompilerOptions {
        allow_missing_semicolon: true,
        ..Default::default()
    };
    let res = parse_program(&source, &FileId::default(), &options);
    assert!(res.is_ok())
}

#[test]
fn parse_when_first_steps_function_block_counter_fbd_then_builds_structure() {
    let actual = parse_resource("first_steps_function_block_counter_fbd.st").unwrap();
    let expected = new_library(LibraryElementKind::FunctionBlockDeclaration(
        FunctionBlockDeclaration {
            name: TypeName::from("CounterFBD"),
            variables: vec![
                VarDecl::simple("Reset", "BOOL").with_type(VariableType::Input),
                VarDecl::simple("OUT", "INT").with_type(VariableType::Output),
                VarDecl::simple("Cnt", "INT"),
                VarDecl {
                    identifier: VariableIdentifier::new_symbol("ResetCounterValue"),
                    var_type: VariableType::External,
                    qualifier: DeclarationQualifier::Constant,
                    initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                        type_name: TypeName::from("INT"),
                        initial_value: None,
                    }),
                    block: next_block_id(),
                },
                VarDecl::simple("_TMP_ADD4_OUT", "INT"),
                VarDecl::simple("_TMP_SEL7_OUT", "INT"),
            ],
            edge_variables: vec![],
            body: FunctionBlockBodyKind::stmts(vec![
                StmtKind::simple_assignment("Cnt", "_TMP_SEL7_OUT"),
                StmtKind::simple_assignment("OUT", "Cnt"),
            ]),
            span: SourceSpan::default(),
            oop: None,
        },
    ));
    assert_eq!(actual, expected);
}

#[test]
fn parse_when_first_steps_func_avg_val_then_builds_structure() {
    let actual = parse_resource("first_steps_func_avg_val.st").unwrap();
    let expected = new_library(LibraryElementKind::FunctionDeclaration(
        FunctionDeclaration {
            name: Id::from("AverageVal"),
            return_type: FunctionReturnType::Named(TypeName::from("REAL")),
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
                        type_name: TypeName::from("REAL"),
                        initial_value: Some(ConstantKind::RealLiteral(RealLiteral {
                            value: 5.1,
                            data_type: None,
                        })),
                    }),
                    block: next_block_id(),
                },
            ],
            edge_variables: vec![],
            body: vec![StmtKind::assignment(
                Variable::named("AverageVal"),
                ExprKind::binary(
                    Operator::Div,
                    ExprKind::Function(Function {
                        name: Id::from("INT_TO_REAL"),
                        param_assignment: vec![ParamAssignmentKind::positional(ExprKind::binary(
                            Operator::Add,
                            ExprKind::binary(
                                Operator::Add,
                                ExprKind::binary(
                                    Operator::Add,
                                    ExprKind::binary(
                                        Operator::Add,
                                        ExprKind::late_bound("Cnt1"),
                                        ExprKind::late_bound("Cnt2"),
                                    ),
                                    ExprKind::late_bound("Cnt3"),
                                ),
                                ExprKind::late_bound("Cnt4"),
                            ),
                            ExprKind::late_bound("Cnt5"),
                        ))],
                    }),
                    ExprKind::late_bound("InputsNumber"),
                ),
            )],
        },
    ));
    assert_eq!(actual, expected)
}

#[test]
fn parse_when_first_steps_program_declaration_then_builds_structure() {
    let actual = parse_resource("first_steps_program.st").unwrap();
    let expected = new_library(LibraryElementKind::ProgramDeclaration(ProgramDeclaration {
        name: Id::from("plc_prg"),
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
        access_variables: vec![],
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
    assert_eq!(actual, expected)
}

#[test]
fn parse_when_first_steps_configuration_then_builds_structure() {
    let actual = parse_resource("first_steps_configuration.st").unwrap();
    let expected = new_library(LibraryElementKind::ConfigurationDeclaration(
        ConfigurationDeclaration {
            name: Id::from("config"),
            global_var: vec![VarDecl {
                identifier: VariableIdentifier::new_symbol("ResetCounterValue"),
                var_type: VariableType::Global,
                qualifier: DeclarationQualifier::Constant,
                initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: TypeName::from("INT"),
                    initial_value: Some(ConstantKind::integer_literal("17").unwrap()),
                }),
                block: next_block_id(),
            }],
            resource_decl: vec![ResourceDeclaration {
                name: Id::from("resource1"),
                resource: Id::from("PLC"),
                global_vars: vec![],
                tasks: vec![TaskConfiguration {
                    name: Id::from("plc_task"),
                    priority: 1,
                    interval: Option::Some(DurationLiteral {
                        span: SourceSpan::default(),
                        interval: Duration::new(0, 100_000_000),
                    }),
                    single: None,
                }],
                programs: vec![ProgramConfiguration {
                    name: Id::from("plc_task_instance"),
                    storage: None,
                    task_name: Option::Some(Id::from("plc_task")),
                    type_name: Id::from("plc_prg"),
                    fb_tasks: vec![],
                    sources: vec![],
                    sinks: vec![],
                }],
            }],
            fb_inits: vec![],
            located_var_inits: vec![],
        },
    ));
    assert_eq!(actual, expected);
}

#[test]
fn parse_when_first_steps_function_block_logger_then_test_apply_when_names_correct_then_passes() {
    let actual = parse_resource("first_steps_function_block_logger.st").unwrap();
    let expected = new_library(LibraryElementKind::FunctionBlockDeclaration(
        FunctionBlockDeclaration {
            name: TypeName::from("LOGGER"),
            variables: vec![
                VarDecl::simple("TRIG", "BOOL").with_type(VariableType::Input),
                VarDecl::string(
                    "MSG",
                    VariableType::Input,
                    DeclarationQualifier::Unspecified,
                ),
                VarDecl::enumerated("LEVEL", "LOGLEVEL", "INFO").with_type(VariableType::Input),
                VarDecl::simple("TRIG0", "BOOL"),
            ],
            edge_variables: vec![],
            body: FunctionBlockBodyKind::stmts(vec![
                StmtKind::if_then(
                    ExprKind::compare(
                        CompareOp::And,
                        ExprKind::late_bound("TRIG"),
                        ExprKind::unary(UnaryOp::Not, ExprKind::late_bound("TRIG0")),
                    ),
                    vec![],
                ),
                StmtKind::assignment(Variable::named("TRIG0"), ExprKind::late_bound("TRIG")),
            ]),
            span: SourceSpan::default(),
            oop: None,
        },
    ));

    assert_eq!(actual, expected);
}

#[test]
fn parse_when_first_steps_function_block_counter_sfc_then_builds_structure() {
    let actual = parse_resource("first_steps_function_block_counter_sfc.st").unwrap();
    let expected = new_library(LibraryElementKind::FunctionBlockDeclaration(
        FunctionBlockDeclaration {
            name: TypeName::from("CounterSFC"),
            variables: vec![
                VarDecl::simple("Reset", "BOOL").with_type(VariableType::Input),
                VarDecl::simple("OUT", "INT").with_type(VariableType::Output),
                VarDecl::simple("Cnt", "INT"),
                VarDecl {
                    identifier: VariableIdentifier::new_symbol("ResetCounterValue"),
                    var_type: VariableType::External,
                    qualifier: DeclarationQualifier::Constant,
                    initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                        type_name: TypeName::from("INT"),
                        initial_value: None,
                    }),
                    block: next_block_id(),
                },
            ],
            edge_variables: vec![],
            body: FunctionBlockBodyKind::sfc(vec![Network {
                initial_step: Step {
                    name: Id::from("Start"),
                    action_associations: vec![],
                },
                elements: vec![
                    ElementKind::transition("Start", "ResetCounter", ExprKind::late_bound("Reset")),
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
                        ExprKind::unary(UnaryOp::Not, ExprKind::late_bound("Reset")),
                    ),
                    ElementKind::transition(
                        "Start",
                        "Count",
                        ExprKind::unary(UnaryOp::Not, ExprKind::late_bound("Reset")),
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
                                ExprKind::late_bound("Cnt"),
                                ExprKind::integer_literal("1"),
                            ),
                        )],
                    ),
                    ElementKind::action(
                        "COUNT_INLINE4",
                        vec![StmtKind::simple_assignment("OUT", "Cnt")],
                    ),
                    ElementKind::transition("Count", "Start", ExprKind::late_bound("Reset")),
                ],
            }]),
            span: SourceSpan::default(),
            oop: None,
        },
    ));
    assert_eq!(actual, expected);
}

#[test]
fn parse_when_first_steps_data_type_decl_then_builds_structure() {
    let actual = parse_resource("first_steps_data_type_decl.st").unwrap();
    let expected = new_library(LibraryElementKind::DataTypeDeclaration(
        DataTypeDeclarationKind::Enumeration(EnumerationDeclaration {
            type_name: TypeName::from("LOGLEVEL"),
            spec_init: EnumeratedSpecificationInit::values_and_default(
                vec!["CRITICAL", "WARNING", "INFO", "DEBUG"],
                "INFO",
            ),
        }),
    ));
    assert_eq!(actual, expected)
}
