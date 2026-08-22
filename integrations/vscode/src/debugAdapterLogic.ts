import * as path from 'path';

/**
 * Pure decision logic for the IronPLC debug adapter integration. Everything
 * that can be decided without touching the `vscode` API or the filesystem
 * lives here so it can be unit tested directly (mirrors the split used by
 * `compilerDiscovery.ts` and `taskProviderLogic.ts`). The `vscode`-facing glue
 * lives in `debugAdapter.ts`.
 */

/**
 * Injected environment for DAP-server discovery, mirroring
 * `CompilerEnvironment` in `compilerDiscovery.ts`. Keeping the platform,
 * filesystem, environment, and configuration behind an interface makes the
 * resolution order testable without a real machine.
 */
export interface DapEnvironment {
  platform: string;
  existsSync: (path: string) => boolean;
  getEnv: (name: string) => string | undefined;
  getConfig: (key: string) => string | undefined;
}

export interface DapDiscoveryResult {
  path: string;
  source: string;
}

/** The compiled-container extension the debug server's `launch` expects. */
export const CONTAINER_EXTENSION = '.iplc';

/*
 * The names of the things the debug integration talks to, in one place.
 *
 * Each of these strings also exists outside this file — the binary in the
 * compiler's `[[bin]]` target, the setting id in `package.json`, the binary
 * again in the E0007 message in `resources/problem-codes.csv` — and none of
 * those can import a TypeScript constant. Every copy the extension *can*
 * reach reads these constants instead of retyping the string, and the copies
 * it cannot reach are pinned to them by the guards in
 * `test/unit/debugAdapterLogic.test.ts`. Renaming the server is then a change
 * here plus whatever those guards report, not a grep.
 */

/** The debug-server executable, without any platform extension. */
export const DEBUG_SERVER_BINARY = 'ironplcvmd';

/** Environment variable that points directly at the debug-server binary. */
export const DEBUG_SERVER_ENV_VAR = 'IRONPLCVMD';

/** The configuration section every IronPLC setting lives under. */
export const CONFIG_SECTION = 'ironplc';

/** Setting key, within [`CONFIG_SECTION`], that points at the binary. */
export const DEBUG_SERVER_PATH_SETTING = 'debugServerPath';

/** The setting as users and `package.json` spell it. */
export const DEBUG_SERVER_PATH_SETTING_ID = `${CONFIG_SECTION}.${DEBUG_SERVER_PATH_SETTING}`;

/** The debug-server file name on `platform` (`.exe` on Windows). */
export function debugServerFileName(platform: string): string {
  return platform === 'win32' ? DEBUG_SERVER_BINARY + '.exe' : DEBUG_SERVER_BINARY;
}

/**
 * The E0007 context line: what to do when the debug server is not found.
 * Lives here so the setting id and binary name in the user-facing text come
 * from the constants above rather than from a hand-typed literal.
 */
export function debugServerNotFoundHint(): string {
  return `Set "${DEBUG_SERVER_PATH_SETTING_ID}" or install the IronPLC compiler `
    + `alongside ${DEBUG_SERVER_BINARY}.`;
}

/** A subset of a VS Code `contributes.languages` entry. */
export interface LanguageContribution {
  extensions?: string[];
}

/**
 * The set of source file extensions the debugger must compile before launching,
 * derived from the extension's own `contributes.languages` declarations. This
 * is the single source of truth: every language the extension registers (ST,
 * the TwinCAT dialects, and any future OOP/dialect additions) contributes its
 * extensions here automatically, so there is no hand-maintained copy to drift.
 * Extensions are lowercased for case-insensitive matching (e.g. `.TcPOU`).
 */
export function sourceExtensionsFromLanguages(
  languages: readonly LanguageContribution[],
): string[] {
  const extensions = new Set<string>();
  for (const language of languages) {
    for (const ext of language.extensions ?? []) {
      extensions.add(ext.toLowerCase());
    }
  }
  return [...extensions];
}

/**
 * How a launch `program` path should be handled:
 * - `source`: a Structured Text source that must be compiled to a container;
 * - `container`: an already-compiled `.iplc` that launches directly;
 * - `unknown`: neither — must be rejected rather than handed to the server,
 *   which would fail with an opaque "invalid magic number".
 */
export type ProgramKind = 'source' | 'container' | 'unknown';

/**
 * Classifies a launch `program` path by its extension. `sourceExtensions` is
 * the debugger's source-extension set (see [`sourceExtensionsFromLanguages`]).
 */
export function programKind(program: string, sourceExtensions: readonly string[]): ProgramKind {
  const ext = path.extname(program).toLowerCase();
  if (sourceExtensions.includes(ext)) {
    return 'source';
  }
  if (ext === CONTAINER_EXTENSION) {
    return 'container';
  }
  return 'unknown';
}

/**
 * True when `program` is a source file that must be compiled to an `.iplc`
 * container before debugging.
 */
export function isSourceProgram(program: string, sourceExtensions: readonly string[]): boolean {
  return programKind(program, sourceExtensions) === 'source';
}

/**
 * True when `program` can be debugged: a source file (compiled first) or an
 * already-compiled `.iplc` container. Anything else must be rejected before it
 * reaches the DAP server.
 */
export function isDebuggableProgram(program: string, sourceExtensions: readonly string[]): boolean {
  return programKind(program, sourceExtensions) !== 'unknown';
}

/**
 * The temporary `.iplc` path a source `program` is compiled to, placed under
 * `tmpDir`. The basename is derived from the source so the container is easy
 * to identify, and a fixed directory keeps repeated launches from leaking a
 * new file each time.
 */
export function containerOutputPath(program: string, tmpDir: string): string {
  const base = path.basename(program, path.extname(program));
  return path.join(tmpDir, base + CONTAINER_EXTENSION);
}

/**
 * Resolves the [`DEBUG_SERVER_BINARY`] executable. The resolution order is
 * environment variable, then setting, then bundled (alongside the discovered
 * `ironplcc` compiler, whose directory is `compilerDir`), matching
 * `specs/design/debugger-support.md` §"Phase 5". Returns the first candidate
 * that exists, or `undefined` if none is found.
 */
export function findDapServerPath(
  env: DapEnvironment,
  compilerDir?: string,
): DapDiscoveryResult | undefined {
  const exe = debugServerFileName(env.platform);

  const trials: (() => [string | undefined, string])[] = [
    () => {
      // Environment variable pointing directly at the binary. Not generally set.
      return [env.getEnv(DEBUG_SERVER_ENV_VAR), 'environment'];
    },
    () => {
      // Setting pointing directly at the binary.
      return [env.getConfig(DEBUG_SERVER_PATH_SETTING), 'configuration'];
    },
    () => {
      // Bundled next to the compiler discovered by `findCompilerPath`.
      return [compilerDir ? path.join(compilerDir, exe) : undefined, 'bundled'];
    },
  ];

  for (const trial of trials) {
    const [candidate, source] = trial();
    if (!candidate) {
      continue;
    }
    if (env.existsSync(candidate)) {
      return { path: candidate, source };
    }
  }

  return undefined;
}

/**
 * Fills in the launch program from the active editor when the launch
 * configuration omits it, so a bare `F5` on an open source file just works.
 * Returns `undefined` when neither the config nor the editor supplies a path.
 */
export function resolveProgramPath(
  configProgram: string | undefined,
  activeEditorPath: string | undefined,
): string | undefined {
  if (configProgram && configProgram.length > 0) {
    return configProgram;
  }
  return activeEditorPath;
}

/** The first non-empty, trimmed line of `text`, or the empty string. */
export function firstLine(text: string): string {
  for (const line of text.split('\n')) {
    const trimmed = line.trim();
    if (trimmed.length > 0) {
      return trimmed;
    }
  }
  return '';
}

/**
 * The message shown when a custom request is refused by the debug server.
 *
 * `DebugSession.customRequest` rejects when the adapter answers with
 * `success: false`, which is what the server returns (`requestNotApplicable`)
 * for a request that is legal only in another phase. The debug toolbar shows
 * its buttons for the whole session, so scan stepping can be pressed after the
 * program has terminated or faulted, when there is no cycle left to step.
 * Without this, the rejection escapes the command handler as an unhandled error
 * and the user sees no useful explanation. `title` is the command's UI title, so
 * the message names the button that was pressed.
 */
export function customRequestFailedMessage(title: string): string {
  return `IronPLC: "${title}" is only available while the program is paused.`;
}
