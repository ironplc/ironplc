//! CONFIGURATION parsing: the resources a configuration declares.

use super::common::*;

/// The configuration from issue #1856: two resources, each with its own task
/// and program instance.
const TWO_RESOURCES: &str = "
    CONFIGURATION config
        RESOURCE r1 ON PLC
            TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
            PROGRAM a WITH t1 : counter;
        END_RESOURCE
        RESOURCE r2 ON PLC
            TASK t2(INTERVAL := T#20ms, PRIORITY := 2);
            PROGRAM b WITH t2 : counter;
        END_RESOURCE
    END_CONFIGURATION";

#[test]
fn parse_when_two_resources_then_builds_resource_per_declaration() {
    let lib = parse_text(TWO_RESOURCES);

    let config = cast!(
        &lib.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );
    let names: Vec<&Id> = config
        .resource_decl
        .iter()
        .map(|resource| &resource.name)
        .collect();
    assert_eq!(names, vec![&Id::from("r1"), &Id::from("r2")]);
}

#[test]
fn parse_when_two_resources_then_tasks_and_programs_stay_in_their_resource() {
    let lib = parse_text(TWO_RESOURCES);

    let config = cast!(
        &lib.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );
    let second = &config.resource_decl[1];
    assert_eq!(second.tasks.len(), 1);
    assert_eq!(second.tasks[0].name, Id::from("t2"));
    assert_eq!(second.programs.len(), 1);
    assert_eq!(second.programs[0].name, Id::from("b"));
    assert_eq!(second.programs[0].task_name.as_ref(), Some(&Id::from("t2")));
}

#[test]
fn parse_when_two_resources_and_var_config_then_keeps_initializations() {
    let source = "
        CONFIGURATION config
            RESOURCE r1 ON PLC
                PROGRAM a : counter;
            END_RESOURCE
            RESOURCE r2 ON PLC
                PROGRAM b : counter;
            END_RESOURCE
            VAR_CONFIG
                r2.b.fb : FB_TYPE := (ELEM := 1);
            END_VAR
        END_CONFIGURATION";

    let lib = parse_text(source);

    let config = cast!(
        &lib.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );
    assert_eq!(config.resource_decl.len(), 2);
    assert_eq!(config.fb_inits.len(), 1);
}
