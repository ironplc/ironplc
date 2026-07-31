//! VAR_TEMP and mixed located / non-located variable blocks.

use super::common::*;

#[test]
fn parse_when_function_with_var_temp_then_succeeds() {
    let lib = parse_text(
        "FUNCTION my_func : DINT
VAR_INPUT
    a : DINT;
END_VAR
VAR_TEMP
    temp : DINT;
END_VAR
    temp := a * 2;
    my_func := temp;
END_FUNCTION",
    );
    let func = cast!(&lib.elements[0], LibraryElementKind::FunctionDeclaration);
    assert_eq!(func.variables.len(), 2);
    assert_eq!(func.variables[0].var_type, VariableType::Input);
    assert_eq!(func.variables[1].var_type, VariableType::VarTemp);
}

#[test]
fn parse_when_function_block_with_var_temp_then_succeeds() {
    let lib = parse_text(
        "FUNCTION_BLOCK my_fb
VAR_TEMP
    t : INT;
END_VAR
    t := 42;
END_FUNCTION_BLOCK",
    );
    let fb = cast!(
        &lib.elements[0],
        LibraryElementKind::FunctionBlockDeclaration
    );
    assert_eq!(fb.variables.len(), 1);
    assert_eq!(fb.variables[0].var_type, VariableType::VarTemp);
}

#[test]
fn parse_when_program_with_var_temp_then_fails() {
    let source = "PROGRAM main
VAR_TEMP
    t : INT;
END_VAR
    t := 42;
END_PROGRAM";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}

#[test]
fn parse_when_program_mixed_located_and_non_located_vars_then_ok() {
    let lib = parse_text(
        "PROGRAM main
VAR
    Motor : BOOL;
    xStart AT %IX0.0 : BOOL;
    xStop AT %IX0.1 : BOOL;
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(prog.variables.len(), 3);
    assert!(matches!(
        &prog.variables[0].identifier,
        VariableIdentifier::Symbol(_)
    ));
    assert!(matches!(
        &prog.variables[1].identifier,
        VariableIdentifier::Direct(_)
    ));
    assert!(matches!(
        &prog.variables[2].identifier,
        VariableIdentifier::Direct(_)
    ));
}

#[test]
fn parse_when_program_mixed_vars_with_retain_qualifier_then_ok() {
    let lib = parse_text(
        "PROGRAM main
VAR RETAIN
    counter : INT;
    saved AT %MW0 : INT;
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(prog.variables.len(), 2);
    assert_eq!(prog.variables[0].qualifier, DeclarationQualifier::Retain);
    assert_eq!(prog.variables[1].qualifier, DeclarationQualifier::Retain);
}

#[test]
fn parse_when_program_motor_control_style_then_ok() {
    let lib = parse_text(
        "TYPE
  MotorState : (STOPPED, RUNNING, FAULTED);
END_TYPE

FUNCTION_BLOCK FB_MotorControl
  VAR_INPUT
    START_PB : BOOL;
    STOP_PB : BOOL;
    OL_CONTACT : BOOL;
    FAULT_RESET : BOOL;
  END_VAR
  VAR_OUTPUT
    CONTACTOR : BOOL;
    RUN_LAMP : BOOL;
    FAULT_LAMP : BOOL;
  END_VAR
  VAR
    Seal : BOOL;
  END_VAR

  IF NOT OL_CONTACT THEN
    Seal := FALSE;
  ELSE
    IF START_PB AND STOP_PB THEN
      Seal := TRUE;
    END_IF;
    IF NOT STOP_PB THEN
      Seal := FALSE;
    END_IF;
  END_IF;

  CONTACTOR := Seal;
  RUN_LAMP := CONTACTOR;
  FAULT_LAMP := NOT OL_CONTACT;
END_FUNCTION_BLOCK

PROGRAM PLC_PRG
  VAR
    Motor : FB_MotorControl;
    xStart AT %IX0.0 : BOOL;
    xStop AT %IX0.1 : BOOL;
    xOverload AT %IX0.2 : BOOL;
    xReset AT %IX0.3 : BOOL;
    yContactor AT %QX0.0 : BOOL;
    yRunLamp AT %QX0.1 : BOOL;
    yFaultLamp AT %QX0.2 : BOOL;
  END_VAR

  Motor(
    START_PB := xStart,
    STOP_PB := xStop,
    OL_CONTACT := xOverload,
    FAULT_RESET := xReset,
    CONTACTOR => yContactor,
    RUN_LAMP => yRunLamp,
    FAULT_LAMP => yFaultLamp
  );
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[2], LibraryElementKind::ProgramDeclaration);
    assert_eq!(prog.variables.len(), 8);
    assert!(matches!(
        &prog.variables[0].identifier,
        VariableIdentifier::Symbol(_)
    ));
    assert!(matches!(
        &prog.variables[1].identifier,
        VariableIdentifier::Direct(_)
    ));
}

// ---------------------------------------------------------------------
// CODESYS/TwinCAT FB-instance call-style initializer.
// See specs/plans/2026-07-31-twincat-inline-fb-call-style-initializer.md.
// ---------------------------------------------------------------------

#[test]
fn parse_when_fb_call_style_init_named_and_positional_then_parses_call_params() {
    // Matches real usage found in a private test corpus: both named
    // (comm := comm) and positional (THIS) arguments in the same
    // call-style initializer.
    let source = "
FUNCTION_BLOCK FB_Comm
VAR_INPUT
    retries : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Example
VAR
    comm : FB_Comm(retries := 3, THIS);
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_text(source);

    let fb = cast!(
        &library.elements[1],
        LibraryElementKind::FunctionBlockDeclaration
    );
    assert_eq!(fb.variables.len(), 1);
    let fb_init = cast!(
        &fb.variables[0].initializer,
        InitialValueAssignmentKind::FunctionBlock
    );
    assert_eq!(fb_init.type_name.to_string(), "FB_Comm");
    assert!(fb_init.init.is_empty());
    let call_params = fb_init
        .call_params
        .as_ref()
        .expect("call_params must be Some");
    assert_eq!(call_params.len(), 2);
    assert!(matches!(call_params[0], ParamAssignmentKind::NamedInput(_)));
    assert!(matches!(
        call_params[1],
        ParamAssignmentKind::PositionalInput(_)
    ));
}

#[test]
fn parse_when_fb_call_style_init_empty_parens_then_parses() {
    let source = "
FUNCTION_BLOCK FB_Comm
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Example
VAR
    comm : FB_Comm();
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_text(source);

    let fb = cast!(
        &library.elements[1],
        LibraryElementKind::FunctionBlockDeclaration
    );
    let fb_init = cast!(
        &fb.variables[0].initializer,
        InitialValueAssignmentKind::FunctionBlock
    );
    assert_eq!(fb_init.call_params.as_ref().map(|p| p.len()), Some(0));
}

#[test]
fn parse_when_fb_bare_decl_then_no_call_params() {
    // Regression: an ordinary bare FB instance declaration (no
    // initializer at all) must be unaffected -- it continues to flow
    // through the existing late-bound-resolution path, not the new
    // call-style rule (which requires the parens unconditionally).
    let source = "
FUNCTION_BLOCK FB_Comm
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Example
VAR
    comm : FB_Comm;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_text(source);

    let fb = cast!(
        &library.elements[1],
        LibraryElementKind::FunctionBlockDeclaration
    );
    // Bare declarations resolve to LateResolvedType at parse time
    // (kind is only known once the type environment is built), not
    // eagerly to FunctionBlock.
    assert!(matches!(
        &fb.variables[0].initializer,
        InitialValueAssignmentKind::LateResolvedType(_)
    ));
}

#[test]
fn parse_when_fb_struct_init_then_still_parses() {
    // Regression: the standard `:= (member := value)` named-struct-init
    // form must still parse unchanged.
    let source = "
FUNCTION_BLOCK FB_Comm
VAR_INPUT
    retries : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Example
VAR
    comm : FB_Comm := (retries := 3);
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_text(source);

    let fb = cast!(
        &library.elements[1],
        LibraryElementKind::FunctionBlockDeclaration
    );
    assert_eq!(fb.variables.len(), 1);
}
