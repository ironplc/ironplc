import * as assert from 'assert';
import {
  escapeHtml,
  formatOffset,
  getOpcodeClass,
  getErrorHtml,
  renderHeader,
  renderConstants,
  renderFunctions,
  getDisassemblyHtml,
} from '../../iplcRendering';
import {
  createTestHeader,
  createTestInstruction,
  createTestFunction,
  createTestDisassemblyResult,
} from './testHelpers';

suite('escapeHtml', () => {
  test('escapeHtml_when_no_special_chars_then_returns_unchanged', () => {
    assert.strictEqual(escapeHtml('hello world'), 'hello world');
  });

  test('escapeHtml_when_html_entities_then_escapes_all', () => {
    assert.strictEqual(escapeHtml('<div class="a">&'), '&lt;div class=&quot;a&quot;&gt;&amp;');
  });

  test('escapeHtml_when_empty_string_then_returns_empty', () => {
    assert.strictEqual(escapeHtml(''), '');
  });
});

suite('formatOffset', () => {
  test('formatOffset_when_zero_then_returns_padded', () => {
    assert.strictEqual(formatOffset(0), '0x0000');
  });

  test('formatOffset_when_small_value_then_pads_to_four_digits', () => {
    assert.strictEqual(formatOffset(255), '0x00FF');
  });

  test('formatOffset_when_large_value_then_shows_full_hex', () => {
    assert.strictEqual(formatOffset(65535), '0xFFFF');
  });
});

suite('getOpcodeClass', () => {
  test('getOpcodeClass_when_load_opcode_then_returns_op_load', () => {
    assert.strictEqual(getOpcodeClass('LOAD_VAR'), 'op-load');
  });

  test('getOpcodeClass_when_store_opcode_then_returns_op_store', () => {
    assert.strictEqual(getOpcodeClass('STORE_VAR'), 'op-store');
  });

  test('getOpcodeClass_when_add_opcode_then_returns_op_arith', () => {
    assert.strictEqual(getOpcodeClass('ADD_INT'), 'op-arith');
  });

  test('getOpcodeClass_when_ret_opcode_then_returns_op_ctrl', () => {
    assert.strictEqual(getOpcodeClass('RET'), 'op-ctrl');
  });

  test('getOpcodeClass_when_unknown_opcode_then_returns_op_unknown', () => {
    assert.strictEqual(getOpcodeClass('UNKNOWN_42'), 'op-unknown');
  });

  test('getOpcodeClass_when_unrecognized_opcode_then_returns_empty', () => {
    assert.strictEqual(getOpcodeClass('NOP'), '');
  });
});

suite('getErrorHtml', () => {
  test('getErrorHtml_when_simple_message_then_contains_message', () => {
    const html = getErrorHtml('Something went wrong');
    assert.ok(html.includes('Something went wrong'));
    assert.ok(html.includes('class="error"'));
  });

  test('getErrorHtml_when_html_in_message_then_escapes_it', () => {
    const html = getErrorHtml('<script>alert("xss")</script>');
    assert.ok(!html.includes('<script>'));
    assert.ok(html.includes('&lt;script&gt;'));
  });

  test('getErrorHtml_when_called_then_returns_valid_html_document', () => {
    const html = getErrorHtml('test');
    assert.ok(html.includes('<!DOCTYPE html>'));
    assert.ok(html.includes('<html lang="en">'));
    assert.ok(html.includes('</html>'));
  });
});

suite('renderHeader', () => {
  test('renderHeader_when_null_header_then_returns_empty', () => {
    assert.strictEqual(renderHeader(null as any), '');
  });

  test('renderHeader_when_valid_header_then_contains_format_version', () => {
    const header = createTestHeader({ formatVersion: 3 });
    const html = renderHeader(header);
    assert.ok(html.includes('Format Version'));
    assert.ok(html.includes('3'));
  });

  test('renderHeader_when_no_flags_set_then_shows_none', () => {
    const header = createTestHeader();
    const html = renderHeader(header);
    assert.ok(html.includes('None'));
  });

  test('renderHeader_when_flags_set_then_shows_flag_names', () => {
    const header = createTestHeader({
      flags: { hasContentSignature: true, hasDebugSection: true, hasTypeSection: false },
    });
    const html = renderHeader(header);
    assert.ok(html.includes('Content Signature'));
    assert.ok(html.includes('Debug Section'));
  });

  test('renderHeader_when_hashes_present_then_renders_with_hash_class', () => {
    const header = createTestHeader({ contentHash: 'aabbcc', sourceHash: 'ddeeff' });
    const html = renderHeader(header);
    assert.ok(html.includes('class="hash"'));
    assert.ok(html.includes('aabbcc'));
    assert.ok(html.includes('ddeeff'));
  });
});

suite('renderConstants', () => {
  test('renderConstants_when_empty_array_then_shows_empty_label', () => {
    const html = renderConstants([]);
    assert.ok(html.includes('Constant Pool (empty)'));
  });

  test('renderConstants_when_null_then_shows_empty_label', () => {
    const html = renderConstants(null as any);
    assert.ok(html.includes('Constant Pool (empty)'));
  });

  test('renderConstants_when_one_constant_then_shows_count_and_data', () => {
    const html = renderConstants([{ index: 0, type: 'INT', value: '42' }]);
    assert.ok(html.includes('Constant Pool (1)'));
    assert.ok(html.includes('INT'));
    assert.ok(html.includes('42'));
  });

  test('renderConstants_when_html_in_value_then_escapes_it', () => {
    const html = renderConstants([{ index: 0, type: 'STRING', value: '<b>bold</b>' }]);
    assert.ok(html.includes('&lt;b&gt;bold&lt;/b&gt;'));
    assert.ok(!html.includes('<b>bold</b>'));
  });
});

suite('renderFunctions', () => {
  test('renderFunctions_when_empty_array_then_shows_none_label', () => {
    const html = renderFunctions([]);
    assert.ok(html.includes('Functions (none)'));
  });

  test('renderFunctions_when_null_then_shows_none_label', () => {
    const html = renderFunctions(null as any);
    assert.ok(html.includes('Functions (none)'));
  });

  test('renderFunctions_when_one_function_then_renders_metadata', () => {
    const func = createTestFunction({ id: 7, maxStackDepth: 4, numLocals: 3, bytecodeLength: 32 });
    const html = renderFunctions([func]);
    assert.ok(html.includes('Function 7'));
    assert.ok(html.includes('32 bytes'));
  });

  test('renderFunctions_when_instruction_has_comment_then_renders_comment', () => {
    const instr = createTestInstruction({ opcode: 'LOAD', operands: '0', comment: '; local var' });
    const func = createTestFunction({ instructions: [instr] });
    const html = renderFunctions([func]);
    assert.ok(html.includes('class="comment"'));
    assert.ok(html.includes('; local var'));
  });

  test('renderFunctions_when_multiple_functions_then_renders_all', () => {
    const f1 = createTestFunction({ id: 0 });
    const f2 = createTestFunction({ id: 1 });
    const html = renderFunctions([f1, f2]);
    assert.ok(html.includes('Function 0'));
    assert.ok(html.includes('Function 1'));
  });
});

suite('getDisassemblyHtml', () => {
  test('getDisassemblyHtml_when_error_field_set_then_returns_error_html', () => {
    const data = createTestDisassemblyResult({ error: 'parse failed' });
    const html = getDisassemblyHtml(data);
    assert.ok(html.includes('parse failed'));
    assert.ok(html.includes('class="error"'));
    assert.ok(!html.includes('IPLC Bytecode Viewer'));
  });

  test('getDisassemblyHtml_when_valid_data_then_returns_full_document', () => {
    const data = createTestDisassemblyResult();
    const html = getDisassemblyHtml(data);
    assert.ok(html.includes('<!DOCTYPE html>'));
    assert.ok(html.includes('IPLC Bytecode Viewer'));
    assert.ok(html.includes('File Header'));
  });

  test('getDisassemblyHtml_when_valid_data_then_contains_all_sections', () => {
    const data = createTestDisassemblyResult({
      constants: [{ index: 0, type: 'REAL', value: '3.14' }],
      functions: [createTestFunction({ id: 5 })],
    });
    const html = getDisassemblyHtml(data);
    assert.ok(html.includes('File Header'));
    assert.ok(html.includes('Constant Pool (1)'));
    assert.ok(html.includes('Function 5'));
  });
});
