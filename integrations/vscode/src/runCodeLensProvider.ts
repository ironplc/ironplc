/**
 * CodeLens provider that shows a "Run Program" action above PROGRAM
 * declarations in IEC 61131-3 Structured Text files.
 */

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
 * descriptors for each one.
 */
export function findProgramLenses(text: string): CodeLensLike[] {
  const lines = text.split('\n');
  const lenses: CodeLensLike[] = [];

  for (let i = 0; i < lines.length; i++) {
    const match = PROGRAM_RE.exec(lines[i]);
    if (match) {
      const programName = match[1];
      lenses.push({
        range: {
          start: { line: i, character: 0 },
          end: { line: i, character: lines[i].length },
        },
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
