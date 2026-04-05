import * as assert from 'assert';
import { RunSession, RunState, RunSessionCallbacks, RunResult } from '../../runSession';
import { LanguageClientLike } from '../../iplcEditorLogic';

/** Creates a mock client that records sent requests and returns configurable results. */
function createRunMockClient(overrides?: {
  runResult?: RunResult;
  stepResult?: RunResult;
}): { client: LanguageClientLike; requests: { method: string; params: unknown }[] } {
  const requests: { method: string; params: unknown }[] = [];

  const defaultOk: RunResult = { ok: true, variables: [], total_scans: 0 };

  const client: LanguageClientLike = {
    isRunning: () => true,
    sendRequest: (method: string, params: unknown) => {
      requests.push({ method, params });
      if (method === 'ironplc/run') {
        return Promise.resolve(overrides?.runResult ?? defaultOk);
      }
      if (method === 'ironplc/step') {
        return Promise.resolve(overrides?.stepResult ?? { ...defaultOk, total_scans: 1, variables: [{ index: 0, value: '42', name: 'x', type_name: 'DINT' }] });
      }
      if (method === 'ironplc/stop') {
        return Promise.resolve(defaultOk);
      }
      return Promise.resolve(defaultOk);
    },
    onDidChangeState: () => ({ dispose: () => {} }),
  };

  return { client, requests };
}

function createTrackingCallbacks(): { callbacks: RunSessionCallbacks; states: RunState[]; errors: string[] } {
  const states: RunState[] = [];
  const errors: string[] = [];
  const callbacks: RunSessionCallbacks = {
    onStateChange: (state) => states.push(state),
    onVariablesUpdate: () => {},
    onError: (msg) => errors.push(msg),
  };
  return { callbacks, states, errors };
}

suite('RunSession', () => {
  test('start_when_valid_source_then_transitions_to_running', async () => {
    const { client } = createRunMockClient();
    const { callbacks, states } = createTrackingCallbacks();
    const session = new RunSession(client, callbacks);

    await session.start('PROGRAM main END_PROGRAM');

    assert.strictEqual(session.getState(), 'running');
    assert.ok(states.includes('running'));
    session.dispose();
  });

  test('start_when_compile_error_then_transitions_to_error', async () => {
    const { client } = createRunMockClient({
      runResult: { ok: false, variables: [], total_scans: 0, error: 'Syntax error' },
    });
    const { callbacks, states, errors } = createTrackingCallbacks();
    const session = new RunSession(client, callbacks);

    await session.start('INVALID');

    assert.strictEqual(session.getState(), 'error');
    assert.ok(states.includes('error'));
    assert.ok(errors.some(e => e.includes('Syntax error')));
    session.dispose();
  });

  test('start_when_called_sends_run_request', async () => {
    const { client, requests } = createRunMockClient();
    const { callbacks } = createTrackingCallbacks();
    const session = new RunSession(client, callbacks);

    await session.start('PROGRAM main END_PROGRAM');

    const runReqs = requests.filter(r => r.method === 'ironplc/run');
    assert.strictEqual(runReqs.length, 1);
    assert.deepStrictEqual((runReqs[0].params as Record<string, unknown>).source, 'PROGRAM main END_PROGRAM');
    session.dispose();
  });

  test('pause_when_running_then_transitions_to_paused', async () => {
    const { client } = createRunMockClient();
    const { callbacks } = createTrackingCallbacks();
    const session = new RunSession(client, callbacks);

    await session.start('PROGRAM main END_PROGRAM');
    session.pause();

    assert.strictEqual(session.getState(), 'paused');
    session.dispose();
  });

  test('pause_when_idle_then_stays_idle', () => {
    const { client } = createRunMockClient();
    const { callbacks } = createTrackingCallbacks();
    const session = new RunSession(client, callbacks);

    session.pause();

    assert.strictEqual(session.getState(), 'idle');
    session.dispose();
  });

  test('resume_when_paused_then_transitions_to_running', async () => {
    const { client } = createRunMockClient();
    const { callbacks } = createTrackingCallbacks();
    const session = new RunSession(client, callbacks);

    await session.start('PROGRAM main END_PROGRAM');
    session.pause();
    session.resume();

    assert.strictEqual(session.getState(), 'running');
    session.dispose();
  });

  test('resume_when_idle_then_stays_idle', () => {
    const { client } = createRunMockClient();
    const { callbacks } = createTrackingCallbacks();
    const session = new RunSession(client, callbacks);

    session.resume();

    assert.strictEqual(session.getState(), 'idle');
    session.dispose();
  });

  test('stop_when_running_then_sends_stop_and_transitions_to_idle', async () => {
    const { client, requests } = createRunMockClient();
    const { callbacks } = createTrackingCallbacks();
    const session = new RunSession(client, callbacks);

    await session.start('PROGRAM main END_PROGRAM');
    await session.stop();

    assert.strictEqual(session.getState(), 'idle');
    const stopReqs = requests.filter(r => r.method === 'ironplc/stop');
    assert.strictEqual(stopReqs.length, 1);
    session.dispose();
  });

  test('stop_when_idle_then_stays_idle', async () => {
    const { client, requests } = createRunMockClient();
    const { callbacks } = createTrackingCallbacks();
    const session = new RunSession(client, callbacks);

    await session.stop();

    assert.strictEqual(session.getState(), 'idle');
    const stopReqs = requests.filter(r => r.method === 'ironplc/stop');
    assert.strictEqual(stopReqs.length, 0);
    session.dispose();
  });

  test('getState_when_initial_then_returns_idle', () => {
    const { client } = createRunMockClient();
    const { callbacks } = createTrackingCallbacks();
    const session = new RunSession(client, callbacks);

    assert.strictEqual(session.getState(), 'idle');
    session.dispose();
  });

  test('start_when_already_running_then_stops_previous_and_starts_new', async () => {
    const { client, requests } = createRunMockClient();
    const { callbacks } = createTrackingCallbacks();
    const session = new RunSession(client, callbacks);

    await session.start('PROGRAM first END_PROGRAM');
    await session.start('PROGRAM second END_PROGRAM');

    const runReqs = requests.filter(r => r.method === 'ironplc/run');
    assert.strictEqual(runReqs.length, 2);
    assert.strictEqual(session.getState(), 'running');
    session.dispose();
  });
});
