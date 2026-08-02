import * as vscode from 'vscode';
import * as os from 'os';
import { existsSync } from 'fs';
import { execFile } from 'child_process';
import {
  DapEnvironment,
  DapDiscoveryResult,
  buildDebugCompileArgs,
  containerOutputPath,
  findDapServerPath,
  firstLine,
  isDebuggableProgram,
  programKind,
  resolveProgramPath,
} from './debugAdapterLogic';
import { ProblemCode } from './problems';

/** The debug type used in `launch.json` and the `debuggers` contribution. */
export const IRONPLC_DEBUG_TYPE = 'ironplc';

/**
 * Reports a coded problem to the user (typically a notification with an
 * "Open Online Help" action). Injected so this module stays decoupled from the
 * extension's activation-time state (the extension version used to build the
 * help URL).
 */
export type ReportProblem = (code: ProblemCode, context?: string) => void;

/** Result of running the compiler for a debug launch. */
interface CompileResult {
  code: number | null;
  stdout: string;
  stderr: string;
}

/**
 * Fills in launch-configuration defaults and compiles a source program to an
 * `.iplc` container before the session starts.
 *
 * A user may press F5 with no `launch.json` at all (VS Code then passes an
 * empty configuration), so `resolveDebugConfiguration` supplies the standard
 * `type`/`request`/`name` and defaults `program` to the active editor.
 * `resolveDebugConfigurationWithSubstitutedVariables` runs after VS Code has
 * expanded `${file}`-style variables; that is where the source→container
 * compile happens, so the adapter always receives a compiled `.iplc` path.
 *
 * Every failure path logs to the output channel and reports a coded problem so
 * a launch that aborts is never silent.
 */
export class IronplcDebugConfigurationProvider
implements vscode.DebugConfigurationProvider {
  constructor(
    private readonly compilerPath: string,
    private readonly reportProblem: ReportProblem,
    private readonly log: vscode.OutputChannel,
  ) {}

  resolveDebugConfiguration(
    _folder: vscode.WorkspaceFolder | undefined,
    config: vscode.DebugConfiguration,
  ): vscode.ProviderResult<vscode.DebugConfiguration> {
    // A completely empty configuration means "debug the active file".
    if (!config.type && !config.request && !config.name) {
      config.type = IRONPLC_DEBUG_TYPE;
      config.request = 'launch';
      config.name = 'IronPLC: Debug Active File';
    }

    // Fall back to the active editor only when it is itself a debuggable file,
    // so pressing Run from the launch.json editor does not pick launch.json as
    // the program. A literal "${file}" is kept as-is for VS Code to substitute.
    const active = vscode.window.activeTextEditor?.document.uri.fsPath;
    const activeDebuggable = active && isDebuggableProgram(active) ? active : undefined;
    const program = resolveProgramPath(config.program, activeDebuggable);
    if (!program) {
      this.reportProblem(ProblemCode.DebugNoProgram, 'Open a Structured Text file, or set "program" to a .st or .iplc path in launch.json.');
      return undefined;
    }
    config.program = program;
    return config;
  }

  async resolveDebugConfigurationWithSubstitutedVariables(
    _folder: vscode.WorkspaceFolder | undefined,
    config: vscode.DebugConfiguration,
  ): Promise<vscode.DebugConfiguration | undefined> {
    const program: string | undefined = config.program;
    if (!program) {
      return undefined;
    }

    switch (programKind(program)) {
      case 'container':
        // Already a compiled `.iplc`: launch it as-is.
        this.log.appendLine(`Launching compiled container: ${program}`);
        return config;
      case 'unknown':
        // Not a source file or a container. Reject with a clear message rather
        // than handing it to the server, which would fail with an opaque
        // "invalid magic number".
        this.log.appendLine(`Cannot debug "${program}": not a .st source or .iplc container.`);
        this.reportProblem(ProblemCode.DebugProgramNotDebuggable, `"${program}" — set "program" in launch.json to a .st or .iplc path.`);
        return undefined;
      case 'source':
        break;
    }

    const output = containerOutputPath(program, os.tmpdir());
    const result = await this.compile(program, output);
    if (result.code !== 0) {
      // Surface the compiler's own diagnostics: a single file that references
      // POUs or types defined in sibling files fails analysis on its own.
      this.log.appendLine(`Compilation failed (exit ${result.code}). The program must compile on its own — a file that references POUs or types from other files will not compile in isolation.`);
      this.log.show(true);
      const detail = firstLine(result.stderr) || firstLine(result.stdout) || `compiler exited with ${result.code}`;
      this.reportProblem(ProblemCode.DebugCompileFailed, `${program}: ${detail} (see the "IronPLC Debug" output for details).`);
      return undefined;
    }

    this.log.appendLine(`Compiled ${program} -> ${output}`);
    config.program = output;
    return config;
  }

  /** Runs `ironplcc compile` and captures its exit code and output. */
  private compile(program: string, output: string): Promise<CompileResult> {
    const args = buildDebugCompileArgs(program, output);
    this.log.appendLine(`$ ${this.compilerPath} ${args.join(' ')}`);
    return new Promise((resolve) => {
      execFile(this.compilerPath, args, (error, stdout, stderr) => {
        if (stdout) {
          this.log.append(stdout);
        }
        if (stderr) {
          this.log.append(stderr);
        }
        // execFile reports a non-zero exit as an error carrying `code`; a spawn
        // failure (e.g. the compiler is missing) has no numeric code.
        const code = error && typeof (error as { code?: unknown }).code === 'number'
          ? (error as { code: number }).code
          : (error ? null : 0);
        resolve({ code, stdout: stdout ?? '', stderr: stderr ?? '' });
      });
    });
  }
}

/**
 * Produces the DAP adapter executable. The `ironplcdap` binary speaks DAP over
 * stdin/stdout and takes no arguments — the program under debug is delivered by
 * the `launch` request, not the command line.
 */
export class IronplcDebugAdapterDescriptorFactory
implements vscode.DebugAdapterDescriptorFactory {
  constructor(
    private readonly compilerDir: string | undefined,
    private readonly reportProblem: ReportProblem,
  ) {}

  createDebugAdapterDescriptor(
    _session: vscode.DebugSession,
  ): vscode.ProviderResult<vscode.DebugAdapterDescriptor> {
    const server = this.resolveServer();
    if (!server) {
      this.reportProblem(ProblemCode.DebugServerNotFound, 'Set "ironplc.dapServerPath" or install the IronPLC compiler alongside ironplcdap.');
      return undefined;
    }
    return new vscode.DebugAdapterExecutable(server.path, []);
  }

  private resolveServer(): DapDiscoveryResult | undefined {
    const env: DapEnvironment = {
      platform: process.platform,
      existsSync: existsSync,
      getEnv: name => process.env[name],
      getConfig: key => vscode.workspace.getConfiguration('ironplc').get<string>(key),
    };
    return findDapServerPath(env, this.compilerDir);
  }
}
