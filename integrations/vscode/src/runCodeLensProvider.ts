/**
 * CodeLens provider that shows run/pause/stop actions above PROGRAM
 * declarations in IEC 61131-3 Structured Text files.
 */

import { RunState } from './runSession';

/** Minimal subset of vscode types needed for code lens, enabling unit tests
 *  without depending on the vscode module. */
export interface CodeLensLike {
  range: { start: { line: number; character: number }; end: { line: number; character: number } };
  command?: {
    title: string;
    command: string;
    arguments?: unknown[];
  };
}

/** Regex that matches a PROGRAM declaration line. */
const PROGRAM_RE = /^\s*PROGRAM\s+(\w+)/i;

/**
 * Find PROGRAM declarations in the given source text and return code lens
 * descriptors for each one, reflecting the current run session state.
 *
 * When the session is `idle` or `error`, each PROGRAM line gets a single
 * "Run Program" lens. When `running`, the line gets "Pause" and "Stop"
 * lenses. When `paused`, the line gets "Resume" and "Stop" lenses. Pause and
 * stop lenses take no arguments; the run lens carries the program name so
 * it can be surfaced to the run command.
 */
export function findProgramLenses(
  text: string,
  state: RunState = 'idle',
  hasCompiler: boolean = true,
): CodeLensLike[] {
  const lines = text.split('\n');
  const lenses: CodeLensLike[] = [];

  for (let i = 0; i < lines.length; i++) {
    const match = PROGRAM_RE.exec(lines[i]);
    if (!match) {
      continue;
    }
    const programName = match[1];
    const range = {
      start: { line: i, character: 0 },
      end: { line: i, character: lines[i].length },
    };

    if (state === 'running') {
      lenses.push({
        range,
        command: {
          title: '$(debug-pause) Pause',
          command: 'ironplc.pauseProgram',
        },
      });
      lenses.push({
        range,
        command: {
          title: '$(debug-stop) Stop',
          command: 'ironplc.stopProgram',
        },
      });
    } else if (state === 'paused') {
      lenses.push({
        range,
        command: {
          title: '$(debug-continue) Resume',
          command: 'ironplc.pauseProgram',
        },
      });
      lenses.push({
        range,
        command: {
          title: '$(debug-stop) Stop',
          command: 'ironplc.stopProgram',
        },
      });
    } else if (!hasCompiler) {
      lenses.push({
        range,
        command: {
          title: '$(warning) Run Program (no compiler)',
          command: 'ironplc.runProgram',
        },
      });
    } else {
      lenses.push({
        range,
        command: {
          title: '$(play) Run Program',
          command: 'ironplc.runProgram',
          arguments: [programName],
        },
      });
    }
  }

  return lenses;
}
