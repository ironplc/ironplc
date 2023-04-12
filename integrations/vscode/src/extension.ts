import * as vscode from 'vscode';
import * as path from 'path';
import { existsSync } from 'fs';
import {
	Executable,
	LanguageClient,
	LanguageClientOptions,
	ServerOptions,
	TransportKind
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

// This method is called when this extension is activated.
export function activate(context: vscode.ExtensionContext) {
	console.log('Extension "ironplc" is activating!');

	// The command has been defined in the package.json file
	// Now provide the implementation of the command with registerCommand
	// The commandId parameter must match the command field in package.json
	let disposable = vscode.commands.registerCommand('ironplc.helloWorld', () => {
		// The code you place here will be executed every time your command is executed
		// Display a message box to the user
		vscode.window.showInformationMessage('Hello World from IronPLC!');
	});

	context.subscriptions.push(disposable);
	startServer(context);

	console.log('Extension "ironplc" is active!');	
}

function startServer(context: vscode.ExtensionContext) {
	const compilerFilePath = findCompiler();
	if (!compilerFilePath) {
		return;
	}

	const application: Executable = {
		command: compilerFilePath,
		transport: TransportKind.stdio,
		args: ['lsp'],
		options: {
			env: ['RUST_LOG=lsp_server=debug']
		}
	};

	const serverOptions: ServerOptions = application;

	// Options to control the language client
	const clientOptions: LanguageClientOptions = {
		// Register the server for plain text documents
		documentSelector: [{ scheme: 'file', language: 'st', pattern: '*.st' }],
		synchronize: {
			// Notify the server about file changes to '.clientrc files contained in the workspace
			fileEvents: vscode.workspace.createFileSystemWatcher('**/.clientrc')
		}
	};

	// Create the language client and start the client.
	client = new LanguageClient(
		'ironplc',
		'IronPLC',
		serverOptions,
		clientOptions
	);
}

function findCompiler() {
	const ext = process.platform === 'win32' ? '.exe' : '';

	const trialGenerator = [
		() => {
			// Try to get from configuration
			const config = vscode.workspace.getConfiguration("ironplc");
			return [config.get<string|undefined>('path'), 'configuration'];
		},
		() => {
			// Try to get from environment variable
			return [process.env.IRONPLC, 'environment'];
		}
	];

	for (let trial of trialGenerator) {
		const result = trial();
		const testDir = result[0];
		if (!testDir) {
			// If this returns falsy, then the trial is not valid and we continue
			continue;
		}

		const testExe = path.join(testDir, 'ironplcc' + ext);
		if (!existsSync(testExe)) {
			// The file name doesn't exist
			continue;
		}

		console.log('Found IronPLC compiler using ' + result[1] + ' at "' + testExe + '"');
		return testExe;
	}

	console.log('Did not find IronPLC compiler');
	return undefined;
}

// This method is called when this extension is deactivated
export function deactivate() : Thenable<void> | undefined {
	if (!client) {
		return undefined;
	}
	return client.stop();
}
