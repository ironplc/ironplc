import * as vscode from 'vscode';
import * as path from 'path';
import { existsSync } from 'fs';
import {
  LanguageClient,
  LanguageClientOptions,
  TransportKind,
  ServerOptions,
} from 'vscode-languageclient/node';
import { IplcEditorProvider } from './iplcEditorProvider';

const VERBOSITY = new Map<string, string[]>([
  ['ERROR', []],
  ['WARN', ['-v']],
  ['INFO', ['-v', '-v']],
  ['DEBUG', ['-v', '-v', '-v']],
  ['TRACE', ['-v', '-v', '-v', '-v']],
]);

let client: LanguageClient | undefined;

function openProblemInBrowser(code: string) {
  vscode.env.openExternal(vscode.Uri.parse('https://www.ironplc.com/vscode/problems/' + code + '.html'));
}

// This method is called when this extension is activated.
export function activate(context: vscode.ExtensionContext) {
  console.debug('Extension "ironplc" is activating!');

  context.subscriptions.push(vscode.commands.registerCommand('ironplc.createNewStructuredTextFile', async () => {
    await vscode.workspace.openTextDocument({ language: '61131-3-st' }).then((newFile) => {
      vscode.window.showTextDocument(newFile);
    });
  }));

  const compilerFilePath = findCompiler();
  if (!compilerFilePath) {
    return;
  }
  const config = vscode.workspace.getConfiguration('ironplc');

  client = createClient(compilerFilePath, config);

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

function findCompiler() {
  const ext = process.platform === 'win32' ? '.exe' : '';

  const trialGenerator = [
    () => {
      // Try to get from configuration
      const config = vscode.workspace.getConfiguration('ironplc');
      return [config.get<string | undefined>('path'), 'configuration'];
    },
    () => {
      // Try to get from environment variable. Not generally set.
      return [process.env.IRONPLC, 'environment'];
    },
    () => {
      // Mac well known directory
      const homebrewDir = process.platform === 'darwin' ? '/opt/homebrew/bin' : undefined;
      return [homebrewDir, 'homebrew'];
    },
    () => {
      // Windows user-install well-known path
      const name = 'localappdata';
      const localAppData = process.env.LOCALAPPDATA;

      if (process.platform !== 'win32' || !localAppData) {
        return [undefined, name];
      }
      const winAppDataDir = path.join(localAppData, 'Programs', 'IronPLC Compiler', 'bin');
      return [winAppDataDir, name];
    },
  ];

  let triedLocations: string[] = [];

  for (let trial of trialGenerator) {
    const result = trial();
    const testDir = result[0];
    const typeType = result[1];

    if (!testDir) {
      // If this returns falsy, then the trial is not valid and we continue
      continue;
    }

    const testExe = path.join(testDir, 'ironplcc' + ext);
    console.debug('Checking for IronPLC compiler at "' + testExe + '"');
    if (!existsSync(testExe)) {
      console.debug('IronPLC compiler not found at at "' + testExe + '"');
      triedLocations.push(typeType + ': (' + testExe + ')');
      // The file name doesn't exist
      continue;
    }

    console.log('Found IronPLC compiler using ' + typeType + ' at "' + testExe + '"');
    return testExe;
  }

  vscode.window.showErrorMessage('E0001 - Unable to locate IronPLC compiler. Tried ' + triedLocations.join(', ') + '. IronPLC is not installed or not configured.', 'Open Online Help')
    .then((item) => {
      openProblemInBrowser('E0001');
    });
  return undefined;
}

// This method is called when this extension is deactivated
export function deactivate(): Thenable<void> | undefined {
  console.log('Extension "ironplc" is deactivating!');

  if (!client) {
    return undefined;
  }
  return client.stop();
}
