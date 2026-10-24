//! End-to-end tests of each body's prologue emitted from the lowered
//! program (REQ-LOW-codegen-202): a program's `VAR_TEMP` starts again at the
//! head of every scan, as every body's does (REQ-LOW-lowering-201).

use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_sources::parsers::xml_parser;

use crate::common::{compile_to_run, run_compiled_scans};

/// A PLCopen XML program, which may declare a `VAR_TEMP`, unlike one written
/// in Structured Text: each scan reads an integer and a string `VAR_TEMP`,
/// then writes it. PLCopen XML gives a variable no initial value yet, so
/// each starts at its type's default.
const PLCOPEN_VAR_TEMP: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
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
            <variable name="count"><type><INT/></type></variable>
            <variable name="length"><type><INT/></type></variable>
          </localVars>
          <tempVars>
            <variable name="t"><type><INT/></type></variable>
            <variable name="s"><type><string length="8"/></type></variable>
          </tempVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">count := t;
length := LEN(s);
t := t + 1;
s := CONCAT(s, 'x');</xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;

/// `count` and `length` after each of two scans of `PLCOPEN_VAR_TEMP`.
fn two_scans() -> [(i64, i64); 2] {
    let options = CompilerOptions::default();
    let library = xml_parser::parse(PLCOPEN_VAR_TEMP, &FileId::default(), &options).unwrap();
    let (library, context) = ironplc_analyzer::stages::analyze(&[&library], &options).unwrap();
    let program = compile_to_run(&library, &context, &options).unwrap();
    run_compiled_scans(&program, |session| {
        [0, 1000].map(|time_us| {
            session.scan(time_us).unwrap();
            (session.read_as("count"), session.read_as("length"))
        })
    })
}

/// A program's `VAR_TEMP` starts again at the head of every scan, so the
/// second scan reads what the first did.
#[test]
fn end_to_end_when_program_var_temp_read_before_written_then_starts_again_each_scan() {
    assert_eq!(two_scans(), [(0, 0), (0, 0)]);
}
