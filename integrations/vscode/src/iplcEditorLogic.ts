import { DisassemblyResult, getErrorHtml, getDisassemblyHtml } from './iplcRendering';

// Numeric values match vscode-languageclient's State enum.
export const STATE_RUNNING = 2;
export const STATE_STOPPED = 1;

/** Minimal subset of LanguageClient used by the editor provider, enabling
 *  unit tests to supply a mock without depending on vscode-languageclient. */
export interface LanguageClientLike {
  isRunning(): boolean;
  sendRequest(method: string, params: unknown): Promise<unknown>;
  onDidChangeState(listener: (e: { newState: number }) => void): { dispose(): void };
}

/** Wait for the language client to reach the Running state.
 *  Resolves `true` if the client starts, `false` if it stops or the
 *  timeout expires. Returns immediately if the client is already running. */
export function waitForClient(client: LanguageClientLike, timeoutMs: number): Promise<boolean> {
  if (client.isRunning()) {
    return Promise.resolve(true);
  }
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      disposable.dispose();
      resolve(false);
    }, timeoutMs);

    const disposable = client.onDidChangeState((e) => {
      if (e.newState === STATE_RUNNING) {
        clearTimeout(timer);
        disposable.dispose();
        resolve(true);
      }
      else if (e.newState === STATE_STOPPED) {
        clearTimeout(timer);
        disposable.dispose();
        resolve(false);
      }
    });
  });
}

/** Produce the HTML content for an .iplc custom editor.
 *  Waits for the client if it is not yet running, sends a disassemble
 *  request, and returns rendered HTML (or an error page on failure). */
export async function resolveEditorContent(
  client: LanguageClientLike,
  documentUri: string,
  timeoutMs: number = 5000,
): Promise<string> {
  if (!client.isRunning()) {
    const ready = await waitForClient(client, timeoutMs);
    if (!ready) {
      return getErrorHtml(
        'E0002 - IronPLC compiler not found. Install the compiler to view .iplc files.',
      );
    }
  }

  try {
    const result = await client.sendRequest('ironplc/disassemble', {
      uri: documentUri,
    });
    return getDisassemblyHtml(result as DisassemblyResult);
  }
  catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return getErrorHtml(`E0003 - Failed to disassemble .iplc file: ${message}`);
  }
}
