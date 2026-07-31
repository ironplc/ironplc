//! TASK configuration parsing.

use super::common::*;

#[test]
fn parse_when_task_with_interval_and_priority_then_builds_structure() {
    let source = "
        CONFIGURATION config
            RESOURCE resource1 ON PLC
                TASK my_task(INTERVAL := T#50ms, PRIORITY := 3);
                PROGRAM instance1 WITH my_task : my_prg;
            END_RESOURCE
        END_CONFIGURATION";

    let lib = parse_text(source);
    let config = cast!(
        &lib.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );
    let task = &config.resource_decl[0].tasks[0];
    assert_eq!(task.name, Id::from("my_task"));
    assert_eq!(task.priority, 3);
    assert!(task.interval.is_some());
    assert!(task.single.is_none());
}

#[test]
fn parse_when_task_with_single_constant_then_builds_structure() {
    let source = "
        CONFIGURATION config
            RESOURCE resource1 ON PLC
                TASK event_task(SINGLE := 1, PRIORITY := 0);
                PROGRAM instance1 WITH event_task : my_prg;
            END_RESOURCE
        END_CONFIGURATION";

    let lib = parse_text(source);
    let config = cast!(
        &lib.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );
    let task = &config.resource_decl[0].tasks[0];
    assert_eq!(task.name, Id::from("event_task"));
    assert_eq!(task.priority, 0);
    assert!(task.interval.is_none());
    assert!(task.single.is_some());
    assert!(matches!(task.single, Some(DataSourceKind::Constant(_))));
}

#[test]
fn parse_when_task_with_single_and_interval_then_builds_structure() {
    let source = "
        CONFIGURATION config
            RESOURCE resource1 ON PLC
                TASK my_task(SINGLE := 1, INTERVAL := T#100ms, PRIORITY := 1);
                PROGRAM instance1 WITH my_task : my_prg;
            END_RESOURCE
        END_CONFIGURATION";

    let lib = parse_text(source);
    let config = cast!(
        &lib.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );
    let task = &config.resource_decl[0].tasks[0];
    assert_eq!(task.name, Id::from("my_task"));
    assert_eq!(task.priority, 1);
    assert!(task.interval.is_some());
    assert!(task.single.is_some());
}

#[test]
fn parse_when_task_with_priority_only_then_builds_structure() {
    let source = "
        CONFIGURATION config
            RESOURCE resource1 ON PLC
                TASK free_task(PRIORITY := 5);
                PROGRAM instance1 WITH free_task : my_prg;
            END_RESOURCE
        END_CONFIGURATION";

    let lib = parse_text(source);
    let config = cast!(
        &lib.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );
    let task = &config.resource_decl[0].tasks[0];
    assert_eq!(task.name, Id::from("free_task"));
    assert_eq!(task.priority, 5);
    assert!(task.interval.is_none());
    assert!(task.single.is_none());
}

#[test]
fn parse_when_task_with_interval_non_duration_then_parse_error() {
    let source = "
        CONFIGURATION config
            RESOURCE resource1 ON PLC
                TASK bad_task(INTERVAL := 42, PRIORITY := 1);
                PROGRAM instance1 WITH bad_task : my_prg;
            END_RESOURCE
        END_CONFIGURATION";

    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}
