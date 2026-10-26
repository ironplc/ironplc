//! Tests of programs whose declarations include some that are no fields of
//! the program's instance (REQ-LOW-lowering-170), compiled from the lowered
//! program: an access path, a located variable with no name, and the global
//! variables a PLCopen XML program declares in its interface.

use ironplc_dsl::core::FileId;
use ironplc_sources::parsers::xml_parser;

use crate::common::{compile_analyzed, compile_to_run, Snapshot};
use crate::compile_body::{assert_compiles, compile_and_run_agreeing, options};

/// A program with an access path and a located variable with no name, which
/// no statement names, among its declarations.
const DECLARATIONS: &str = "
    PROGRAM main
      VAR k : DINT; END_VAR
      VAR AT %IX0.0 : BOOL; AT %QX0.1 : BOOL; END_VAR
      VAR n : DINT := 4; END_VAR
      VAR_ACCESS acc : k : DINT READ_WRITE; END_VAR
      k := n + 1;
    END_PROGRAM";

/// The program's statements lower and compile, and every backend leaves its
/// variables with the same values.
#[test]
fn run_when_program_declares_access_path_and_unnamed_located_then_lowered() {
    assert_compiles(DECLARATIONS);
    let snapshot = compile_and_run_agreeing(DECLARATIONS);
    assert_eq!(snapshot.read_as::<i32>("k"), 5);
}

/// A PLCopen XML program whose interface declares a global variable, which
/// its body reads and writes as a global.
const PLCOPEN_GLOBALS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="Test" productName="Test" productVersion="1.0" creationDateTime="2024-01-01T00:00:00"/>
  <contentHeader name="TestProject">
    <coordinateInfo>
      <fbd><scaling x="1" y="1"/></fbd>
      <ld><scaling x="1" y="1"/></ld>
      <sfc><scaling x="1" y="1"/></sfc>
    </coordinateInfo>
  </contentHeader>
  <types>
    <dataTypes/>
    <pous>
      <pou name="Main" pouType="program">
        <interface>
          <localVars>
            <variable name="k"><type><DINT/></type></variable>
          </localVars>
          <globalVars>
            <variable name="g"><type><DINT/></type></variable>
          </globalVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">g := g + 2;
k := g + 1;</xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;

/// A global variable a PLCopen program declares in its interface is a
/// global, read and written as a global.
#[test]
fn run_when_plcopen_program_declares_global_then_lowered() {
    let options = options();
    let library = xml_parser::parse(PLCOPEN_GLOBALS, &FileId::default(), &options).unwrap();
    let (library, context) = ironplc_analyzer::stages::analyze(&[&library], &options).unwrap();
    compile_to_run(&library, &context, &options).unwrap();
    let container = compile_analyzed(&library, &context, &options).expect("every statement lowers");
    let snapshot = Snapshot::run_compiled(container);
    assert_eq!(snapshot.read_as::<i32>("g"), 2);
    assert_eq!(snapshot.read_as::<i32>("k"), 3);
}
