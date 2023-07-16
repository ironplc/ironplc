import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';

const IRONPLC_EXTENSION_ID = 'garretfick.ironplc';

suite('Extension Test Suite', () => {
	vscode.window.showInformationMessage('Start all tests.');

	test('ironplc.reateNewStructuredTextFile sets 61131-3-st as language ID', async () => {
		await vscode.commands.executeCommand('ironplc.createNewStructuredTextFile');
		const stFile = (vscode.window.activeTextEditor!.document as any | undefined);
		const languageId = stFile.languageId;

		assert.equal(languageId, '61131-3-st');
	});

	test('detects ST extension as 61131-3-st', async () => {
		const filePath = testResourcePath('valid.st');
		const textDocument = await vscode.workspace.openTextDocument(filePath);
		await vscode.window.showTextDocument(textDocument);
		const stFile = (vscode.window.activeTextEditor!.document as any | undefined);
		const languageId = stFile.languageId;

		assert.equal(languageId, '61131-3-st');
	});

	test('does not detect non-ST extension as 61131-3-st', async () => {
		const filePath = testResourcePath('invalid-ext.notst');
		const textDocument = await vscode.workspace.openTextDocument(filePath);
		await vscode.window.showTextDocument(textDocument);
		const stFile = (vscode.window.activeTextEditor!.document as any | undefined);
		const languageId = stFile.languageId;

		assert.notEqual(languageId, '61131-3-st');
	});
});

function testResourcePath(fileName: string): string {
	const testRootDir = path.join(__dirname, '..', '..', '..');
	return path.join(testRootDir, 'src', 'test', 'resources', fileName);
}
