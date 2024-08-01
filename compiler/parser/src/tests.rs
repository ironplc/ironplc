//! Tests of parser.
#[cfg(test)]
mod test {
    use dsl::common::{
        ConstantKind, DataTypeDeclarationKind, DeclarationQualifier, EnumeratedSpecificationInit,
        EnumerationDeclaration, FunctionBlockBodyKind, FunctionBlockDeclaration,
        FunctionDeclaration, InitialValueAssignmentKind, Library, LibraryElementKind,
        ProgramDeclaration, RealLiteral, SimpleInitializer, Type, VarDecl, VariableIdentifier,
        VariableType,
    };
    use dsl::configuration::{
        ConfigurationDeclaration, ProgramConfiguration, ResourceDeclaration, TaskConfiguration,
    };
    use dsl::core::{FileId, Id, SourceSpan};
    use dsl::diagnostic::Diagnostic;
    use dsl::sfc::{ActionAssociation, ActionQualifier, ElementKind, Network, Step};
    use dsl::textual::{
        CompareOp, ExprKind, Function, Operator, ParamAssignmentKind, StmtKind, UnaryOp, Variable,
    };
    use ironplc_test::read_shared_resource;
    use time::Duration;

    use crate::parse_program;

    pub fn parse_resource(name: &'static str) -> Result<Library, Diagnostic> {
        let source = read_shared_resource(name);
        parse_program(&source, &FileId::default())
    }

    #[test]
    fn parse_variable_declarations() {
        let res = parse_resource("var_decl.st");
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
    fn parse_program_when_has_comment_then_ok() {
        let source = "
        TYPE
        (* A comment *)
            CUSTOM_STRUCT : STRUCT 
                NAME: BOOL;
            END_STRUCT;
        END_TYPE";

        let res = parse_program(source, &FileId::default()).unwrap();
        assert_eq!(1, res.elements.len());
    }

    #[test]
    fn parse_program_when_back_to_back_comments_then_ok() {
        let program = "
        TYPE
        (* A comment *)(* A comment *)
           CUSTOM_STRUCT : STRUCT 
             NAME: BOOL;
           END_STRUCT;
        END_TYPE";

        let res = parse_program(program, &FileId::default());
        assert!(res.is_ok());
    }

    #[test]
    fn parse_program_when_right_parent_in_comment_then_ok() {
        let program = "
        TYPE
        (* A comment) *)(* A comment *)
           CUSTOM_STRUCT : STRUCT 
             NAME: BOOL;
           END_STRUCT;
        END_TYPE";

        let res = parse_program(program, &FileId::default());
        assert!(res.is_ok());
    }

    #[test]
    fn parse_program_when_comment_not_closed_then_err() {
        let program = "
        TYPE
        (* A comment
            CUSTOM_STRUCT : STRUCT 
                NAME: BOOL;
            END_STRUCT;
        END_TYPE";

        let res = parse_program(program, &FileId::default());
        assert!(res.is_err());

        let err = res.unwrap_err();
        assert_eq!(
            "Unmatched character sequence in source text".to_owned(),
            err.description()
        );
        assert_eq!("The text '(* A comment\n            CUSTOM_STRUCT : STRUCT \n                NAME: BOOL;\n            END_STRUCT;\n        END_TYPE' is not valid IEC 61131-3 text at line 3 colum 9.".to_owned(), err.primary.message);
    }

    #[test]
    fn parse_program_when_bad_name_then_err() {
        let program = "
        TYPE
            CUSTOM_STRUCT : STRUCT& 
                NAME: BOOL;
            END_STRUCT;
        END_TYPE";

        let res = parse_program(program, &FileId::default());
        assert!(res.is_err());

        let err = res.unwrap_err();
        assert_eq!("Syntax error".to_owned(), err.description());
        assert_eq!("Expected ' ' (space) | '\\t' (tab) | '(* ... *)' (comment) | '\\n' (new line) | (identifier). Found text '&' that matched token 'AND' | '&'".to_owned(), err.primary.message);
    }

    #[test]
    fn parse_program_when_not_valid_top_item_then_err() {
        let program = "ACTION
        END_ACTION";

        let res = parse_program(program, &FileId::default());
        assert!(res.is_err());

        let err = res.unwrap_err();
        assert_eq!("Syntax error".to_owned(), err.description());
        assert_eq!("Expected ' ' (space) | '\\t' (tab) | '(* ... *)' (comment) | 'CONFIGURATION' | 'FUNCTION' | 'FUNCTION_BLOCK' | 'PROGRAM' | 'TYPE' | '\\n' (new line). Found text 'ACTION' that matched token 'ACTION'".to_owned(), err.primary.message);
    }

    #[test]
    fn parse_program_when_complex_bit_string_then_ok() {
        let program = "
FUNCTION fun:DWORD

VAR_IN_OUT
    VAR1: INT;
END_VAR

VAR1 := DWORD#16#0000FFFF;

END_FUNCTION";

        let res = parse_program(program, &FileId::default());
        assert!(res.is_ok());
    }

    #[test]
    fn parse_program_when_real_then_ok() {
        let program = "
FUNCTION fun:DWORD

VAR
    InputsNumber : REAL := -5.0E-1;
END_VAR

fun := InputsNumber;

END_FUNCTION";
        let res = parse_program(program, &FileId::default()).unwrap();

        let expected = new_library(LibraryElementKind::FunctionDeclaration(
            FunctionDeclaration {
                name: Id::from("fun"),
                return_type: Type::from("DWORD"),
                variables: vec![VarDecl {
                    identifier: VariableIdentifier::new_symbol("InputsNumber"),
                    var_type: VariableType::Var,
                    qualifier: DeclarationQualifier::Unspecified,
                    initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                        type_name: Type::from("REAL"),
                        initial_value: Some(ConstantKind::RealLiteral(RealLiteral {
                            value: -0.5,
                            data_type: None,
                        })),
                    }),
                }],
                body: vec![StmtKind::simple_assignment("fun", "InputsNumber")],
            },
        ));
        assert_eq!(res, expected);
    }

    #[test]
    fn parse_when_first_steps_function_block_counter_fbd_then_builds_structure() {
        let src = read_shared_resource("first_steps_function_block_counter_fbd.st");
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
                            type_name: Type::from("INT"),
                            initial_value: None,
                        }),
                    },
                    VarDecl::simple("_TMP_ADD4_OUT", "INT"),
                    VarDecl::simple("_TMP_SEL7_OUT", "INT"),
                ],
                body: FunctionBlockBodyKind::stmts(vec![
                    StmtKind::simple_assignment("Cnt", "_TMP_SEL7_OUT"),
                    StmtKind::simple_assignment("OUT", "Cnt"),
                ]),
                span: SourceSpan::default(),
            },
        ));
        let res = parse_program(src.as_str(), &FileId::default()).unwrap();
        assert_eq!(res, expected);
    }

    #[test]
    fn parse_when_first_steps_func_avg_val_then_builds_structure() {
        let src = read_shared_resource("first_steps_func_avg_val.st");
        let expected = new_library(LibraryElementKind::FunctionDeclaration(
            FunctionDeclaration {
                name: Id::from("AverageVal"),
                return_type: Type::from("REAL"),
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
                            type_name: Type::from("REAL"),
                            initial_value: Some(ConstantKind::RealLiteral(RealLiteral {
                                value: 5.1,
                                data_type: None,
                            })),
                        }),
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
                                                ExprKind::late_bound("Cnt1"),
                                                ExprKind::late_bound("Cnt2"),
                                            ),
                                            ExprKind::late_bound("Cnt3"),
                                        ),
                                        ExprKind::late_bound("Cnt4"),
                                    ),
                                    ExprKind::late_bound("Cnt5"),
                                ),
                            )],
                        }),
                        ExprKind::late_bound("InputsNumber"),
                    ),
                )],
            },
        ));
        let program = parse_program(src.as_str(), &FileId::default()).unwrap();
        assert_eq!(program, expected)
    }

    #[test]
    fn parse_when_first_steps_program_declaration_then_builds_structure() {
        let src = read_shared_resource("first_steps_program.st");
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
            parse_program(src.as_str(), &FileId::default()).unwrap(),
            expected
        )
    }

    #[test]
    fn parse_when_first_steps_configuration_then_builds_structure() {
        let src = read_shared_resource("first_steps_configuration.st");
        let expected = new_library(LibraryElementKind::ConfigurationDeclaration(
            ConfigurationDeclaration {
                name: Id::from("config"),
                global_var: vec![VarDecl {
                    identifier: VariableIdentifier::new_symbol("ResetCounterValue"),
                    var_type: VariableType::Global,
                    qualifier: DeclarationQualifier::Constant,
                    initializer: InitialValueAssignmentKind::Simple(SimpleInitializer {
                        type_name: Type::from("INT"),
                        initial_value: Some(ConstantKind::integer_literal("17").unwrap()),
                    }),
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
                        fb_tasks: vec![],
                        sources: vec![],
                        sinks: vec![],
                    }],
                }],
                fb_inits: vec![],
                located_var_inits: vec![],
            },
        ));
        let res = parse_program(src.as_str(), &FileId::default()).unwrap();
        assert_eq!(res, expected);
        format!("{:?}", res.clone());
    }

    #[test]
    fn parse_when_first_steps_function_block_logger_then_test_apply_when_names_correct_then_passes()
    {
        let src = read_shared_resource("first_steps_function_block_logger.st");
        let expected = new_library(LibraryElementKind::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("LOGGER"),
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
            },
        ));

        let res = parse_program(src.as_str(), &FileId::default()).unwrap();
        assert_eq!(res, expected);
        format!("{:?}", res.clone());
    }

    #[test]
    fn parse_when_first_steps_function_block_counter_sfc_then_builds_structure() {
        let src = read_shared_resource("first_steps_function_block_counter_sfc.st");
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
                            type_name: Type::from("INT"),
                            initial_value: None,
                        }),
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
                            ExprKind::late_bound("Reset"),
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
            },
        ));
        let res = parse_program(src.as_str(), &FileId::default()).unwrap();
        assert_eq!(res, expected);
        format!("{:?}", res.clone());
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
    fn parse_when_first_steps_data_type_decl_then_builds_structure() {
        let src = read_shared_resource("first_steps_data_type_decl.st");
        let expected = new_library(LibraryElementKind::DataTypeDeclaration(
            DataTypeDeclarationKind::Enumeration(EnumerationDeclaration {
                type_name: Type::from("LOGLEVEL"),
                spec_init: EnumeratedSpecificationInit::values_and_default(
                    vec!["CRITICAL", "WARNING", "INFO", "DEBUG"],
                    "INFO",
                ),
            }),
        ));
        assert_eq!(
            parse_program(src.as_str(), &FileId::default()).unwrap(),
            expected
        )
    }

    #[cfg(test)]
    pub fn new_library(element: LibraryElementKind) -> Library {
        Library {
            elements: vec![element],
        }
    }
}
