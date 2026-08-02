import * as assert from 'assert';
import * as path from 'path';
import {
  DapEnvironment,
  buildDebugCompileArgs,
  containerOutputPath,
  findDapServerPath,
  isSourceProgram,
  resolveProgramPath,
  scanCountMessage,
} from '../../debugAdapterLogic';

function createDapEnv(overrides?: Partial<DapEnvironment>): DapEnvironment {
  return {
    platform: 'linux',
    existsSync: () => false,
    getEnv: () => undefined,
    getConfig: () => undefined,
    ...overrides,
  };
}

suite('isSourceProgram', () => {
  test('isSourceProgram_when_st_extension_then_true', () => {
    assert.strictEqual(isSourceProgram('/work/main.st'), true);
  });

  test('isSourceProgram_when_iec_extension_then_true', () => {
    assert.strictEqual(isSourceProgram('/work/main.iec'), true);
  });

  test('isSourceProgram_when_twincat_extension_uppercase_then_true', () => {
    // Matched case-insensitively so the on-disk casing of .TcPOU does not matter.
    assert.strictEqual(isSourceProgram('/work/Main.TcPOU'), true);
  });

  test('isSourceProgram_when_iplc_container_then_false', () => {
    assert.strictEqual(isSourceProgram('/work/main.iplc'), false);
  });

  test('isSourceProgram_when_no_extension_then_false', () => {
    assert.strictEqual(isSourceProgram('/work/main'), false);
  });
});

suite('containerOutputPath', () => {
  test('containerOutputPath_when_source_then_iplc_under_tmp', () => {
    const result = containerOutputPath('/work/src/main.st', '/tmp');
    assert.strictEqual(result, path.join('/tmp', 'main.iplc'));
  });

  test('containerOutputPath_when_twincat_source_then_strips_original_extension', () => {
    const result = containerOutputPath('/work/Motor.TcPOU', '/tmp');
    assert.strictEqual(result, path.join('/tmp', 'Motor.iplc'));
  });
});

suite('findDapServerPath', () => {
  test('findDapServerPath_when_env_var_set_then_returns_environment_source', () => {
    const env = createDapEnv({
      getEnv: name => name === 'IRONPLCDAP' ? '/env/ironplcdap' : undefined,
      existsSync: p => p === '/env/ironplcdap',
    });
    const result = findDapServerPath(env, '/bundled');
    assert.ok(result);
    assert.strictEqual(result.path, '/env/ironplcdap');
    assert.strictEqual(result.source, 'environment');
  });

  test('findDapServerPath_when_only_setting_then_returns_configuration_source', () => {
    const env = createDapEnv({
      getConfig: key => key === 'dapServerPath' ? '/cfg/ironplcdap' : undefined,
      existsSync: p => p === '/cfg/ironplcdap',
    });
    const result = findDapServerPath(env, '/bundled');
    assert.ok(result);
    assert.strictEqual(result.path, '/cfg/ironplcdap');
    assert.strictEqual(result.source, 'configuration');
  });

  test('findDapServerPath_when_env_and_setting_set_then_env_wins', () => {
    const env = createDapEnv({
      getEnv: name => name === 'IRONPLCDAP' ? '/env/ironplcdap' : undefined,
      getConfig: key => key === 'dapServerPath' ? '/cfg/ironplcdap' : undefined,
      existsSync: () => true,
    });
    const result = findDapServerPath(env, '/bundled');
    assert.ok(result);
    assert.strictEqual(result.source, 'environment');
  });

  test('findDapServerPath_when_only_bundled_then_returns_bundled_source', () => {
    const expected = path.join('/bundled', 'ironplcdap');
    const env = createDapEnv({
      existsSync: p => p === expected,
    });
    const result = findDapServerPath(env, '/bundled');
    assert.ok(result);
    assert.strictEqual(result.path, expected);
    assert.strictEqual(result.source, 'bundled');
  });

  test('findDapServerPath_when_win32_then_uses_exe_extension', () => {
    const expected = path.join('/bundled', 'ironplcdap.exe');
    const env = createDapEnv({
      platform: 'win32',
      existsSync: p => p === expected,
    });
    const result = findDapServerPath(env, '/bundled');
    assert.ok(result);
    assert.ok(result.path.endsWith('ironplcdap.exe'));
  });

  test('findDapServerPath_when_no_compiler_dir_and_nothing_set_then_undefined', () => {
    const env = createDapEnv();
    const result = findDapServerPath(env, undefined);
    assert.strictEqual(result, undefined);
  });

  test('findDapServerPath_when_candidate_missing_on_disk_then_falls_through', () => {
    // The env var points somewhere that does not exist; discovery falls back
    // to the bundled binary.
    const bundled = path.join('/bundled', 'ironplcdap');
    const env = createDapEnv({
      getEnv: name => name === 'IRONPLCDAP' ? '/env/missing' : undefined,
      existsSync: p => p === bundled,
    });
    const result = findDapServerPath(env, '/bundled');
    assert.ok(result);
    assert.strictEqual(result.source, 'bundled');
  });
});

suite('resolveProgramPath', () => {
  test('resolveProgramPath_when_config_program_set_then_returns_config', () => {
    assert.strictEqual(resolveProgramPath('/work/main.st', '/other/open.st'), '/work/main.st');
  });

  test('resolveProgramPath_when_config_empty_then_returns_active_editor', () => {
    assert.strictEqual(resolveProgramPath('', '/other/open.st'), '/other/open.st');
  });

  test('resolveProgramPath_when_config_undefined_then_returns_active_editor', () => {
    assert.strictEqual(resolveProgramPath(undefined, '/other/open.st'), '/other/open.st');
  });

  test('resolveProgramPath_when_neither_set_then_undefined', () => {
    assert.strictEqual(resolveProgramPath(undefined, undefined), undefined);
  });
});

suite('buildDebugCompileArgs', () => {
  test('buildDebugCompileArgs_when_program_and_output_then_compile_args', () => {
    const args = buildDebugCompileArgs('/work/main.st', '/tmp/main.iplc');
    assert.deepStrictEqual(args, ['compile', '/work/main.st', '-o', '/tmp/main.iplc']);
  });
});

suite('scanCountMessage', () => {
  test('scanCountMessage_when_count_present_then_includes_count', () => {
    assert.ok(scanCountMessage({ scanCount: 7 }).includes('7'));
  });

  test('scanCountMessage_when_count_missing_then_not_available', () => {
    assert.ok(scanCountMessage({}).includes('not available'));
  });

  test('scanCountMessage_when_response_undefined_then_not_available', () => {
    assert.ok(scanCountMessage(undefined).includes('not available'));
  });
});
