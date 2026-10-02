//! OOP extension: PROPERTY ... END_PROPERTY declarations on a function block,
//! with GET and SET accessors. Each accessor is a `MethodDeclaration`; see
//! `PropertyDeclaration`.

use super::common::*;

fn parse_fb_with(members: &str, options: &CompilerOptions) -> FunctionBlockDeclaration {
    let source = format!(
        "
FUNCTION_BLOCK FB_Motor
VAR
    _speed : REAL;
END_VAR
{members}
END_FUNCTION_BLOCK"
    );
    let library = parse_program(&source, &FileId::default(), options).unwrap();
    extract_fb(&library).clone()
}

/// The property keywords stay ordinary identifiers in standard mode, like
/// the METHOD keywords (see `methods.rs`).
#[test]
fn parse_when_standard_mode_then_property_keywords_are_valid_identifiers() {
    let program = "
FUNCTION_BLOCK FB_ALL_PROPERTY_KEYWORDS_AS_VARS
VAR
    PROPERTY : INT;
    END_PROPERTY : INT;
    END_GET : INT;
    END_SET : INT;
END_VAR

PROPERTY := 1;
END_PROPERTY := 2;
END_GET := 3;
END_SET := 4;
END_FUNCTION_BLOCK
";
    let result = parse_program(program, &FileId::default(), &CompilerOptions::default());
    assert!(
        result.is_ok(),
        "property keywords must remain valid identifiers in standard mode: {:?}",
        result.err()
    );
}

#[test]
fn parse_when_property_and_default_dialect_then_err() {
    let source = "
FUNCTION_BLOCK FB_Motor
PROPERTY Speed : REAL
GET
    Speed := 1.0;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}

#[test]
fn parse_when_property_has_get_and_set_then_accessors_are_methods() {
    let fb = parse_fb_with(
        "
PROPERTY Speed : REAL
GET
    Speed := _speed;
END_GET
SET
    _speed := Speed;
END_SET
END_PROPERTY",
        &opts_with_fb_inheritance(),
    );

    assert!(fb.methods.is_empty());
    assert_eq!(fb.properties.len(), 1);
    let property = &fb.properties[0];
    assert_eq!(property.name, Id::from("Speed"));
    let real = FunctionReturnType::Named(TypeName::from("REAL"));
    assert_eq!(property.property_type, real);

    let get = property.get.as_ref().unwrap();
    assert_eq!(get.name, Id::from("Speed"));
    assert_eq!(get.return_type, Some(real));
    assert!(get.variables.is_empty());
    assert_eq!(get.body.len(), 1);

    let set = property.set.as_ref().unwrap();
    assert_eq!(set.name, Id::from("Speed"));
    assert_eq!(set.return_type, None);
    assert_eq!(set.body.len(), 1);
    assert!(property.set_declared_variables().is_empty());
    assert_eq!(set.variables.len(), 1);
    let value = &set.variables[0];
    assert_eq!(value.identifier.symbolic_id(), Some(&Id::from("Speed")));
    assert_eq!(value.var_type, VariableType::Input);
    assert_eq!(
        value.initializer,
        InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer::bare(
            TypeName::from("REAL")
        ))
    );
}

#[test]
fn parse_when_get_only_with_var_block_then_set_is_none() {
    let fb = parse_fb_with(
        "
PROPERTY Speed : REAL
GET
VAR
    tmp : REAL;
END_VAR
    tmp := _speed;
    Speed := tmp;
END_GET
END_PROPERTY",
        &opts_with_fb_inheritance(),
    );

    let property = &fb.properties[0];
    let get = property.get.as_ref().unwrap();
    assert_eq!(get.variables.len(), 1);
    assert_eq!(get.body.len(), 2);
    assert!(property.set.is_none());
}

#[test]
fn parse_when_set_only_with_var_block_then_declared_variables_exclude_implicit_input() {
    let fb = parse_fb_with(
        "
PROPERTY Speed : REAL
SET
VAR
    tmp : REAL;
END_VAR
    tmp := Speed;
    _speed := tmp;
END_SET
END_PROPERTY",
        &opts_with_fb_inheritance(),
    );

    let property = &fb.properties[0];
    assert!(property.get.is_none());
    let declared = property.set_declared_variables();
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].identifier.symbolic_id(), Some(&Id::from("tmp")));
    assert_eq!(property.set.as_ref().unwrap().variables.len(), 2);
}

#[test]
fn parse_when_string_property_then_set_input_is_string_with_length() {
    let fb = parse_fb_with(
        "
PROPERTY Name : STRING[20]
SET
    _speed := 0.0;
END_SET
END_PROPERTY",
        &opts_with_fb_inheritance(),
    );

    let set = fb.properties[0].set.as_ref().unwrap();
    let InitialValueAssignmentKind::String(string) = &set.variables[0].initializer else {
        panic!(
            "expected a STRING input, got {:?}",
            set.variables[0].initializer
        );
    };
    assert_eq!(string.width, StringType::String);
    assert!(string.length.is_some());
}

#[test]
fn parse_when_accessor_keywords_lowercase_then_ok() {
    let fb = parse_fb_with(
        "
property Speed : REAL
get
    Speed := _speed;
end_get
set
    _speed := Speed;
end_set
end_property",
        &opts_with_fb_inheritance(),
    );

    assert!(fb.properties[0].get.is_some());
    assert!(fb.properties[0].set.is_some());
}

/// TwinCAT stores methods and properties in file order, so they interleave.
#[test]
fn parse_when_methods_and_properties_interleave_then_each_list_keeps_its_order() {
    let fb = parse_fb_with(
        "
METHOD Start
    _speed := 1.0;
END_METHOD
PROPERTY Speed : REAL
GET
    Speed := _speed;
END_GET
END_PROPERTY
METHOD Stop
    _speed := 0.0;
END_METHOD
PROPERTY Running : BOOL
GET
    Running := _speed > 0.0;
END_GET
END_PROPERTY",
        &opts_with_fb_inheritance(),
    );

    let methods: Vec<_> = fb.methods.iter().map(|m| m.name.to_string()).collect();
    let properties: Vec<_> = fb.properties.iter().map(|p| p.name.to_string()).collect();
    assert_eq!(methods, ["Start", "Stop"]);
    assert_eq!(properties, ["Speed", "Running"]);
}

/// `GET`/`SET` are keywords only right after a property header. Under the
/// TwinCAT dialect, where properties are enabled, they stay usable as names:
/// the `RS` block's input is called `SET`.
#[test]
fn parse_when_twincat_dialect_then_get_and_set_remain_identifiers() {
    let source = "
FUNCTION_BLOCK FB_Latch
VAR
    fbLatch : RS;
    Get : BOOL;
END_VAR
fbLatch(SET := TRUE, RESET1 := FALSE);
Get := fbLatch.Q1;
PROPERTY Set : BOOL
GET
    Set := Get;
END_GET
END_PROPERTY
END_FUNCTION_BLOCK";
    let options = CompilerOptions::from_dialect(Dialect::TwinCat);
    let library = parse_program(source, &FileId::default(), &options).unwrap();
    let fb = extract_fb(&library);
    assert_eq!(fb.properties[0].name, Id::from("Set"));
}
