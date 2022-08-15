extern crate ironplc_dsl;
extern crate ironplc_parser;

pub fn main() {
    ironplc_parser::parse_program("");
    // Static analysis (binding) and building symbol table
    // Code generation
}

#[cfg(test)]
mod tests {
    use super::*;

    use ironplc_dsl::ast::*;
    use ironplc_dsl::dsl::*;
    use ironplc_dsl::sfc::*;
    use std::fs;
    use std::path::PathBuf;
    use time::Duration;

    fn read_resource(name: &'static str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/test");
        path.push(name);

        fs::read_to_string(path).expect("Unable to read file")
    }

    #[test]
    fn first_steps() {
        let src = read_resource("first_steps.st");
        let res = ironplc_parser::parse_program(src.as_str());
        assert!(res.is_ok())
    }

    #[test]
    fn first_steps_data_type_decl() {
        let src = read_resource("first_steps_data_type_decl.st");
        let expected = Ok(vec![LibraryElement::DataTypeDeclaration(vec![
            EnumerationDeclaration {
                name: String::from("LOGLEVEL"),
                initializer: TypeInitializer::EnumeratedValues {
                    values: vec![
                        String::from("CRITICAL"),
                        String::from("WARNING"),
                        String::from("INFO"),
                        String::from("DEBUG"),
                    ],
                    default: Option::Some(String::from("INFO")),
                },
            },
        ])]);
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    }

    #[test]
    fn first_steps_function_block_logger() {
        let src = read_resource("first_steps_function_block_logger.st");
        let expected = Ok(vec![LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: String::from("LOGGER"),
                var_decls: vec![
                    VarInitKind::simple("TRIG", "BOOL"),
                    VarInitKind::simple("MSG", "STRING"),
                    VarInitKind::enumerated("LEVEL", "LOGLEVEL", "INFO"),
                    VarInitKind::simple("TRIG0", "BOOL"),
                ],
                body: FunctionBlockBody::Statements(vec![
                    StmtKind::If {
                        expr: ExprKind::Compare {
                            op: CompareOp::And,
                            terms: vec![
                                ExprKind::symbolic_variable("TRIG"),
                                ExprKind::UnaryOp {
                                    op: UnaryOp::Not,
                                    term: ExprKind::boxed_symbolic_variable("TRIG0"),
                                },
                            ],
                        },
                        body: vec![],
                        else_body: vec![],
                    },
                    StmtKind::Assignment {
                        target: Variable::SymbolicVariable(String::from("TRIG0")),
                        value: ExprKind::symbolic_variable("TRIG"),
                    },
                ]),
            },
        )]);
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    }

    #[test]
    fn first_steps_function_block_counter_sfc() {
        let src = read_resource("first_steps_function_block_counter_sfc.st");
        let expected = Ok(vec![LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: String::from("CounterSFC"),
                var_decls: vec![
                    VarInitKind::simple("Reset", "BOOL"),
                    VarInitKind::simple("OUT", "INT"),
                    VarInitKind::simple("Cnt", "INT"),
                    VarInitKind::VarInit(VarInit {
                        name: String::from("ResetCounterValue"),
                        storage_class: StorageClass::Constant,
                        initializer: Some(TypeInitializer::Simple {
                            type_name: String::from("INT"),
                            initial_value: None,
                        }),
                    }),
                ],
                body: FunctionBlockBody::Sfc(vec![Network {
                    initial_step: Element::InitialStep {
                        name: String::from("Start"),
                        action_associations: vec![],
                    },
                    elements: vec![
                        Element::transition(
                            "Start",
                            "ResetCounter",
                            ExprKind::symbolic_variable("Reset"),
                        ),
                        Element::Step {
                            name: String::from("ResetCounter"),
                            action_associations: vec![
                                ActionAssociation::new(
                                    "RESETCOUNTER_INLINE1",
                                    Some(ActionQualifier::N),
                                ),
                                ActionAssociation::new(
                                    "RESETCOUNTER_INLINE2",
                                    Some(ActionQualifier::N),
                                ),
                            ],
                        },
                        Element::action(
                            "RESETCOUNTER_INLINE1",
                            vec![StmtKind::assignment("Cnt", vec!["ResetCounterValue"])],
                        ),
                        Element::action(
                            "RESETCOUNTER_INLINE2",
                            vec![StmtKind::assignment("OUT", vec!["Cnt"])],
                        ),
                        Element::transition(
                            "ResetCounter",
                            "Start",
                            ExprKind::UnaryOp {
                                op: UnaryOp::Not,
                                term: ExprKind::boxed_symbolic_variable("Reset"),
                            },
                        ),
                        Element::transition(
                            "Start",
                            "Count",
                            ExprKind::UnaryOp {
                                op: UnaryOp::Not,
                                term: ExprKind::boxed_symbolic_variable("Reset"),
                            },
                        ),
                        Element::Step {
                            name: String::from("Count"),
                            action_associations: vec![
                                ActionAssociation::new("COUNT_INLINE3", Some(ActionQualifier::N)),
                                ActionAssociation::new("COUNT_INLINE4", Some(ActionQualifier::N)),
                            ],
                        },
                        Element::action(
                            "COUNT_INLINE3",
                            vec![StmtKind::Assignment {
                                target: Variable::SymbolicVariable(String::from("Cnt")),
                                value: ExprKind::BinaryOp {
                                    ops: vec![Operator::Add],
                                    terms: vec![
                                        ExprKind::symbolic_variable("Cnt"),
                                        ExprKind::integer_literal(1),
                                    ],
                                },
                            }],
                        ),
                        Element::action(
                            "COUNT_INLINE4",
                            vec![StmtKind::assignment("OUT", vec!["Cnt"])],
                        ),
                        Element::transition("Count", "Start", ExprKind::symbolic_variable("Reset")),
                    ],
                }]),
            },
        )]);
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    }

    #[test]
    fn first_steps_function_block_counter_fbd() {
        let src = read_resource("first_steps_function_block_counter_fbd.st");
        let expected = Ok(vec![LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: String::from("CounterFBD"),
                var_decls: vec![
                    VarInitKind::simple("Reset", "BOOL"),
                    VarInitKind::simple("OUT", "INT"),
                    VarInitKind::simple("Cnt", "INT"),
                    VarInitKind::VarInit(VarInit {
                        name: String::from("ResetCounterValue"),
                        storage_class: StorageClass::Constant,
                        initializer: Some(TypeInitializer::Simple {
                            type_name: String::from("INT"),
                            initial_value: None,
                        }),
                    }),
                    VarInitKind::simple("_TMP_ADD4_OUT", "INT"),
                    VarInitKind::simple("_TMP_SEL7_OUT", "INT"),
                ],
                body: FunctionBlockBody::Statements(vec![
                    StmtKind::assignment("Cnt", vec!["_TMP_SEL7_OUT"]),
                    StmtKind::assignment("OUT", vec!["Cnt"]),
                ]),
            },
        )]);
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    }

    #[test]
    fn first_steps_function_declaration() {
        let src = read_resource("first_steps_func_avg_val.st");
        let expected = Ok(vec![LibraryElement::FunctionDeclaration(
            FunctionDeclaration {
                name: String::from("AverageVal"),
                return_type: String::from("REAL"),
                var_decls: vec![
                    VarInit::simple("Cnt1", "INT"),
                    VarInit::simple("Cnt2", "INT"),
                    VarInit::simple("Cnt3", "INT"),
                    VarInit::simple("Cnt4", "INT"),
                    VarInit::simple("Cnt5", "INT"),
                    VarInit {
                        name: String::from("InputsNumber"),
                        storage_class: StorageClass::Unspecified,
                        initializer: Some(TypeInitializer::Simple {
                            type_name: String::from("REAL"),
                            initial_value: Some(Initializer::Simple(Constant::RealLiteral(
                                Float {
                                    value: 5.1,
                                    data_type: None,
                                },
                            ))),
                        }),
                    },
                ],
                body: vec![StmtKind::Assignment {
                    target: Variable::SymbolicVariable(String::from("AverageVal")),
                    value: ExprKind::BinaryOp {
                        // TODO This operator is incorrect
                        ops: vec![Operator::Mul],
                        terms: vec![
                            ExprKind::Function {
                                name: String::from("INT_TO_REAL"),
                                param_assignment: vec![ParamAssignment::Input {
                                    name: None,
                                    expr: ExprKind::BinaryOp {
                                        ops: vec![Operator::Add],
                                        terms: vec![
                                            ExprKind::symbolic_variable("Cnt1"),
                                            ExprKind::symbolic_variable("Cnt2"),
                                            ExprKind::symbolic_variable("Cnt3"),
                                            ExprKind::symbolic_variable("Cnt4"),
                                            ExprKind::symbolic_variable("Cnt5"),
                                        ],
                                    },
                                }],
                            },
                            ExprKind::symbolic_variable("InputsNumber"),
                        ],
                    },
                }],
            },
        )]);
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
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
    fn first_steps_program_declaration() {
        let src = read_resource("first_steps_program.st");
        let expected = Ok(vec![LibraryElement::ProgramDeclaration(
            ProgramDeclaration {
                type_name: String::from("plc_prg"),
                var_declarations: vec![
                    VarInitKind::VarInit(VarInit::simple("Reset", "BOOL")),
                    VarInitKind::VarInit(VarInit::simple("Cnt1", "INT")),
                    VarInitKind::VarInit(VarInit::simple("Cnt2", "INT")),
                    VarInitKind::VarInit(VarInit::simple("Cnt3", "INT")),
                    VarInitKind::VarInit(VarInit::simple("Cnt4", "INT")),
                    VarInitKind::VarInit(VarInit::simple("Cnt5", "INT")),
                    // TODO this are being understood as enumerated types not function blocks
                    VarInitKind::VarInit(VarInit::late_bound("CounterST0", "CounterST")),
                    VarInitKind::VarInit(VarInit::late_bound("CounterFBD0", "CounterFBD")),
                    VarInitKind::VarInit(VarInit::late_bound("CounterSFC0", "CounterSFC")),
                    VarInitKind::VarInit(VarInit::late_bound("CounterIL0", "CounterIL")),
                    VarInitKind::VarInit(VarInit::late_bound("CounterLD0", "CounterLD")),
                    VarInitKind::VarInit(VarInit::simple("AVCnt", "REAL")),
                    VarInitKind::VarInit(VarInit::simple("_TMP_AverageVal17_OUT", "REAL")),
                ],
                body: FunctionBlockBody::Statements(vec![
                    StmtKind::fb_call_mapped("CounterST0", vec![("Reset", "Reset")]),
                    StmtKind::assignment("Cnt1", vec!["CounterST0", "OUT"]),
                    StmtKind::fb_assign(
                        "AverageVal",
                        vec!["Cnt1", "Cnt2", "Cnt3", "Cnt4", "Cnt5"],
                        "_TMP_AverageVal17_OUT",
                    ),
                    StmtKind::assignment("AVCnt", vec!["_TMP_AverageVal17_OUT"]),
                    StmtKind::fb_call_mapped("CounterFBD0", vec![("Reset", "Reset")]),
                    StmtKind::assignment("Cnt2", vec!["CounterFBD0", "OUT"]),
                    StmtKind::fb_call_mapped("CounterSFC0", vec![("Reset", "Reset")]),
                    StmtKind::assignment("Cnt3", vec!["CounterSFC0", "OUT"]),
                    StmtKind::fb_call_mapped("CounterIL0", vec![("Reset", "Reset")]),
                    StmtKind::assignment("Cnt4", vec!["CounterIL0", "OUT"]),
                    StmtKind::fb_call_mapped("CounterLD0", vec![("Reset", "Reset")]),
                    StmtKind::assignment("Cnt5", vec!["CounterLD0", "Out"]),
                ]),
            },
        )]);
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    }

    #[test]
    fn first_steps_configuration() {
        let src = read_resource("first_steps_configuration.st");
        let expected = Ok(vec![LibraryElement::ConfigurationDeclaration(
            ConfigurationDeclaration {
                name: String::from("config"),
                global_var: vec![Declaration {
                    name: String::from("ResetCounterValue"),
                    storage_class: StorageClass::Constant,
                    at: None,
                    initializer: Option::Some(TypeInitializer::Simple {
                        type_name: String::from("INT"),
                        initial_value: Option::Some(Initializer::Simple(Constant::IntegerLiteral(
                            17,
                        ))),
                    }),
                }],
                resource_decl: vec![ResourceDeclaration {
                    name: String::from("resource1"),
                    tasks: vec![TaskConfiguration {
                        name: String::from("plc_task"),
                        priority: 1,
                        interval: Option::Some(Duration::new(0, 100_000_000)),
                    }],
                    programs: vec![ProgramConfiguration {
                        name: String::from("plc_task_instance"),
                        task_name: Option::Some(String::from("plc_task")),
                        type_name: String::from("plc_prg"),
                    }],
                }],
            },
        )]);
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    }
}
