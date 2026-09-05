import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import { ProblemCode, PROBLEM_MESSAGES, formatProblem } from '../../problems';
import { DEBUG_SERVER_BINARY } from '../../debugAdapterLogic';

suite('ProblemCode', () => {
  test('ProblemCode_when_accessed_then_returns_expected_codes', () => {
    assert.strictEqual(ProblemCode.NoCompiler, 'E0001');
    assert.strictEqual(ProblemCode.ViewerCompilerNotFound, 'E0002');
    assert.strictEqual(ProblemCode.DisassemblyFailed, 'E0003');
  });

  test('ProblemCode_when_debug_codes_accessed_then_returns_expected_codes', () => {
    assert.strictEqual(ProblemCode.DebugNoProgram, 'E0004');
    assert.strictEqual(ProblemCode.DebugProgramNotDebuggable, 'E0005');
    assert.strictEqual(ProblemCode.DebugCompileFailed, 'E0006');
    assert.strictEqual(ProblemCode.DebugServerNotFound, 'E0007');
  });

  test('PROBLEM_MESSAGES_when_debug_server_not_found_then_names_the_binary', () => {
    // problem-codes.csv is not TypeScript and cannot import the constant, so
    // this is what keeps the generated message in step with the binary name.
    assert.ok(
      PROBLEM_MESSAGES[ProblemCode.DebugServerNotFound].includes(DEBUG_SERVER_BINARY),
      `E0007 must name ${DEBUG_SERVER_BINARY}, but says: `
      + PROBLEM_MESSAGES[ProblemCode.DebugServerNotFound],
    );
  });

  test('PROBLEM_MESSAGES_when_accessed_then_has_entry_for_each_code', () => {
    for (const code of Object.values(ProblemCode)) {
      assert.ok(PROBLEM_MESSAGES[code], `missing message for ${code}`);
    }
  });

  test('ProblemCode_when_each_code_then_has_documentation_page', () => {
    // Every code the extension can raise must have an "Open Online Help" target.
    // From out/test/unit/ -> repo root is 5 levels up.
    const repoRoot = path.resolve(__dirname, '..', '..', '..', '..', '..');
    const docsDir = path.join(repoRoot, 'docs', 'reference', 'editor', 'problems');
    for (const code of Object.values(ProblemCode)) {
      const page = path.join(docsDir, `${code}.rst`);
      assert.ok(fs.existsSync(page), `missing documentation page ${page}`);
    }
  });
});

suite('formatProblem', () => {
  test('formatProblem_when_no_context_then_returns_code_and_message', () => {
    const result = formatProblem(ProblemCode.NoCompiler);
    assert.strictEqual(result, 'E0001 - Unable to locate IronPLC compiler');
  });

  test('formatProblem_when_context_provided_then_appends_context_after_period', () => {
    const result = formatProblem(ProblemCode.NoCompiler, 'IronPLC is not installed or not configured.');
    assert.strictEqual(result, 'E0001 - Unable to locate IronPLC compiler. IronPLC is not installed or not configured.');
  });

  test('formatProblem_when_viewer_compiler_not_found_then_formats_correctly', () => {
    const result = formatProblem(ProblemCode.ViewerCompilerNotFound, 'Install the compiler to view .iplc files.');
    assert.strictEqual(result, 'E0002 - IronPLC compiler not found when opening .iplc file. Install the compiler to view .iplc files.');
  });

  test('formatProblem_when_disassembly_failed_with_error_then_includes_error_message', () => {
    const result = formatProblem(ProblemCode.DisassemblyFailed, 'connection lost');
    assert.strictEqual(result, 'E0003 - Failed to disassemble .iplc bytecode file. connection lost');
  });

  test('formatProblem_when_debug_compile_failed_then_includes_code_and_detail', () => {
    const result = formatProblem(ProblemCode.DebugCompileFailed, 'main.st: P0001 undeclared X.');
    assert.strictEqual(result, 'E0006 - Failed to compile the program for debugging. main.st: P0001 undeclared X.');
  });

  test('formatProblem_when_debug_server_not_found_then_includes_code', () => {
    const result = formatProblem(ProblemCode.DebugServerNotFound);
    assert.strictEqual(result, `E0007 - IronPLC debug server (${DEBUG_SERVER_BINARY}) not found`);
  });
});
