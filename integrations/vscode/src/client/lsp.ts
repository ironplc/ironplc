import * as vscode from 'vscode';
import {
	LanguageClient,
	LanguageClientOptions,
	ServerOptions,
	TransportKind
} from 'vscode-languageclient/node';

const VERBOSITY = new Map<string, string[]>([
	["ERROR", []],
	["WARN", ["-v"]],
	["INFO", ["-v", "-v"]],
	["DEBUG", ["-v", "-v", "-v"]],
	["TRACE", ["-v", "-v", "-v", "-v"]],
]);

export function createClient(compilerFilePath: string, config: vscode.WorkspaceConfiguration) {
	const logLevel = config.get<string>('logLevel', 'ERROR');
	const logVerbosity = VERBOSITY.get(logLevel) || [];

	const args = logVerbosity.concat(['lsp']);
	console.log('Extension "ironplc" starting with args' + args);

	const application = {
		command: compilerFilePath,
		transport: TransportKind.stdio,
		args: args,
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
	const client = new LanguageClient(
		'ironplc',
		'IronPLC',
		serverOptions,
		clientOptions
	);

	return client;
}
