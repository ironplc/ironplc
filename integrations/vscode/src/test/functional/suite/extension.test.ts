import * as assert from 'assert';
import * as vscode from 'vscode';
import * as path from 'path';

suite('Extension Test Suite', () => {
  vscode.window.showInformationMessage('Start all tests.');

  teardown(closeActiveWindows);

  test('ironplc.reateNewStructuredTextFile sets 61131-3-st as language ID', async () => {
    await vscode.commands.executeCommand('ironplc.createNewStructuredTextFile');
    const stFile = vscode.window.activeTextEditor!.document as any | undefined;
    const languageId = stFile.languageId;

    assert.equal(languageId, '61131-3-st');
  });

  test('detects ST extension as 61131-3-st', async () => {
    const filePath = testResourcePath('valid.st');
    const textDocument = await vscode.workspace.openTextDocument(filePath);
    await vscode.window.showTextDocument(textDocument);
    const stFile = vscode.window.activeTextEditor!.document as any | undefined;
    const languageId = stFile.languageId;

    assert.equal(languageId, '61131-3-st');
  });

  test('does not detect non-ST extension as 61131-3-st', async () => {
    const filePath = testResourcePath('invalid-ext.notst');
    const textDocument = await vscode.workspace.openTextDocument(filePath);
    await vscode.window.showTextDocument(textDocument);
    const stFile = vscode.window.activeTextEditor!.document as any | undefined;
    const languageId = stFile.languageId;

    assert.notEqual(languageId, '61131-3-st');
  });
});

function testResourcePath(fileName: string): string {
  const testRootDir = path.join(__dirname, '..', '..', '..', '..');
  return path.join(testRootDir, 'src', 'test', 'functional', 'resources', fileName);
}

async function closeActiveWindows(): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    // Attempt to fix #1301.
    // Lets not waste too much time.
    const timer = setTimeout(() => {
      reject(new Error('Command \'workbench.action.closeAllEditors\' timed out'));
    }, 15000);
    vscode.commands.executeCommand('workbench.action.closeAllEditors').then(
      () => {
        clearTimeout(timer);
        resolve();
      },
      (ex) => {
        clearTimeout(timer);
        reject(ex);
      },
    );
  });
}
