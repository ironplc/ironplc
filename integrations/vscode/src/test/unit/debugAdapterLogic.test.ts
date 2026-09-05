import * as assert from 'assert';
import * as path from 'path';
import * as fs from 'fs';
import {
  CONFIG_SECTION,
  DapEnvironment,
  DEBUG_SERVER_BINARY,
  DEBUG_SERVER_ENV_VAR,
  DEBUG_SERVER_PATH_SETTING,
  DEBUG_SERVER_PATH_SETTING_ID,
  containerOutputPath,
  debugServerFileName,
  debugServerNotFoundHint,
  findDapServerPath,
  firstLine,
  isDebuggableProgram,
  isSourceProgram,
  programKind,
  resolveProgramPath,
  customRequestFailedMessage,
  sourceExtensionsFromLanguages,
} from '../../debugAdapterLogic';

/** The source extensions the extension currently contributes, lowercased. */
const SRC = ['.st', '.iec', '.tcpou', '.tcgvl', '.tcdut'];

function createDapEnv(overrides?: Partial<DapEnvironment>): DapEnvironment {
  return {
    platform: 'linux',
    existsSync: () => false,
    getEnv: () => undefined,
    getConfig: () => undefined,
    ...overrides,
  };
}

suite('sourceExtensionsFromLanguages', () => {
  test('sourceExtensionsFromLanguages_when_languages_then_flattens_and_lowercases', () => {
    const result = sourceExtensionsFromLanguages([
      { extensions: ['.st', '.iec'] },
      { extensions: ['.TcPOU'] },
      { extensions: [] },
      {},
    ]);
    assert.deepStrictEqual(result.sort(), ['.iec', '.st', '.tcpou']);
  });

  test('sourceExtensionsFromLanguages_when_duplicate_extensions_then_deduplicated', () => {
    const result = sourceExtensionsFromLanguages([
      { extensions: ['.st'] },
      { extensions: ['.ST'] },
    ]);
    assert.deepStrictEqual(result, ['.st']);
  });

  test('sourceExtensionsFromLanguages_when_empty_then_empty', () => {
    assert.deepStrictEqual(sourceExtensionsFromLanguages([]), []);
  });
});

suite('isSourceProgram', () => {
  test('isSourceProgram_when_st_extension_then_true', () => {
    assert.strictEqual(isSourceProgram('/work/main.st', SRC), true);
  });

  test('isSourceProgram_when_iec_extension_then_true', () => {
    assert.strictEqual(isSourceProgram('/work/main.iec', SRC), true);
  });

  test('isSourceProgram_when_twincat_extension_uppercase_then_true', () => {
    // Matched case-insensitively so the on-disk casing of .TcPOU does not matter.
    assert.strictEqual(isSourceProgram('/work/Main.TcPOU', SRC), true);
  });

  test('isSourceProgram_when_iplc_container_then_false', () => {
    assert.strictEqual(isSourceProgram('/work/main.iplc', SRC), false);
  });

  test('isSourceProgram_when_no_extension_then_false', () => {
    assert.strictEqual(isSourceProgram('/work/main', SRC), false);
  });
});

suite('programKind', () => {
  test('programKind_when_source_extension_then_source', () => {
    assert.strictEqual(programKind('/work/main.st', SRC), 'source');
  });

  test('programKind_when_iplc_then_container', () => {
    assert.strictEqual(programKind('/work/main.iplc', SRC), 'container');
  });

  test('programKind_when_json_then_unknown', () => {
    // Regression: a launch.json path must not be treated as a container.
    assert.strictEqual(programKind('/work/.vscode/launch.json', SRC), 'unknown');
  });

  test('programKind_when_no_extension_then_unknown', () => {
    assert.strictEqual(programKind('/work/main', SRC), 'unknown');
  });
});

suite('isDebuggableProgram', () => {
  test('isDebuggableProgram_when_source_then_true', () => {
    assert.strictEqual(isDebuggableProgram('/work/main.st', SRC), true);
  });

  test('isDebuggableProgram_when_container_then_true', () => {
    assert.strictEqual(isDebuggableProgram('/work/main.iplc', SRC), true);
  });

  test('isDebuggableProgram_when_launch_json_then_false', () => {
    assert.strictEqual(isDebuggableProgram('/work/.vscode/launch.json', SRC), false);
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
      getEnv: name => name === DEBUG_SERVER_ENV_VAR ? `/env/${DEBUG_SERVER_BINARY}` : undefined,
      existsSync: p => p === `/env/${DEBUG_SERVER_BINARY}`,
    });
    const result = findDapServerPath(env, '/bundled');
    assert.ok(result);
    assert.strictEqual(result.path, `/env/${DEBUG_SERVER_BINARY}`);
    assert.strictEqual(result.source, 'environment');
  });

  test('findDapServerPath_when_only_setting_then_returns_configuration_source', () => {
    const env = createDapEnv({
      getConfig: key => key === DEBUG_SERVER_PATH_SETTING ? `/cfg/${DEBUG_SERVER_BINARY}` : undefined,
      existsSync: p => p === `/cfg/${DEBUG_SERVER_BINARY}`,
    });
    const result = findDapServerPath(env, '/bundled');
    assert.ok(result);
    assert.strictEqual(result.path, `/cfg/${DEBUG_SERVER_BINARY}`);
    assert.strictEqual(result.source, 'configuration');
  });

  test('findDapServerPath_when_env_and_setting_set_then_env_wins', () => {
    const env = createDapEnv({
      getEnv: name => name === DEBUG_SERVER_ENV_VAR ? `/env/${DEBUG_SERVER_BINARY}` : undefined,
      getConfig: key => key === DEBUG_SERVER_PATH_SETTING ? `/cfg/${DEBUG_SERVER_BINARY}` : undefined,
      existsSync: () => true,
    });
    const result = findDapServerPath(env, '/bundled');
    assert.ok(result);
    assert.strictEqual(result.source, 'environment');
  });

  test('findDapServerPath_when_only_bundled_then_returns_bundled_source', () => {
    const expected = path.join('/bundled', DEBUG_SERVER_BINARY);
    const env = createDapEnv({
      existsSync: p => p === expected,
    });
    const result = findDapServerPath(env, '/bundled');
    assert.ok(result);
    assert.strictEqual(result.path, expected);
    assert.strictEqual(result.source, 'bundled');
  });

  test('findDapServerPath_when_win32_then_uses_exe_extension', () => {
    const expected = path.join('/bundled', DEBUG_SERVER_BINARY + '.exe');
    const env = createDapEnv({
      platform: 'win32',
      existsSync: p => p === expected,
    });
    const result = findDapServerPath(env, '/bundled');
    assert.ok(result);
    assert.ok(result.path.endsWith(DEBUG_SERVER_BINARY + '.exe'));
  });

  test('findDapServerPath_when_no_compiler_dir_and_nothing_set_then_undefined', () => {
    const env = createDapEnv();
    const result = findDapServerPath(env, undefined);
    assert.strictEqual(result, undefined);
  });

  test('findDapServerPath_when_candidate_missing_on_disk_then_falls_through', () => {
    // The env var points somewhere that does not exist; discovery falls back
    // to the bundled binary.
    const bundled = path.join('/bundled', DEBUG_SERVER_BINARY);
    const env = createDapEnv({
      getEnv: name => name === DEBUG_SERVER_ENV_VAR ? '/env/missing' : undefined,
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

suite('firstLine', () => {
  test('firstLine_when_multiple_lines_then_first_nonempty', () => {
    assert.strictEqual(firstLine('\n\n  P0001: undeclared X\nnext line'), 'P0001: undeclared X');
  });

  test('firstLine_when_all_blank_then_empty', () => {
    assert.strictEqual(firstLine('\n   \n'), '');
  });

  test('firstLine_when_empty_then_empty', () => {
    assert.strictEqual(firstLine(''), '');
  });
});

suite('customRequestFailedMessage', () => {
  test('customRequestFailedMessage_when_given_title_then_names_the_command', () => {
    const message = customRequestFailedMessage('Step Scan Cycle');
    assert.ok(message.includes('Step Scan Cycle'));
    assert.ok(message.includes('paused'));
  });
});

suite('debugServerFileName', () => {
  test('debugServerFileName_when_posix_then_returns_bare_binary', () => {
    assert.strictEqual(debugServerFileName('linux'), DEBUG_SERVER_BINARY);
    assert.strictEqual(debugServerFileName('darwin'), DEBUG_SERVER_BINARY);
  });

  test('debugServerFileName_when_win32_then_appends_exe', () => {
    assert.strictEqual(debugServerFileName('win32'), DEBUG_SERVER_BINARY + '.exe');
  });
});

suite('debugServerNotFoundHint', () => {
  test('debugServerNotFoundHint_when_called_then_names_setting_and_binary', () => {
    // The hint is the only place the E0007 remedy is spelled out, so it must
    // name both things the user can act on.
    const hint = debugServerNotFoundHint();
    assert.ok(hint.includes(DEBUG_SERVER_PATH_SETTING_ID), hint);
    assert.ok(hint.includes(DEBUG_SERVER_BINARY), hint);
  });
});

/**
 * Guards for the copies of the debug-server names that live outside
 * TypeScript and so cannot import the constants in `debugAdapterLogic.ts`:
 * the extension manifest and the compiler's `[[bin]]` target. Without these,
 * renaming the server leaves the extension compiling and its tests green while
 * discovery fails at runtime as E0007 on a user's machine.
 */
suite('debug server name consistency', () => {
  // From out/test/unit/: the extension root is 3 levels up, the repo root 5.
  const extensionRoot = path.resolve(__dirname, '..', '..', '..');
  const repoRoot = path.resolve(extensionRoot, '..', '..');

  test('packageJson_when_read_then_declares_the_debug_server_path_setting', () => {
    const manifest = JSON.parse(
      fs.readFileSync(path.join(extensionRoot, 'package.json'), 'utf-8'),
    );
    const properties = manifest.contributes.configuration.properties;
    assert.ok(
      Object.prototype.hasOwnProperty.call(properties, DEBUG_SERVER_PATH_SETTING_ID),
      `package.json has no "${DEBUG_SERVER_PATH_SETTING_ID}" setting; `
      + `it declares ${JSON.stringify(Object.keys(properties))}`,
    );
    assert.ok(
      Object.keys(properties).every(key => key.startsWith(`${CONFIG_SECTION}.`)),
      `every setting must live under "${CONFIG_SECTION}."`,
    );
  });

  test('packageJson_when_setting_described_then_description_names_the_binary', () => {
    const manifest = JSON.parse(
      fs.readFileSync(path.join(extensionRoot, 'package.json'), 'utf-8'),
    );
    const setting = manifest.contributes.configuration.properties[DEBUG_SERVER_PATH_SETTING_ID];
    assert.ok(
      setting.markdownDescription.includes(DEBUG_SERVER_BINARY),
      `the "${DEBUG_SERVER_PATH_SETTING_ID}" description must name ${DEBUG_SERVER_BINARY}, `
      + `but says: ${setting.markdownDescription}`,
    );
  });

  test('cargoManifest_when_read_then_declares_the_debug_server_binary', () => {
    // The extension launches a binary the compiler builds. Nothing else ties
    // the two names together, so a rename on either side must fail here.
    const cargoToml = fs.readFileSync(
      path.join(repoRoot, 'compiler', 'vm-cli', 'Cargo.toml'),
      'utf-8',
    );
    // Only `[[bin]]` blocks — the `[package]` name is not a binary.
    const binNames: string[] = [];
    let inBin = false;
    for (const line of cargoToml.split('\n')) {
      const trimmed = line.trim();
      if (trimmed.startsWith('#')) {
        continue;
      }
      if (trimmed.startsWith('[')) {
        inBin = trimmed === '[[bin]]';
        continue;
      }
      const name = inBin ? /^name\s*=\s*"([^"]+)"/.exec(trimmed) : null;
      if (name) {
        binNames.push(name[1]);
      }
    }
    assert.ok(
      binNames.includes(DEBUG_SERVER_BINARY),
      `compiler/vm-cli/Cargo.toml declares no target named ${DEBUG_SERVER_BINARY}; `
      + `it declares ${JSON.stringify(binNames)}`,
    );
  });
});

