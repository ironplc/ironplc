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
  isSourceProgram,
  resolveProgramPath,
} from './debugAdapterLogic';

/** The debug type used in `launch.json` and the `debuggers` contribution. */
export const IRONPLC_DEBUG_TYPE = 'ironplc';

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
 */
export class IronplcDebugConfigurationProvider
implements vscode.DebugConfigurationProvider {
  constructor(private readonly compilerPath: string) {}

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

    const activeEditorPath = vscode.window.activeTextEditor?.document.uri.fsPath;
    const program = resolveProgramPath(config.program, activeEditorPath);
    if (!program) {
      return vscode.window
        .showErrorMessage('IronPLC: no program to debug. Open a Structured Text file or set "program" in launch.json.')
        .then(() => undefined);
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

    // Already a compiled container: launch it as-is.
    if (!isSourceProgram(program)) {
      return config;
    }

    const output = containerOutputPath(program, os.tmpdir());
    try {
      await this.compile(program, output);
    } catch (err) {
      const reason = err instanceof Error ? err.message : String(err);
      await vscode.window.showErrorMessage(`IronPLC: failed to compile "${program}" for debugging: ${reason}`);
      return undefined;
    }
    config.program = output;
    return config;
  }

  /** Runs `ironplcc compile` to emit a debug-enabled container. */
  private compile(program: string, output: string): Promise<void> {
    const args = buildDebugCompileArgs(program, output);
    return new Promise((resolve, reject) => {
      execFile(this.compilerPath, args, (error, _stdout, stderr) => {
        if (error) {
          reject(new Error(stderr?.trim() || error.message));
          return;
        }
        resolve();
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
  constructor(private readonly compilerDir: string | undefined) {}

  createDebugAdapterDescriptor(
    _session: vscode.DebugSession,
  ): vscode.ProviderResult<vscode.DebugAdapterDescriptor> {
    const server = this.resolveServer();
    if (!server) {
      void vscode.window.showErrorMessage('IronPLC: the debug server (ironplcdap) was not found. Set "ironplc.dapServerPath" or install the IronPLC compiler.');
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
