//! The compiler as individual stages (to enable testing).
use crate::{
    ironplc_dsl::dsl::Library, rule_constant_vars_initialized, rule_enumeration_values_unique,
    rule_program_task_definition_exists, rule_use_declared_enumerated_value, rule_use_declared_fb,
    rule_use_declared_symbolic_var, type_resolver,
};

/// Parse combines lexical and sematic analysis (stages 1 & 2).
///
/// The stage takes text and returns a library of domain specific
/// objects including resolution of all ambiguous syntax.
///
/// Returns `Ok(Library)` if parsing succeeded.
/// Returns `Err(String)` if parsing did not succeed.
pub fn parse(source: &str) -> Result<Library, String> {
    let library = ironplc_parser::parse_program(source)?;

    // Resolve the late bound type declarations, replacing with
    // the type-specific declarations. This just simplifies
    // code generation because we know the type of every declaration
    // exactly
    type_resolver::apply(library).map_err(|err| err.to_string())
}

/// Semantic implements semantic analysis (stage 3).
///
/// Returns `Ok(())` if the library is free of semantic errors.
/// Returns `Err(String)` if the library contains a semantic error.
pub fn semantic(library: &Library) -> Result<(), String> {
    rule_use_declared_symbolic_var::apply(&library)?;
    rule_use_declared_enumerated_value::apply(&library)?;
    rule_use_declared_fb::apply(&library)?;
    rule_constant_vars_initialized::apply(&library)?;
    rule_enumeration_values_unique::apply(&library)?;
    rule_program_task_definition_exists::apply(&library)?;

    // 1. Check all identifiers defined (need scope)
    // 2. Type checking
    // 3. Check types only defined once
    // 4. Check reserved identifiers
    // 5. Check assignment types compatible
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_helpers;

    use super::*;

    use ironplc_dsl::ast::*;
    use ironplc_dsl::dsl::*;
    use ironplc_dsl::sfc::*;
    use test_helpers::*;

    use time::Duration;

    #[test]
    fn parse_when_first_steps_then_result_is_ok() {
        let src = read_resource("first_steps.st");
        let res = ironplc_parser::parse_program(src.as_str());
        assert!(res.is_ok())
    }

    #[test]
    fn parse_when_first_steps_data_type_decl_then_builds_structure() {
        let src = read_resource("first_steps_data_type_decl.st");
        let expected = new_library(LibraryElement::DataTypeDeclaration(vec![
            EnumerationDeclaration {
                name: Id::from("LOGLEVEL"),
                spec: EnumeratedSpecificationKind::Values(vec![
                    Id::from("CRITICAL"),
                    Id::from("WARNING"),
                    Id::from("INFO"),
                    Id::from("DEBUG"),
                ]),
                default: Option::Some(Id::from("INFO")),
            },
        ]));
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    }

    #[test]
    fn parse_when_first_steps_function_block_logger_then_test_apply_when_names_correct_then_passes()
    {
        let src = read_resource("first_steps_function_block_logger.st");
        let expected = new_library(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("LOGGER"),
                inputs: vec![
                    VarInitDecl::simple("TRIG", "BOOL"),
                    VarInitDecl::simple("MSG", "STRING"),
                    VarInitDecl::enumerated("LEVEL", "LOGLEVEL", "INFO"),
                ],
                outputs: vec![],
                inouts: vec![],
                vars: vec![VarInitDecl::simple("TRIG0", "BOOL")],
                externals: vec![],
                body: FunctionBlockBody::stmts(vec![
                    StmtKind::if_then(
                        ExprKind::Compare {
                            op: CompareOp::And,
                            terms: vec![
                                ExprKind::symbolic_variable("TRIG"),
                                ExprKind::UnaryOp {
                                    op: UnaryOp::Not,
                                    term: ExprKind::boxed_symbolic_variable("TRIG0"),
                                },
                            ],
                        },
                        vec![],
                    ),
                    StmtKind::assignment(
                        Variable::symbolic("TRIG0"),
                        ExprKind::symbolic_variable("TRIG"),
                    ),
                ]),
            },
        ));
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    }

    #[test]
    fn parse_when_first_steps_function_block_counter_sfc_then_builds_structure() {
        let src = read_resource("first_steps_function_block_counter_sfc.st");
        let expected = new_library(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("CounterSFC"),
                inputs: vec![VarInitDecl::simple("Reset", "BOOL")],
                outputs: vec![VarInitDecl::simple("OUT", "INT")],
                inouts: vec![],
                vars: vec![VarInitDecl::simple("Cnt", "INT")],
                externals: vec![VarInitDecl {
                    name: Id::from("ResetCounterValue"),
                    storage_class: StorageClass::Constant,
                    initializer: Some(TypeInitializer::Simple {
                        type_name: Id::from("INT"),
                        initial_value: None,
                    }),
                }],
                body: FunctionBlockBody::sfc(vec![Network {
                    initial_step: Element::InitialStep {
                        name: Id::from("Start"),
                        action_associations: vec![],
                    },
                    elements: vec![
                        Element::transition(
                            "Start",
                            "ResetCounter",
                            ExprKind::symbolic_variable("Reset"),
                        ),
                        Element::Step {
                            name: Id::from("ResetCounter"),
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
                            vec![StmtKind::simple_assignment(
                                "Cnt",
                                vec!["ResetCounterValue"],
                            )],
                        ),
                        Element::action(
                            "RESETCOUNTER_INLINE2",
                            vec![StmtKind::simple_assignment("OUT", vec!["Cnt"])],
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
                            name: Id::from("Count"),
                            action_associations: vec![
                                ActionAssociation::new("COUNT_INLINE3", Some(ActionQualifier::N)),
                                ActionAssociation::new("COUNT_INLINE4", Some(ActionQualifier::N)),
                            ],
                        },
                        Element::action(
                            "COUNT_INLINE3",
                            vec![StmtKind::assignment(
                                Variable::symbolic("Cnt"),
                                ExprKind::BinaryOp {
                                    ops: vec![Operator::Add],
                                    terms: vec![
                                        ExprKind::symbolic_variable("Cnt"),
                                        ExprKind::integer_literal(1),
                                    ],
                                },
                            )],
                        ),
                        Element::action(
                            "COUNT_INLINE4",
                            vec![StmtKind::simple_assignment("OUT", vec!["Cnt"])],
                        ),
                        Element::transition("Count", "Start", ExprKind::symbolic_variable("Reset")),
                    ],
                }]),
            },
        ));
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    }

    #[test]
    fn parse_when_first_steps_function_block_counter_fbd_then_builds_structure() {
        let src = read_resource("first_steps_function_block_counter_fbd.st");
        let expected = new_library(LibraryElement::FunctionBlockDeclaration(
            FunctionBlockDeclaration {
                name: Id::from("CounterFBD"),
                inputs: vec![VarInitDecl::simple("Reset", "BOOL")],
                outputs: vec![VarInitDecl::simple("OUT", "INT")],
                inouts: vec![],
                vars: vec![
                    VarInitDecl::simple("Cnt", "INT"),
                    VarInitDecl::simple("_TMP_ADD4_OUT", "INT"),
                    VarInitDecl::simple("_TMP_SEL7_OUT", "INT"),
                ],
                externals: vec![VarInitDecl {
                    name: Id::from("ResetCounterValue"),
                    storage_class: StorageClass::Constant,
                    initializer: Some(TypeInitializer::Simple {
                        type_name: Id::from("INT"),
                        initial_value: None,
                    }),
                }],
                body: FunctionBlockBody::stmts(vec![
                    StmtKind::simple_assignment("Cnt", vec!["_TMP_SEL7_OUT"]),
                    StmtKind::simple_assignment("OUT", vec!["Cnt"]),
                ]),
            },
        ));
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    }

    #[test]
    fn parse_when_first_steps_func_avg_val_then_builds_structure() {
        let src = read_resource("first_steps_func_avg_val.st");
        let expected = new_library(LibraryElement::FunctionDeclaration(FunctionDeclaration {
            name: Id::from("AverageVal"),
            return_type: Id::from("REAL"),
            inputs: vec![
                VarInitDecl::simple("Cnt1", "INT"),
                VarInitDecl::simple("Cnt2", "INT"),
                VarInitDecl::simple("Cnt3", "INT"),
                VarInitDecl::simple("Cnt4", "INT"),
                VarInitDecl::simple("Cnt5", "INT"),
            ],
            outputs: vec![],
            inouts: vec![],
            vars: vec![VarInitDecl {
                name: Id::from("InputsNumber"),
                storage_class: StorageClass::Unspecified,
                initializer: Some(TypeInitializer::Simple {
                    type_name: Id::from("REAL"),
                    initial_value: Some(Initializer::Simple(Constant::RealLiteral(Float {
                        value: 5.1,
                        data_type: None,
                    }))),
                }),
            }],
            externals: vec![],
            body: vec![StmtKind::assignment(
                Variable::symbolic("AverageVal"),
                ExprKind::BinaryOp {
                    // TODO This operator is incorrect
                    ops: vec![Operator::Mul],
                    terms: vec![
                        ExprKind::Function {
                            name: Id::from("INT_TO_REAL"),
                            param_assignment: vec![ParamAssignment::positional(
                                ExprKind::BinaryOp {
                                    ops: vec![Operator::Add],
                                    terms: vec![
                                        ExprKind::symbolic_variable("Cnt1"),
                                        ExprKind::symbolic_variable("Cnt2"),
                                        ExprKind::symbolic_variable("Cnt3"),
                                        ExprKind::symbolic_variable("Cnt4"),
                                        ExprKind::symbolic_variable("Cnt5"),
                                    ],
                                },
                            )],
                        },
                        ExprKind::symbolic_variable("InputsNumber"),
                    ],
                },
            )],
        }));
        let program = ironplc_parser::parse_program(src.as_str());
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
            inputs: vec![VarInitDecl::simple("Reset", "BOOL")],
            outputs: vec![
                VarInitDecl::simple("Cnt1", "INT"),
                VarInitDecl::simple("Cnt2", "INT"),
                VarInitDecl::simple("Cnt3", "INT"),
                VarInitDecl::simple("Cnt4", "INT"),
                VarInitDecl::simple("Cnt5", "INT"),
            ],
            inouts: vec![],
            vars: vec![
                // TODO this are being understood as enumerated types not function blocks
                VarInitDecl::late_bound("CounterST0", "CounterST"),
                VarInitDecl::late_bound("CounterFBD0", "CounterFBD"),
                VarInitDecl::late_bound("CounterSFC0", "CounterSFC"),
                VarInitDecl::late_bound("CounterIL0", "CounterIL"),
                VarInitDecl::late_bound("CounterLD0", "CounterLD"),
                VarInitDecl::simple("AVCnt", "REAL"),
                VarInitDecl::simple("_TMP_AverageVal17_OUT", "REAL"),
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
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    }

    #[test]
    fn parse_when_first_steps_configuration_then_builds_structure() {
        let src = read_resource("first_steps_configuration.st");
        let expected = new_library(LibraryElement::ConfigurationDeclaration(
            ConfigurationDeclaration {
                name: Id::from("config"),
                global_var: vec![Declaration {
                    name: Id::from("ResetCounterValue"),
                    storage_class: StorageClass::Constant,
                    at: None,
                    initializer: Option::Some(TypeInitializer::Simple {
                        type_name: Id::from("INT"),
                        initial_value: Option::Some(Initializer::Simple(Constant::IntegerLiteral(
                            17,
                        ))),
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
                    }],
                }],
            },
        ));
        assert_eq!(ironplc_parser::parse_program(src.as_str()), expected)
    }
}
