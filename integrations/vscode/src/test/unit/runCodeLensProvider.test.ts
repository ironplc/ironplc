import * as assert from 'assert';
import { findProgramLenses } from '../../runCodeLensProvider';

suite('RunCodeLensProvider', () => {
  test('findProgramLenses_when_single_program_then_returns_one_lens', () => {
    const text = 'PROGRAM main\n  VAR\n    x : INT;\n  END_VAR\nEND_PROGRAM';
    const lenses = findProgramLenses(text);

    assert.strictEqual(lenses.length, 1);
    assert.strictEqual(lenses[0].range.start.line, 0);
    assert.strictEqual(lenses[0].command?.command, 'ironplc.runProgram');
    assert.deepStrictEqual(lenses[0].command?.arguments, ['main']);
  });

  test('findProgramLenses_when_indented_program_then_still_matches', () => {
    const text = '  PROGRAM indented\n  END_PROGRAM';
    const lenses = findProgramLenses(text);

    assert.strictEqual(lenses.length, 1);
    assert.deepStrictEqual(lenses[0].command?.arguments, ['indented']);
  });

  test('findProgramLenses_when_lowercase_program_then_matches', () => {
    const text = 'program lower\nend_program';
    const lenses = findProgramLenses(text);

    assert.strictEqual(lenses.length, 1);
    assert.deepStrictEqual(lenses[0].command?.arguments, ['lower']);
  });

  test('findProgramLenses_when_no_program_then_returns_empty', () => {
    const text = 'FUNCTION_BLOCK fb\nEND_FUNCTION_BLOCK';
    const lenses = findProgramLenses(text);

    assert.strictEqual(lenses.length, 0);
  });

  test('findProgramLenses_when_multiple_programs_then_returns_all', () => {
    const text = 'PROGRAM first\nEND_PROGRAM\n\nPROGRAM second\nEND_PROGRAM';
    const lenses = findProgramLenses(text);

    assert.strictEqual(lenses.length, 2);
    assert.deepStrictEqual(lenses[0].command?.arguments, ['first']);
    assert.deepStrictEqual(lenses[1].command?.arguments, ['second']);
  });

  test('findProgramLenses_when_end_program_keyword_then_not_matched', () => {
    const text = 'PROGRAM main\nEND_PROGRAM';
    const lenses = findProgramLenses(text);

    // Only the PROGRAM line matches, not END_PROGRAM
    assert.strictEqual(lenses.length, 1);
    assert.strictEqual(lenses[0].range.start.line, 0);
  });

  test('findProgramLenses_when_empty_text_then_returns_empty', () => {
    const lenses = findProgramLenses('');

    assert.strictEqual(lenses.length, 0);
  });

  test('findProgramLenses_when_comment_contains_program_then_matches', () => {
    // This is acceptable behavior - regex-based detection may match comments.
    // A false positive code lens above a comment is harmless.
    const text = '(* PROGRAM in_comment *)\nPROGRAM real_program\nEND_PROGRAM';
    const lenses = findProgramLenses(text);

    // At least the real program is found
    assert.ok(lenses.some(l => l.command?.arguments?.[0] === 'real_program'));
  });

  test('findProgramLenses_lens_has_play_icon_title', () => {
    const text = 'PROGRAM main\nEND_PROGRAM';
    const lenses = findProgramLenses(text);

    assert.ok(lenses[0].command?.title.includes('Run Program'));
  });

  test('findProgramLenses_when_running_state_then_returns_pause_and_stop_lenses', () => {
    const text = 'PROGRAM main\nEND_PROGRAM';
    const lenses = findProgramLenses(text, 'running');

    assert.strictEqual(lenses.length, 2);
    assert.ok(lenses[0].command?.title.includes('Pause'));
    assert.strictEqual(lenses[0].command?.command, 'ironplc.pauseProgram');
    assert.ok(lenses[1].command?.title.includes('Stop'));
    assert.strictEqual(lenses[1].command?.command, 'ironplc.stopProgram');
    assert.strictEqual(lenses[0].range.start.line, 0);
    assert.strictEqual(lenses[1].range.start.line, 0);
  });

  test('findProgramLenses_when_paused_state_then_returns_resume_and_stop_lenses', () => {
    const text = 'PROGRAM main\nEND_PROGRAM';
    const lenses = findProgramLenses(text, 'paused');

    assert.strictEqual(lenses.length, 2);
    assert.ok(lenses[0].command?.title.includes('Resume'));
    assert.strictEqual(lenses[0].command?.command, 'ironplc.pauseProgram');
    assert.ok(lenses[1].command?.title.includes('Stop'));
    assert.strictEqual(lenses[1].command?.command, 'ironplc.stopProgram');
  });

  test('findProgramLenses_when_error_state_then_returns_run_lens', () => {
    const text = 'PROGRAM main\nEND_PROGRAM';
    const lenses = findProgramLenses(text, 'error');

    assert.strictEqual(lenses.length, 1);
    assert.ok(lenses[0].command?.title.includes('Run Program'));
    assert.strictEqual(lenses[0].command?.command, 'ironplc.runProgram');
  });

  test('findProgramLenses_when_no_compiler_and_idle_then_returns_warning_lens', () => {
    const text = 'PROGRAM main\nEND_PROGRAM';
    const lenses = findProgramLenses(text, 'idle', false);

    assert.strictEqual(lenses.length, 1);
    assert.ok(lenses[0].command?.title.includes('no compiler'));
    assert.strictEqual(lenses[0].command?.command, 'ironplc.runProgram');
  });

  test('findProgramLenses_when_running_and_multiple_programs_then_each_gets_pause_stop', () => {
    const text = 'PROGRAM first\nEND_PROGRAM\n\nPROGRAM second\nEND_PROGRAM';
    const lenses = findProgramLenses(text, 'running');

    assert.strictEqual(lenses.length, 4);
    assert.strictEqual(lenses[0].range.start.line, 0);
    assert.strictEqual(lenses[1].range.start.line, 0);
    assert.strictEqual(lenses[2].range.start.line, 3);
    assert.strictEqual(lenses[3].range.start.line, 3);
    assert.strictEqual(lenses[0].command?.command, 'ironplc.pauseProgram');
    assert.strictEqual(lenses[1].command?.command, 'ironplc.stopProgram');
    assert.strictEqual(lenses[2].command?.command, 'ironplc.pauseProgram');
    assert.strictEqual(lenses[3].command?.command, 'ironplc.stopProgram');
  });
});
