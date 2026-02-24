import * as vscode from 'vscode';
import { LanguageClient, State } from 'vscode-languageclient/node';
import { DisassemblyResult, getErrorHtml, getDisassemblyHtml } from './iplcRendering';

interface IplcDocument extends vscode.CustomDocument {
  readonly uri: vscode.Uri;
}

export class IplcEditorProvider implements vscode.CustomReadonlyEditorProvider<IplcDocument> {
  public static readonly viewType = 'ironplc.iplcViewer';

  constructor(private readonly client: LanguageClient) {}

  public static register(_context: vscode.ExtensionContext, client: LanguageClient): vscode.Disposable {
    const provider = new IplcEditorProvider(client);
    return vscode.window.registerCustomEditorProvider(
      IplcEditorProvider.viewType,
      provider,
      { supportsMultipleEditorsPerDocument: true },
    );
  }

  openCustomDocument(uri: vscode.Uri): IplcDocument {
    return { uri, dispose: () => {} };
  }

  async resolveCustomEditor(
    document: IplcDocument,
    webviewPanel: vscode.WebviewPanel,
  ): Promise<void> {
    webviewPanel.webview.options = { enableScripts: false };

    // The LSP client may still be starting when the custom editor opens.
    // Wait briefly for it to reach the Running state.
    if (!this.client.isRunning()) {
      const ready = await this.waitForClient(5000);
      if (!ready) {
        webviewPanel.webview.html = getErrorHtml(
          'E0002 - IronPLC compiler not found. Install the compiler to view .iplc files.',
        );
        return;
      }
    }

    try {
      const result = await this.client.sendRequest('ironplc/disassemble', {
        uri: document.uri.toString(),
      });
      webviewPanel.webview.html = getDisassemblyHtml(result as DisassemblyResult);
    }
    catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      webviewPanel.webview.html = getErrorHtml(`E0003 - Failed to disassemble .iplc file: ${message}`);
    }
  }

  private waitForClient(timeoutMs: number): Promise<boolean> {
    if (this.client.isRunning()) {
      return Promise.resolve(true);
    }
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        disposable.dispose();
        resolve(false);
      }, timeoutMs);

      const disposable = this.client.onDidChangeState((e) => {
        if (e.newState === State.Running) {
          clearTimeout(timer);
          disposable.dispose();
          resolve(true);
        }
        else if (e.newState === State.Stopped) {
          clearTimeout(timer);
          disposable.dispose();
          resolve(false);
        }
      });
    });
  }
}
