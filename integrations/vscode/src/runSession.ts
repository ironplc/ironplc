import { LanguageClientLike } from './iplcEditorLogic';

/** Result returned by the LSP for run/step/stop requests. */
export interface RunResult {
  ok: boolean;
  variables: VariableInfo[];
  total_scans: number;
  error?: string;
}

export interface VariableInfo {
  index: number;
  value: string;
  name: string;
  type_name: string;
}

export type RunState = 'idle' | 'running' | 'paused' | 'error';

/** Callback invoked when the run state or variable values change. */
export interface RunSessionCallbacks {
  onStateChange(state: RunState): void;
  onVariablesUpdate(variables: VariableInfo[], totalScans: number): void;
  onError(message: string): void;
}

/** Default cycle time in microseconds (100 ms). */
const DEFAULT_CYCLE_TIME_US = 100_000;

/** Interval between step requests in milliseconds. */
const STEP_INTERVAL_MS = 100;

/** Interval between UI updates in milliseconds. */
const RENDER_INTERVAL_MS = 500;

/**
 * Manages the lifecycle of a program execution session.
 *
 * Communicates with the LSP server via ironplc/run, ironplc/step,
 * and ironplc/stop custom requests.
 */
export class RunSession {
  private state: RunState = 'idle';
  private stepTimer: ReturnType<typeof setInterval> | undefined;
  private renderTimer: ReturnType<typeof setInterval> | undefined;
  private latestVariables: VariableInfo[] = [];
  private latestTotalScans = 0;

  constructor(
    private readonly client: LanguageClientLike,
    private readonly callbacks: RunSessionCallbacks,
  ) {}

  getState(): RunState {
    return this.state;
  }

  /** Start running the program from source text. */
  async start(source: string, cycleTimeUs?: number): Promise<void> {
    if (this.state === 'running' || this.state === 'paused') {
      await this.stop();
    }

    const result = await this.client.sendRequest('ironplc/run', {
      source,
      cycleTimeUs: cycleTimeUs ?? DEFAULT_CYCLE_TIME_US,
    }) as RunResult;

    if (!result.ok) {
      this.setState('error');
      this.callbacks.onError(result.error ?? 'Failed to start program.');
      return;
    }

    this.setState('running');
    this.startTimers();
  }

  /** Pause execution (stops stepping but keeps the session alive). */
  pause(): void {
    if (this.state !== 'running') {
      return;
    }
    this.clearTimers();
    this.setState('paused');
  }

  /** Resume execution after pause. */
  resume(): void {
    if (this.state !== 'paused') {
      return;
    }
    this.setState('running');
    this.startTimers();
  }

  /** Stop execution and tear down the session. */
  async stop(): Promise<void> {
    this.clearTimers();
    if (this.state !== 'idle') {
      await this.client.sendRequest('ironplc/stop', {});
    }
    this.latestVariables = [];
    this.latestTotalScans = 0;
    this.setState('idle');
  }

  /** Clean up resources. Call when the session is no longer needed. */
  dispose(): void {
    this.clearTimers();
  }

  private setState(state: RunState): void {
    this.state = state;
    this.callbacks.onStateChange(state);
  }

  private startTimers(): void {
    this.stepTimer = setInterval(() => this.doStep(), STEP_INTERVAL_MS);
    this.renderTimer = setInterval(() => this.doRender(), RENDER_INTERVAL_MS);
  }

  private clearTimers(): void {
    if (this.stepTimer !== undefined) {
      clearInterval(this.stepTimer);
      this.stepTimer = undefined;
    }
    if (this.renderTimer !== undefined) {
      clearInterval(this.renderTimer);
      this.renderTimer = undefined;
    }
  }

  private async doStep(): Promise<void> {
    const result = await this.client.sendRequest('ironplc/step', {
      scans: 1,
    }) as RunResult;

    if (!result.ok) {
      this.clearTimers();
      this.setState('error');
      this.callbacks.onError(result.error ?? 'Runtime error.');
      this.latestVariables = result.variables;
      this.latestTotalScans = result.total_scans;
      this.doRender();
      return;
    }

    this.latestVariables = result.variables;
    this.latestTotalScans = result.total_scans;
  }

  private doRender(): void {
    this.callbacks.onVariablesUpdate(this.latestVariables, this.latestTotalScans);
  }
}
