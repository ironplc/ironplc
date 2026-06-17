import * as path from 'path';

export interface CompilerEnvironment {
  platform: string;
  existsSync: (path: string) => boolean;
  getEnv: (name: string) => string | undefined;
  getConfig: (key: string) => string | undefined;
}

export interface CompilerDiscoveryResult {
  path: string;
  source: string;
}

export function findCompilerPath(env: CompilerEnvironment): CompilerDiscoveryResult | undefined {
  const ext = env.platform === 'win32' ? '.exe' : '';

  const trialGenerator: (() => [string | undefined, string])[] = [
    () => {
      // Try to get from configuration
      return [env.getConfig('path'), 'configuration'];
    },
    () => {
      // Try to get from environment variable. Not generally set.
      return [env.getEnv('IRONPLC'), 'environment'];
    },
    () => {
      // Mac well known directory
      const homebrewDir = env.platform === 'darwin' ? '/opt/homebrew/bin' : undefined;
      return [homebrewDir, 'homebrew'];
    },
    () => {
      // Windows user-install well-known path
      const name = 'localappdata';
      const localAppData = env.getEnv('LOCALAPPDATA');

      if (env.platform !== 'win32' || !localAppData) {
        return [undefined, name];
      }
      const winAppDataDir = path.join(localAppData, 'Programs', 'IronPLC Compiler', 'bin');
      return [winAppDataDir, name];
    },
  ];

  const triedLocations: string[] = [];

  for (const trial of trialGenerator) {
    const result = trial();
    const testDir = result[0];
    const trialType = result[1];

    if (!testDir) {
      continue;
    }

    const testExe = path.join(testDir, 'ironplcc' + ext);
    if (!env.existsSync(testExe)) {
      triedLocations.push(trialType + ': (' + testExe + ')');
      continue;
    }

    return { path: testExe, source: trialType };
  }

  return undefined;
}

/**
 * Builds the user-facing message shown when the language client fails to
 * launch the compiler. A common cause is a compiler binary built for a
 * different platform (for example, a Linux binary on Windows produces
 * "spawn ENOEXEC"), so the message includes the resolved path and the
 * discovery source to help the user verify the path that was actually used.
 */
export function formatStartFailure(result: CompilerDiscoveryResult, error: unknown): string {
  const reason = error instanceof Error ? error.message : String(error);
  return 'IronPLC failed to start the compiler at "' + result.path
    + '" (source: ' + result.source + '): ' + reason;
}
