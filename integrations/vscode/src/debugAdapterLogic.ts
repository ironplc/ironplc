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

/**
 * Source file extensions the debugger must compile to an `.iplc` container
 * before launching. A `.iplc` program is already compiled and launches
 * directly. Matched case-insensitively so the TwinCAT extensions (`.TcPOU`,
 * …) resolve regardless of how the path is cased on disk.
 */
export const SOURCE_EXTENSIONS: string[] = [
  '.st',
  '.iec',
  '.tcpou',
  '.tcgvl',
  '.tcdut',
];

/** The compiled-container extension the DAP server's `launch` expects. */
export const CONTAINER_EXTENSION = '.iplc';

/**
 * How a launch `program` path should be handled:
 * - `source`: a Structured Text source that must be compiled to a container;
 * - `container`: an already-compiled `.iplc` that launches directly;
 * - `unknown`: neither — must be rejected rather than handed to the server,
 *   which would fail with an opaque "invalid magic number".
 */
export type ProgramKind = 'source' | 'container' | 'unknown';

/** Classifies a launch `program` path by its extension. */
export function programKind(program: string): ProgramKind {
  const ext = path.extname(program).toLowerCase();
  if (SOURCE_EXTENSIONS.includes(ext)) {
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
export function isSourceProgram(program: string): boolean {
  return programKind(program) === 'source';
}

/**
 * True when `program` can be debugged: a source file (compiled first) or an
 * already-compiled `.iplc` container. Anything else must be rejected before it
 * reaches the DAP server.
 */
export function isDebuggableProgram(program: string): boolean {
  return programKind(program) !== 'unknown';
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
 * Resolves the `ironplcdap` DAP-server binary. The resolution order is
 * environment variable, then setting, then bundled (alongside the discovered
 * `ironplcc` compiler, whose directory is `compilerDir`), matching
 * `specs/design/debugger-support.md` §"Phase 5". Returns the first candidate
 * that exists, or `undefined` if none is found.
 */
export function findDapServerPath(
  env: DapEnvironment,
  compilerDir?: string,
): DapDiscoveryResult | undefined {
  const ext = env.platform === 'win32' ? '.exe' : '';
  const exe = 'ironplcdap' + ext;

  const trials: (() => [string | undefined, string])[] = [
    () => {
      // Environment variable pointing directly at the binary. Not generally set.
      return [env.getEnv('IRONPLCDAP'), 'environment'];
    },
    () => {
      // Setting pointing directly at the binary.
      return [env.getConfig('dapServerPath'), 'configuration'];
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

/**
 * Arguments for compiling `program` to the `output` container with `ironplcc`.
 * The compiler always emits a debug section, which the DAP `launch`
 * precondition requires (`compiler/vm-cli/src/dap/launch.rs`).
 */
export function buildDebugCompileArgs(program: string, output: string): string[] {
  return ['compile', program, '-o', output];
}

/**
 * The reply shape of an `ironplc/scanCount` custom request. The count field is
 * optional so a server that does not yet implement the request (returning an
 * empty body) is handled gracefully.
 */
export interface ScanCountResponse {
  scanCount?: number;
}

/** Formats an `ironplc/scanCount` reply for display in the UI. */
export function scanCountMessage(response: ScanCountResponse | undefined): string {
  const count = response?.scanCount;
  if (typeof count !== 'number') {
    return 'IronPLC: scan count is not available.';
  }
  return `IronPLC: scan cycle ${count}`;
}
