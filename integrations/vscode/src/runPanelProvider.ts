import * as vscode from 'vscode';
import { LanguageClientLike } from './iplcEditorLogic';
import { getRunPanelHtml } from './runPanelRendering';

interface RunResult {
  ok: boolean;
  variables: Array<{ index: number; name: string; type_name: string; value: string }>;
  total_scans: number;
  error?: string;
}

const STEP_INTERVAL_MS = 100;

export class RunPanelProvider {
  private panel: vscode.WebviewPanel | undefined;
  private stepTimer: ReturnType<typeof setInterval> | undefined;
  private isRunning = false;

  constructor(private readonly client: LanguageClientLike) {}

  /** Start running the project and open the output panel. */
  async start(context: vscode.ExtensionContext): Promise<void> {
    // Create or reveal the panel
    if (this.panel) {
      this.panel.reveal(vscode.ViewColumn.Beside);
    }
    else {
      this.panel = vscode.window.createWebviewPanel(
        'ironplc.runPanel',
        'IronPLC: Run Output',
        vscode.ViewColumn.Beside,
        { enableScripts: true, retainContextWhenHidden: true },
      );
      this.panel.webview.html = getRunPanelHtml();
      this.panel.onDidDispose(() => {
        this.stop();
        this.panel = undefined;
      }, null, context.subscriptions);

      // Handle messages from the webview (pause/stop buttons)
      this.panel.webview.onDidReceiveMessage(
        (message: { command: string }) => {
          switch (message.command) {
          case 'stop':
            this.stop();
            break;
          case 'pause':
            this.pause();
            break;
          case 'resume':
            this.resume();
            break;
          }
        },
        undefined,
        context.subscriptions,
      );
    }

    // Send ironplc/run to compile and load the project
    const result = await this.client.sendRequest('ironplc/run', {
      cycleTimeUs: 100000,
    }) as RunResult;

    if (!result.ok) {
      this.postMessage({
        type: 'error',
        message: result.error || 'Failed to compile program',
      });
      return;
    }

    this.postMessage({ type: 'started' });
    this.isRunning = true;
    this.startStepLoop();
  }

  /** Stop execution and release resources. */
  async stop(): Promise<void> {
    this.clearStepTimer();
    this.isRunning = false;

    try {
      await this.client.sendRequest('ironplc/stop', {});
    }
    catch {
      // Ignore errors during stop
    }

    this.postMessage({ type: 'stopped' });
  }

  private pause(): void {
    this.clearStepTimer();
    this.isRunning = false;
    this.postMessage({ type: 'paused' });
  }

  private resume(): void {
    this.isRunning = true;
    this.startStepLoop();
    this.postMessage({ type: 'resumed' });
  }

  private startStepLoop(): void {
    this.clearStepTimer();
    this.stepTimer = setInterval(async () => {
      if (!this.isRunning) {
        return;
      }
      try {
        const result = await this.client.sendRequest('ironplc/step', {
          scans: 1,
        }) as RunResult;
        this.postMessage({ type: 'variables', data: result });
      }
      catch {
        this.stop();
      }
    }, STEP_INTERVAL_MS);
  }

  private clearStepTimer(): void {
    if (this.stepTimer) {
      clearInterval(this.stepTimer);
      this.stepTimer = undefined;
    }
  }

  private postMessage(message: unknown): void {
    this.panel?.webview.postMessage(message);
  }

  dispose(): void {
    this.clearStepTimer();
    this.panel?.dispose();
  }
}
