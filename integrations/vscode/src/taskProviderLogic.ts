import * as path from 'path';

/**
 * Builds the `ironplcc compile` argument vector for compiling `input` to the
 * container at `output`. Shared by the build task (which compiles the whole
 * project via `.`) and the debug adapter (which compiles a single source file).
 */
export function compileArgs(input: string, output: string): string[] {
  return ['compile', input, '-o', output];
}

/**
 * Builds the arguments for the ironplcc compile command.
 */
export function buildCompileArgs(workspaceFolderPath: string, outputFileName: string): { args: string[]; cwd: string } {
  const outputPath = path.join(workspaceFolderPath, outputFileName);
  return {
    args: compileArgs('.', outputPath),
    cwd: workspaceFolderPath,
  };
}

/**
 * Derives the output file name from a workspace folder name.
 */
export function outputFileNameForFolder(folderName: string): string {
  return `${folderName}.iplc`;
}
