import * as vscode from 'vscode';
import { existsSync } from 'fs';
import {
  LanguageClient,
  LanguageClientOptions,
  TransportKind,
  ServerOptions,
} from 'vscode-languageclient/node';
import { IplcEditorProvider } from './iplcEditorProvider';
import { CompilerEnvironment, findCompilerPath } from './compilerDiscovery';
import { ProblemCode, formatProblem } from './problems';

const VERBOSITY = new Map<string, string[]>([
  ['ERROR', []],
  ['WARN', ['-v']],
  ['INFO', ['-v', '-v']],
  ['DEBUG', ['-v', '-v', '-v']],
  ['TRACE', ['-v', '-v', '-v', '-v']],
]);

let client: LanguageClient | undefined;

function openProblemInBrowser(code: ProblemCode) {
  vscode.env.openExternal(vscode.Uri.parse('https://www.ironplc.com/reference/editor/problems/' + code + '.html'));
}

// This method is called when this extension is activated.
export function activate(context: vscode.ExtensionContext) {
  console.debug('Extension "ironplc" is activating!');

  context.subscriptions.push(vscode.commands.registerCommand('ironplc.createNewStructuredTextFile', async () => {
    await vscode.workspace.openTextDocument({ language: '61131-3-st' }).then((newFile) => {
      vscode.window.showTextDocument(newFile);
    });
  }));

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

  // Options to control the language client
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: '61131-3-st' },
      { scheme: 'file', language: 'plcopen-xml' },
      { scheme: 'file', language: 'twincat-pou' },
      { scheme: 'file', language: 'twincat-gvl' },
      { scheme: 'file', language: 'twincat-dut' },
    ],
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

  if (!client) {
    return undefined;
  }
  return client.stop();
}
