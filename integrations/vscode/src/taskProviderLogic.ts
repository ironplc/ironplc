import * as path from 'path';

/**
 * Builds the arguments for the ironplcc compile command.
 */
export function buildCompileArgs(workspaceFolderPath: string, outputFileName: string): { args: string[]; cwd: string } {
  const outputPath = path.join(workspaceFolderPath, outputFileName);
  return {
    args: ['compile', '.', '-o', outputPath],
    cwd: workspaceFolderPath,
  };
}

/**
 * Derives the output file name from a workspace folder name.
 */
export function outputFileNameForFolder(folderName: string): string {
  return `${folderName}.iplc`;
}
