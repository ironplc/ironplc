import * as assert from 'assert';
import { waitForClient, resolveEditorContent } from '../../iplcEditorLogic';
import {
  createMockClient,
  createTestDisassemblyResult,
  STATE_RUNNING,
  STATE_STOPPED,
} from './testHelpers';

suite('waitForClient', () => {
  test('waitForClient_when_already_running_then_resolves_true_immediately', async () => {
    const client = createMockClient({ isRunning: () => true });
    const result = await waitForClient(client, 5000);
    assert.strictEqual(result, true);
  });

  test('waitForClient_when_starts_before_timeout_then_resolves_true', async () => {
    let stateListener: ((e: { newState: number }) => void) | undefined;
    const client = createMockClient({
      isRunning: () => false,
      onDidChangeState: (listener) => {
        stateListener = listener;
        return { dispose: () => {} };
      },
    });

    const promise = waitForClient(client, 5000);
    stateListener!({ newState: STATE_RUNNING });
    const result = await promise;
    assert.strictEqual(result, true);
  });

  test('waitForClient_when_stops_before_timeout_then_resolves_false', async () => {
    let stateListener: ((e: { newState: number }) => void) | undefined;
    const client = createMockClient({
      isRunning: () => false,
      onDidChangeState: (listener) => {
        stateListener = listener;
        return { dispose: () => {} };
      },
    });

    const promise = waitForClient(client, 5000);
    stateListener!({ newState: STATE_STOPPED });
    const result = await promise;
    assert.strictEqual(result, false);
  });

  test('waitForClient_when_timeout_expires_then_resolves_false', async () => {
    const client = createMockClient({
      isRunning: () => false,
      onDidChangeState: () => ({ dispose: () => {} }),
    });

    const result = await waitForClient(client, 10);
    assert.strictEqual(result, false);
  });
});

suite('resolveEditorContent', () => {
  test('resolveEditorContent_when_client_running_and_request_succeeds_then_renders_disassembly', async () => {
    const client = createMockClient({
      isRunning: () => true,
      sendRequest: () => Promise.resolve(createTestDisassemblyResult()),
    });

    const html = await resolveEditorContent(client, 'file:///test.iplc');
    assert.ok(html.includes('IPLC Bytecode Viewer'));
  });

  test('resolveEditorContent_when_client_running_and_request_fails_then_renders_error', async () => {
    const client = createMockClient({
      isRunning: () => true,
      sendRequest: () => Promise.reject(new Error('connection lost')),
    });

    const html = await resolveEditorContent(client, 'file:///test.iplc');
    assert.ok(html.includes('E0003'));
    assert.ok(html.includes('connection lost'));
  });

  test('resolveEditorContent_when_client_running_and_result_has_error_then_renders_error', async () => {
    const client = createMockClient({
      isRunning: () => true,
      sendRequest: () => Promise.resolve({ error: 'invalid bytecode' }),
    });

    const html = await resolveEditorContent(client, 'file:///test.iplc');
    assert.ok(html.includes('invalid bytecode'));
    assert.ok(html.includes('class="error"'));
  });

  test('resolveEditorContent_when_client_not_running_and_starts_within_timeout_then_renders_disassembly', async () => {
    let stateListener: ((e: { newState: number }) => void) | undefined;
    const client = createMockClient({
      isRunning: () => false,
      sendRequest: () => Promise.resolve(createTestDisassemblyResult()),
      onDidChangeState: (listener) => {
        stateListener = listener;
        return { dispose: () => {} };
      },
    });

    const promise = resolveEditorContent(client, 'file:///test.iplc');
    stateListener!({ newState: STATE_RUNNING });
    const html = await promise;
    assert.ok(html.includes('IPLC Bytecode Viewer'));
  });

  test('resolveEditorContent_when_client_not_running_and_times_out_then_renders_compiler_not_found', async () => {
    const client = createMockClient({
      isRunning: () => false,
      onDidChangeState: () => ({ dispose: () => {} }),
    });

    const html = await resolveEditorContent(client, 'file:///test.iplc', 10);
    assert.ok(html.includes('E0002'));
    assert.ok(html.includes('compiler not found'));
  });

  test('resolveEditorContent_when_client_not_running_and_stops_then_renders_compiler_not_found', async () => {
    let stateListener: ((e: { newState: number }) => void) | undefined;
    const client = createMockClient({
      isRunning: () => false,
      onDidChangeState: (listener) => {
        stateListener = listener;
        return { dispose: () => {} };
      },
    });

    const promise = resolveEditorContent(client, 'file:///test.iplc');
    stateListener!({ newState: STATE_STOPPED });
    const html = await promise;
    assert.ok(html.includes('E0002'));
  });
});
