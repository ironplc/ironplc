import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';
import { LanguageClientLike, resolveEditorContent } from './iplcEditorLogic';

interface IplcDocument extends vscode.CustomDocument {
  readonly uri: vscode.Uri;
}

export class IplcEditorProvider implements vscode.CustomReadonlyEditorProvider<IplcDocument> {
  public static readonly viewType = 'ironplc.iplcViewer';

  constructor(private readonly client: LanguageClientLike) {}

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
    webviewPanel.webview.html = await resolveEditorContent(
      this.client,
      document.uri.toString(),
    );
  }
}
