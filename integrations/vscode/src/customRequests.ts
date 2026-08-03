import * as vscode from 'vscode';
import { IRONPLC_DEBUG_TYPE } from './debugAdapter';
import { ScanCountResponse, scanCountMessage } from './debugAdapterLogic';

/**
 * Registers the scan-cycle custom-request commands. These forward IronPLC's
 * custom DAP requests (`ironplc/stepScan`, `ironplc/scanCount`) to the active
 * debug session; without them the requests are unreachable from the UI (see
 * `specs/design/debugger-support.md` §"Phase 5"). The debug toolbar buttons
 * that invoke these commands are contributed in `package.json`.
 */
export function registerCustomRequests(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand('ironplc.stepScan', async () => {
      const session = activeIronplcSession();
      if (!session) {
        return;
      }
      await session.customRequest('ironplc/stepScan');
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('ironplc.scanCount', async () => {
      const session = activeIronplcSession();
      if (!session) {
        return;
      }
      const response = (await session.customRequest('ironplc/scanCount')) as ScanCountResponse | undefined;
      void vscode.window.showInformationMessage(scanCountMessage(response));
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
