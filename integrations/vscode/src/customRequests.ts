import * as vscode from 'vscode';
import { IRONPLC_DEBUG_TYPE } from './debugAdapter';
import { customRequestFailedMessage } from './debugAdapterLogic';

/**
 * Registers the scan-cycle custom-request command, forwarding IronPLC's
 * `ironplc/stepScan` request to the active debug session; without it the
 * request is unreachable from the UI (see
 * `specs/design/debugger-support.md` §"Phase 5"). The debug toolbar button that
 * invokes it is contributed in `package.json`.
 *
 * The scan *count* is deliberately not a command. It is a value you watch while
 * stepping, not one you ask for: the server publishes it in the `Runtime`
 * scope, which the client re-reads at every stop, so it is simply on screen in
 * the Variables panel. A button that popped it in a notification was the wrong
 * shape for it — see §Scopes in the design doc.
 */
export function registerCustomRequests(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand('ironplc.stepScan', async () => {
      const session = activeIronplcSession();
      if (!session) {
        return;
      }
      try {
        await session.customRequest('ironplc/stepScan');
      } catch {
        // Server-side `ironplc/stepScan` is not implemented yet, so the request
        // is refused and the promise rejects. Report it rather than letting the
        // rejection escape as an unhandled error.
        void vscode.window.showWarningMessage(customRequestFailedMessage('Step Scan Cycle'));
      }
    }),
  );
}

/**
 * The active debug session when it belongs to the IronPLC debugger, else
 * `undefined`. Custom requests are only meaningful against an IronPLC session,
 * so a command invoked with no (or a foreign) session no-ops.
 */
function activeIronplcSession(): vscode.DebugSession | undefined {
  const session = vscode.debug.activeDebugSession;
  if (!session || session.type !== IRONPLC_DEBUG_TYPE) {
    return undefined;
  }
  return session;
}
