import * as vscode from 'vscode';
import { existsSync } from 'fs';
import {
  LanguageClient,
  LanguageClientOptions,
  TransportKind,
  ServerOptions,
} from 'vscode-languageclient/node';
import { IplcEditorProvider } from './iplcEditorProvider';
import { IronplcTaskProvider } from './ironplcTaskProvider';
import { CompilerEnvironment, findCompilerPath } from './compilerDiscovery';
import { ProblemCode, formatProblem } from './problems';
import { RunSession, RunState } from './runSession';
import { findProgramLenses } from './runCodeLensProvider';

/**
 * Reactive code lens provider for PROGRAM declarations. Shows "Run Program"
 * when idle and toggles to "Pause"/"Stop" (or "Resume"/"Stop") as the run
 * session transitions states.
 */
class RunProgramCodeLensProvider implements vscode.CodeLensProvider {
  private _state: RunState = 'idle';
  private readonly _emitter = new vscode.EventEmitter<void>();
  readonly onDidChangeCodeLenses = this._emitter.event;

  constructor(private readonly hasCompiler: () => boolean) {}

  setState(state: RunState): void {
    if (this._state === state) {
      return;
    }
    this._state = state;
    this._emitter.fire();
  }

  provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
    const lenses = findProgramLenses(document.getText(), this._state, this.hasCompiler());
    return lenses.map((lens) => {
      const range = new vscode.Range(
        new vscode.Position(lens.range.start.line, lens.range.start.character),
        new vscode.Position(lens.range.end.line, lens.range.end.character),
      );
      return new vscode.CodeLens(range, lens.command ? {
        title: lens.command.title,
        command: lens.command.command,
        arguments: lens.command.arguments as unknown[] | undefined,
      } : undefined);
    });
  }

  dispose(): void {
    this._emitter.dispose();
  }
}

const VERBOSITY = new Map<string, string[]>([
  ['ERROR', []],
  ['WARN', ['-v']],
  ['INFO', ['-v', '-v']],
  ['DEBUG', ['-v', '-v', '-v']],
  ['TRACE', ['-v', '-v', '-v', '-v']],
]);

let client: LanguageClient | undefined;
let runSession: RunSession | undefined;

function openProblemInBrowser(code: ProblemCode) {
  const ext = vscode.extensions.getExtension('ironplc.ironplc');
  const version = ext?.packageJSON?.version ?? '';
  const url = 'https://www.ironplc.com/reference/editor/problems/' + code + '.html?version=' + encodeURIComponent(version);
  vscode.env.openExternal(vscode.Uri.parse(url));
}

// This method is called when this extension is activated.
export function activate(context: vscode.ExtensionContext) {
  console.debug('Extension "ironplc" is activating!');

  context.subscriptions.push(vscode.commands.registerCommand('ironplc.createNewStructuredTextFile', async () => {
    await vscode.workspace.openTextDocument({ language: '61131-3-st' }).then((newFile) => {
      vscode.window.showTextDocument(newFile);
    });
  }));

  // Register run commands unconditionally so they exist even without a
  // compiler (the commands gracefully no-op when no client is available).
  registerRunSupport(context);

  const env: CompilerEnvironment = {
    platform: process.platform,
    existsSync: existsSync,
    getEnv: (name: string) => process.env[name],
    getConfig: (key: string) => vscode.workspace.getConfiguration('ironplc').get<string>(key),
  };

  const result = findCompilerPath(env);
  if (!result) {
    vscode.window.showErrorMessage(
      formatProblem(ProblemCode.NoCompiler, 'IronPLC is not installed or not configured.'),
      'Open Online Help',
    ).then(() => {
      openProblemInBrowser(ProblemCode.NoCompiler);
    });
    return;
  }

  context.subscriptions.push(
    vscode.tasks.registerTaskProvider(
      IronplcTaskProvider.type,
      new IronplcTaskProvider(result.path),
    ),
  );

  const config = vscode.workspace.getConfiguration('ironplc');
  client = createClient(result.path, config);

  if (client) {
    client.start();
    context.subscriptions.push(IplcEditorProvider.register(context, client));
    console.debug('Extension "ironplc" is active!');
  }
  else {
    console.error('Extension "ironplc" is NOT active!');
  }
}

function registerRunSupport(context: vscode.ExtensionContext) {
  // Output channel for variable display
  const outputChannel = vscode.window.createOutputChannel('IronPLC Run');
  context.subscriptions.push(outputChannel);

  // Status bar items
  const pauseItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  pauseItem.command = 'ironplc.pauseProgram';
  context.subscriptions.push(pauseItem);

  const stopItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 99);
  stopItem.text = '$(debug-stop) Stop';
  stopItem.tooltip = 'Stop program execution';
  stopItem.command = 'ironplc.stopProgram';
  context.subscriptions.push(stopItem);

  // CodeLens provider (reactive — refreshes when the run state changes)
  const runLensProvider = new RunProgramCodeLensProvider(() => !!client);
  context.subscriptions.push(runLensProvider);

  function updateStatusBar(state: RunState) {
    if (state === 'running') {
      pauseItem.text = '$(debug-pause) Pause';
      pauseItem.tooltip = 'Pause program execution';
      pauseItem.show();
      stopItem.show();
    } else if (state === 'paused') {
      pauseItem.text = '$(debug-continue) Resume';
      pauseItem.tooltip = 'Resume program execution';
      pauseItem.show();
      stopItem.show();
    } else {
      pauseItem.hide();
      stopItem.hide();
    }
    runLensProvider.setState(state);
  }

  // Initially hidden
  updateStatusBar('idle');

  const stSelector: vscode.DocumentSelector = [
    { scheme: 'file', language: '61131-3-st' },
    { scheme: 'file', language: 'twincat-pou' },
  ];

  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider(stSelector, runLensProvider),
  );

  // Run command
  context.subscriptions.push(
    vscode.commands.registerCommand('ironplc.runProgram', async () => {
      if (!client) {
        vscode.window.showErrorMessage(
          formatProblem(ProblemCode.NoCompiler, 'Install the compiler to run programs.'),
          'Open Online Help',
        ).then((selection) => {
          if (selection === 'Open Online Help') {
            openProblemInBrowser(ProblemCode.NoCompiler);
          }
        });
        return;
      }

      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        vscode.window.showWarningMessage('No active editor with a program to run.');
        return;
      }

      const source = editor.document.getText();

      // Dispose previous session
      if (runSession) {
        runSession.dispose();
      }

      runSession = new RunSession(client, {
        onStateChange: updateStatusBar,
        onVariablesUpdate(variables, totalScans) {
          outputChannel.clear();
          outputChannel.appendLine(`Scan cycle: ${totalScans}`);
          outputChannel.appendLine('---');
          for (const v of variables) {
            const label = v.name || `var[${v.index}]`;
            const typeSuffix = v.type_name ? ` : ${v.type_name}` : '';
            outputChannel.appendLine(`  ${label}${typeSuffix} = ${v.value}`);
          }
        },
        onError(message) {
          vscode.window.showErrorMessage(`IronPLC Run: ${message}`);
          outputChannel.appendLine(`ERROR: ${message}`);
        },
      });

      outputChannel.show(true);
      await runSession.start(source);
    }),
  );

  // Pause/resume command
  context.subscriptions.push(
    vscode.commands.registerCommand('ironplc.pauseProgram', () => {
      if (!runSession) {
        return;
      }
      if (runSession.getState() === 'running') {
        runSession.pause();
      } else if (runSession.getState() === 'paused') {
        runSession.resume();
      }
    }),
  );

  // Stop command
  context.subscriptions.push(
    vscode.commands.registerCommand('ironplc.stopProgram', async () => {
      if (runSession) {
        await runSession.stop();
      }
    }),
  );
}

function createClient(compilerFilePath: string, config: vscode.WorkspaceConfiguration) {
  let args = [];

  // Add the log level
  const logLevel = config.get<string>('logLevel', 'ERROR');
  const logVerbosity = VERBOSITY.get(logLevel) || [];
  args.push(...logVerbosity);

  // Override the log file if set
  const logFile = config.get<string>('logFile', '');
  if (logFile) {
    args.push('--log-file', logFile);
  }

  args.push('lsp');
  console.debug('Extension "ironplc" starting with args: ' + args);

  const application = {
    command: compilerFilePath,
    transport: TransportKind.stdio,
    args: args,
    options: {
      env: ['RUST_LOG=lsp_server=debug'],
    },
  };

  const serverOptions: ServerOptions = application;

  // Read the dialect setting
  const dialect = config.get<string>('dialect', 'iec61131-3-ed2');

  // Options to control the language client
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: '61131-3-st' },
      { scheme: 'file', language: 'plcopen-xml' },
      { scheme: 'file', language: 'twincat-pou' },
      { scheme: 'file', language: 'twincat-gvl' },
      { scheme: 'file', language: 'twincat-dut' },
    ],
    initializationOptions: {
      dialect: dialect,
    },
  };

  // Create the language client and start the client.
  const client = new LanguageClient(
    'ironplc',
    'IronPLC',
    serverOptions,
    clientOptions
  );

  return client;
}

// This method is called when this extension is deactivated
export function deactivate(): Thenable<void> | undefined {
  console.log('Extension "ironplc" is deactivating!');

  if (runSession) {
    runSession.dispose();
    runSession = undefined;
  }

  if (!client) {
    return undefined;
  }
  return client.stop();
}
