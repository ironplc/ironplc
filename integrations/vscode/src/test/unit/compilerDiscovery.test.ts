import * as assert from 'assert';
import * as path from 'path';
import { CompilerEnvironment, findCompilerPath } from '../../compilerDiscovery';

function createTestEnv(overrides?: Partial<CompilerEnvironment>): CompilerEnvironment {
  return {
    platform: 'linux',
    existsSync: () => false,
    getEnv: () => undefined,
    getConfig: () => undefined,
    ...overrides,
  };
}

suite('findCompilerPath', () => {
  test('findCompilerPath_when_config_path_exists_then_returns_config_path', () => {
    const env = createTestEnv({
      getConfig: key => key === 'path' ? '/custom/dir' : undefined,
      existsSync: p => p === '/custom/dir/ironplcc',
    });
    const result = findCompilerPath(env);
    assert.ok(result);
    assert.strictEqual(result.path, '/custom/dir/ironplcc');
    assert.strictEqual(result.source, 'configuration');
  });

  test('findCompilerPath_when_config_path_missing_then_tries_env', () => {
    const env = createTestEnv({
      getConfig: () => undefined,
      getEnv: name => name === 'IRONPLC' ? '/env/dir' : undefined,
      existsSync: p => p === '/env/dir/ironplcc',
    });
    const result = findCompilerPath(env);
    assert.ok(result);
    assert.strictEqual(result.path, '/env/dir/ironplcc');
    assert.strictEqual(result.source, 'environment');
  });

  test('findCompilerPath_when_env_var_exists_then_returns_env_path', () => {
    const env = createTestEnv({
      getEnv: name => name === 'IRONPLC' ? '/my/compiler' : undefined,
      existsSync: p => p === '/my/compiler/ironplcc',
    });
    const result = findCompilerPath(env);
    assert.ok(result);
    assert.strictEqual(result.path, '/my/compiler/ironplcc');
    assert.strictEqual(result.source, 'environment');
  });

  test('findCompilerPath_when_env_var_missing_then_tries_platform_paths', () => {
    const env = createTestEnv({
      platform: 'darwin',
      existsSync: p => p === '/opt/homebrew/bin/ironplcc',
    });
    const result = findCompilerPath(env);
    assert.ok(result);
    assert.strictEqual(result.path, '/opt/homebrew/bin/ironplcc');
    assert.strictEqual(result.source, 'homebrew');
  });

  test('findCompilerPath_when_darwin_and_homebrew_exists_then_returns_homebrew_path', () => {
    const env = createTestEnv({
      platform: 'darwin',
      existsSync: p => p === '/opt/homebrew/bin/ironplcc',
    });
    const result = findCompilerPath(env);
    assert.ok(result);
    assert.strictEqual(result.path, '/opt/homebrew/bin/ironplcc');
    assert.strictEqual(result.source, 'homebrew');
  });

  test('findCompilerPath_when_win32_and_localappdata_exists_then_returns_windows_path', () => {
    const localAppData = '/tmp/appdata';
    const expected = path.join(localAppData, 'Programs', 'IronPLC Compiler', 'bin', 'ironplcc.exe');
    const env = createTestEnv({
      platform: 'win32',
      getEnv: name => name === 'LOCALAPPDATA' ? localAppData : undefined,
      existsSync: p => p === expected,
    });
    const result = findCompilerPath(env);
    assert.ok(result);
    assert.ok(result.path.includes('IronPLC Compiler'));
    assert.ok(result.path.endsWith('ironplcc.exe'));
    assert.strictEqual(result.source, 'localappdata');
  });

  test('findCompilerPath_when_nothing_found_then_returns_undefined', () => {
    const env = createTestEnv();
    const result = findCompilerPath(env);
    assert.strictEqual(result, undefined);
  });

  test('findCompilerPath_when_win32_then_uses_exe_extension', () => {
    const configDir = '/tmp/compiler';
    const expected = path.join(configDir, 'ironplcc.exe');
    const env = createTestEnv({
      platform: 'win32',
      getConfig: key => key === 'path' ? configDir : undefined,
      existsSync: p => p === expected,
    });
    const result = findCompilerPath(env);
    assert.ok(result);
    assert.ok(result.path.endsWith('.exe'));
  });

  test('findCompilerPath_when_linux_then_no_exe_extension', () => {
    const env = createTestEnv({
      platform: 'linux',
      getConfig: key => key === 'path' ? '/usr/bin' : undefined,
      existsSync: p => p === '/usr/bin/ironplcc',
    });
    const result = findCompilerPath(env);
    assert.ok(result);
    assert.ok(!result.path.endsWith('.exe'));
    assert.ok(result.path.endsWith('ironplcc'));
  });
});
