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
	context.subscriptions.push(vscode.commands.registerCommand('ironplc.analyzeFile', () => {
		// The code you place here will be executed every time your command is executed
		// Display a message box to the user
		vscode.window.showInformationMessage('Hello World from IronPLC!');
	}));

  	context.subscriptions.push(vscode.commands.registerCommand("ironplc.createNewStructuredTextFile", () => {
		vscode.workspace.openTextDocument({ language: "61131-3-st"}).then(newFile => {
			vscode.window.showTextDocument(newFile);
		});
	}));

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
		documentSelector: [{ scheme: 'file', language: '61131-3-st' }]
	};

	// Create the language client and start the client.
	client = new LanguageClient(
		'ironplc',
		'IronPLC',
		serverOptions,
		clientOptions
	);

	client.start();
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
		},
		() => {
			const homebrewDir = process.platform === 'darwin' ? '/opt/homebrew/bin' : undefined;
			return [homebrewDir, 'homebrew'];
		}
	];

	let triedLocations : string[] = [];

	for (let trial of trialGenerator) {
		const result = trial();
		const testDir = result[0];
		const typeType = result[1];
		if (!testDir) {
			// If this returns falsy, then the trial is not valid and we continue
			continue;
		}

		const testExe = path.join(testDir, 'ironplcc' + ext);
		if (!existsSync(testExe)) {
			triedLocations.push(typeType + ": " + testExe);
			// The file name doesn't exist
			continue;
		}

		console.log('Found IronPLC compiler using ' + typeType + ' at "' + testExe + '"');
		return testExe;
	}

	vscode.window.showErrorMessage('Unable to locate IronPLC compiler after searching ' + triedLocations + '. IronPLC is not installed or not configured.');
	return undefined;
}

// This method is called when this extension is deactivated
export function deactivate() : Thenable<void> | undefined {
	console.log('Extension "ironplc" is deactivating!');

	if (!client) {
		return undefined;
	}
	return client.stop();
}
