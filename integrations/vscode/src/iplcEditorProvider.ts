import * as vscode from 'vscode';
import { LanguageClient } from 'vscode-languageclient/node';

interface IplcDocument extends vscode.CustomDocument {
  readonly uri: vscode.Uri;
}

export class IplcEditorProvider implements vscode.CustomReadonlyEditorProvider<IplcDocument> {
  public static readonly viewType = 'ironplc.iplcViewer';

  constructor(private readonly client: LanguageClient) {}

  public static register(context: vscode.ExtensionContext, client: LanguageClient): vscode.Disposable {
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

    if (!this.client.isRunning()) {
      webviewPanel.webview.html = this.getErrorHtml(
        'IronPLC compiler not found. Install the compiler to view .iplc files.',
      );
      return;
    }

    try {
      const result = await this.client.sendRequest('ironplc/disassemble', {
        uri: document.uri.toString(),
      });
      webviewPanel.webview.html = this.getDisassemblyHtml(result as DisassemblyResult);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      webviewPanel.webview.html = this.getErrorHtml(`Failed to disassemble: ${message}`);
    }
  }

  private getErrorHtml(message: string): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <style>
    body {
      background: var(--vscode-editor-background);
      color: var(--vscode-editor-foreground);
      font-family: var(--vscode-font-family);
      padding: 20px;
    }
    .error { color: var(--vscode-errorForeground); font-size: 14px; }
  </style>
</head>
<body>
  <p class="error">${escapeHtml(message)}</p>
</body>
</html>`;
  }

  private getDisassemblyHtml(data: DisassemblyResult): string {
    if (data.error) {
      return this.getErrorHtml(data.error);
    }

    const headerHtml = this.renderHeader(data.header);
    const constantsHtml = this.renderConstants(data.constants);
    const functionsHtml = this.renderFunctions(data.functions);

    return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <style>
    body {
      background: var(--vscode-editor-background);
      color: var(--vscode-editor-foreground);
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
      padding: 16px;
      line-height: 1.5;
    }
    h1 { font-size: 1.4em; margin: 0 0 16px 0; }
    details { margin: 8px 0; }
    summary {
      cursor: pointer;
      font-weight: bold;
      padding: 4px 0;
      user-select: none;
    }
    summary:hover { color: var(--vscode-textLink-foreground); }
    table {
      border-collapse: collapse;
      width: 100%;
      margin: 4px 0;
    }
    th, td {
      text-align: left;
      padding: 3px 12px 3px 0;
      border-bottom: 1px solid var(--vscode-panel-border);
      font-size: 0.95em;
    }
    th {
      color: var(--vscode-descriptionForeground);
      font-weight: 600;
    }
    .mono {
      font-family: var(--vscode-editor-font-family);
      font-size: var(--vscode-editor-font-size);
    }
    .offset { color: var(--vscode-descriptionForeground); }
    .op-load { color: var(--vscode-charts-blue); }
    .op-store { color: var(--vscode-charts-green); }
    .op-arith { color: var(--vscode-charts-orange); }
    .op-ctrl { color: var(--vscode-charts-red); }
    .op-unknown { color: var(--vscode-errorForeground); }
    .comment { color: var(--vscode-descriptionForeground); font-style: italic; }
    .hash {
      font-family: var(--vscode-editor-font-family);
      font-size: 0.85em;
    }
    .func-meta {
      display: grid;
      grid-template-columns: auto auto;
      gap: 2px 16px;
      max-width: 400px;
      margin-bottom: 8px;
    }
    .func-meta dt { color: var(--vscode-descriptionForeground); }
    .func-meta dd { margin: 0; }
  </style>
</head>
<body>
  <h1>IPLC Bytecode Viewer</h1>
  ${headerHtml}
  ${constantsHtml}
  ${functionsHtml}
</body>
</html>`;
  }

  private renderHeader(header: DisassemblyHeader): string {
    if (!header) { return ''; }

    const flagsList = [];
    if (header.flags?.hasContentSignature) { flagsList.push('Content Signature'); }
    if (header.flags?.hasDebugSection) { flagsList.push('Debug Section'); }
    if (header.flags?.hasTypeSection) { flagsList.push('Type Section'); }
    const flagsStr = flagsList.length > 0 ? flagsList.join(', ') : 'None';

    return `
<details open>
  <summary>File Header</summary>
  <table>
    <tr><th>Field</th><th>Value</th></tr>
    <tr><td>Format Version</td><td>${header.formatVersion}</td></tr>
    <tr><td>Flags</td><td>${escapeHtml(flagsStr)}</td></tr>
    <tr><td>Entry Function ID</td><td>${header.entryFunctionId}</td></tr>
    <tr><td>Functions</td><td>${header.numFunctions}</td></tr>
    <tr><td>Variables</td><td>${header.numVariables}</td></tr>
    <tr><td>Max Stack Depth</td><td>${header.maxStackDepth}</td></tr>
    <tr><td>Max Call Depth</td><td>${header.maxCallDepth}</td></tr>
    <tr><td>FB Instances</td><td>${header.numFbInstances}</td></tr>
    <tr><td>FB Types</td><td>${header.numFbTypes}</td></tr>
    <tr><td>Arrays</td><td>${header.numArrays}</td></tr>
    <tr><td>Input Image</td><td>${header.inputImageBytes} bytes</td></tr>
    <tr><td>Output Image</td><td>${header.outputImageBytes} bytes</td></tr>
    <tr><td>Memory Image</td><td>${header.memoryImageBytes} bytes</td></tr>
    <tr><td>Content Hash</td><td class="hash">${escapeHtml(header.contentHash ?? '')}</td></tr>
    <tr><td>Source Hash</td><td class="hash">${escapeHtml(header.sourceHash ?? '')}</td></tr>
  </table>
</details>`;
  }

  private renderConstants(constants: DisassemblyConstant[]): string {
    if (!constants || constants.length === 0) {
      return '<details><summary>Constant Pool (empty)</summary></details>';
    }

    const rows = constants
      .map(c => `<tr><td class="mono">${c.index}</td><td>${escapeHtml(c.type)}</td><td class="mono">${escapeHtml(c.value)}</td></tr>`)
      .join('');

    return `
<details open>
  <summary>Constant Pool (${constants.length})</summary>
  <table>
    <tr><th>Index</th><th>Type</th><th>Value</th></tr>
    ${rows}
  </table>
</details>`;
  }

  private renderFunctions(functions: DisassemblyFunction[]): string {
    if (!functions || functions.length === 0) {
      return '<details><summary>Functions (none)</summary></details>';
    }

    return functions.map(func => {
      const instrRows = func.instructions.map(instr => {
        const opcodeClass = getOpcodeClass(instr.opcode);
        const commentHtml = instr.comment
          ? `<span class="comment">  ${escapeHtml(instr.comment)}</span>`
          : '';
        return `<tr>
          <td class="mono offset">${formatOffset(instr.offset)}</td>
          <td class="mono ${opcodeClass}">${escapeHtml(instr.opcode)}</td>
          <td class="mono">${escapeHtml(instr.operands)}${commentHtml}</td>
        </tr>`;
      }).join('');

      return `
<details open>
  <summary>Function ${func.id}</summary>
  <dl class="func-meta">
    <dt>Max Stack Depth</dt><dd>${func.maxStackDepth}</dd>
    <dt>Locals</dt><dd>${func.numLocals}</dd>
    <dt>Bytecode</dt><dd>${func.bytecodeLength} bytes</dd>
  </dl>
  <table>
    <tr><th>Offset</th><th>Opcode</th><th>Operands</th></tr>
    ${instrRows}
  </table>
</details>`;
    }).join('');
  }
}

// --- Types for the disassembly JSON response ---

interface DisassemblyResult {
  error?: string;
  header: DisassemblyHeader;
  constants: DisassemblyConstant[];
  functions: DisassemblyFunction[];
}

interface DisassemblyHeader {
  formatVersion: number;
  flags: { hasContentSignature: boolean; hasDebugSection: boolean; hasTypeSection: boolean };
  maxStackDepth: number;
  maxCallDepth: number;
  numVariables: number;
  numFbInstances: number;
  numFunctions: number;
  numFbTypes: number;
  numArrays: number;
  entryFunctionId: number;
  inputImageBytes: number;
  outputImageBytes: number;
  memoryImageBytes: number;
  contentHash: string;
  sourceHash: string;
}

interface DisassemblyConstant {
  index: number;
  type: string;
  value: string;
}

interface DisassemblyFunction {
  id: number;
  maxStackDepth: number;
  numLocals: number;
  bytecodeLength: number;
  instructions: DisassemblyInstruction[];
}

interface DisassemblyInstruction {
  offset: number;
  opcode: string;
  operands: string;
  comment: string;
}

// --- Utility functions ---

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function formatOffset(offset: number): string {
  return '0x' + offset.toString(16).toUpperCase().padStart(4, '0');
}

function getOpcodeClass(opcode: string): string {
  if (opcode.startsWith('LOAD')) { return 'op-load'; }
  if (opcode.startsWith('STORE')) { return 'op-store'; }
  if (opcode.startsWith('ADD') || opcode.startsWith('SUB') ||
      opcode.startsWith('MUL') || opcode.startsWith('DIV')) { return 'op-arith'; }
  if (opcode.startsWith('RET') || opcode.startsWith('CALL') ||
      opcode.startsWith('JMP') || opcode.startsWith('BR')) { return 'op-ctrl'; }
  if (opcode.startsWith('UNKNOWN')) { return 'op-unknown'; }
  return '';
}
