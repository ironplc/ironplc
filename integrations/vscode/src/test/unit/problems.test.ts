import * as assert from 'assert';
import { ProblemCode, PROBLEM_MESSAGES, formatProblem } from '../../problems';

suite('ProblemCode', () => {
  test('ProblemCode_when_accessed_then_returns_expected_codes', () => {
    assert.strictEqual(ProblemCode.NoCompiler, 'E0001');
    assert.strictEqual(ProblemCode.ViewerCompilerNotFound, 'E0002');
    assert.strictEqual(ProblemCode.DisassemblyFailed, 'E0003');
  });

  test('PROBLEM_MESSAGES_when_accessed_then_has_entry_for_each_code', () => {
    assert.ok(PROBLEM_MESSAGES[ProblemCode.NoCompiler]);
    assert.ok(PROBLEM_MESSAGES[ProblemCode.ViewerCompilerNotFound]);
    assert.ok(PROBLEM_MESSAGES[ProblemCode.DisassemblyFailed]);
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
});
