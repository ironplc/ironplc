import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { execSync } from 'child_process';
import { downloadAndUnzipVSCode } from '@vscode/test-electron';
import {
  captureSyntaxHighlighting,
  captureDiagnostics,
  captureDiagnosticsWithProblems,
  captureRunProgramCodeLens,
  captureRunningProgram,
  captureSettings,
  captureBytecodeViewer,
  captureMcpServers,
} from './captureScreenshots';

function augmentedEnv(): NodeJS.ProcessEnv {
  // Augment PATH with common install locations that may not be inherited when
  // node is launched via `just` (e.g. ~/.cargo/bin for Rust-installed binaries).
  const extraPaths = [
    path.join(os.homedir(), '.cargo', 'bin'),
    '/usr/local/bin',
    '/opt/homebrew/bin',
  ];
  const augmentedPath = [...extraPaths, process.env['PATH'] ?? ''].join(path.delimiter);
  return { ...process.env, PATH: augmentedPath };
}

function hasIronplcc(): boolean {
  try {
    execSync('ironplcc version', { stdio: 'ignore', env: augmentedEnv() });
    return true;
  }
  catch {
    return false;
  }
}

function hasIronplcmcp(): boolean {
  // `ironplcmcp` speaks MCP over stdio and has no --version flag, so probe for
  // existence using platform-appropriate PATH lookup commands.
  const env = augmentedEnv();
  const cmd = process.platform === 'win32' ? 'where ironplcmcp' : 'command -v ironplcmcp';
  try {
    execSync(cmd, { stdio: 'ignore', env });
    return true;
  }
  catch {
    return false;
  }
}

async function main(): Promise<void> {
  const editorOutputDir = path.resolve(__dirname, '../../docs/reference/editor/images');
  const aiAgentsOutputDir = path.resolve(__dirname, '../../docs/how-to-guides/ai-agents/images');
  fs.mkdirSync(editorOutputDir, { recursive: true });
  fs.mkdirSync(aiAgentsOutputDir, { recursive: true });

  const extensionPath = path.resolve(__dirname, '../../');
  const vscodePath = await downloadAndUnzipVSCode('stable');
  console.log(`VS Code path: ${vscodePath}`);

  const userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'ironplc-screenshots-'));
  const userSettingsDir = path.join(userDataDir, 'User');
  fs.mkdirSync(userSettingsDir, { recursive: true });
  fs.copyFileSync(
    path.resolve(__dirname, 'settings.json'),
    path.join(userSettingsDir, 'settings.json'),
  );

  const opts = { vscodePath, extensionPath, userDataDir };
  const ironplccAvailable = hasIronplcc();
  const ironplcmcpAvailable = hasIronplcmcp();

  try {
    console.log('\n--- Syntax Highlighting ---');
    await captureSyntaxHighlighting(opts, path.join(editorOutputDir, 'syntax-highlighting.png'));

    if (ironplccAvailable) {
      console.log('\n--- Diagnostics ---');
      await captureDiagnostics(opts, path.join(editorOutputDir, 'diagnostics-squiggles.png'));

      console.log('\n--- Diagnostics with Problems Panel ---');
      await captureDiagnosticsWithProblems(opts, path.join(editorOutputDir, 'diagnostics-problems.png'));

      console.log('\n--- Run Program Code Lens ---');
      await captureRunProgramCodeLens(opts, path.join(editorOutputDir, 'run-program-code-lens.png'));

      console.log('\n--- Running Program ---');
      await captureRunningProgram(opts, path.join(editorOutputDir, 'run-program-running.png'));
    }
    else {
      console.log('\n--- Diagnostics: SKIPPED (ironplcc not found on PATH) ---');
      console.log('\n--- Diagnostics with Problems Panel: SKIPPED (ironplcc not found on PATH) ---');
      console.log('\n--- Run Program Code Lens: SKIPPED (ironplcc not found on PATH) ---');
      console.log('\n--- Running Program: SKIPPED (ironplcc not found on PATH) ---');
    }

    console.log('\n--- Settings Panel ---');
    await captureSettings(opts, path.join(editorOutputDir, 'settings-panel.png'));

    if (ironplcmcpAvailable) {
      console.log('\n--- MCP Servers View ---');
      await captureMcpServers(opts, path.join(aiAgentsOutputDir, 'mcp-servers-view.png'));
    }
    else {
      console.log('\n--- MCP Servers View: SKIPPED (ironplcmcp not found on PATH) ---');
    }

    const iplcFixture = path.resolve(__dirname, 'fixtures/sample.iplc');
    if (fs.existsSync(iplcFixture)) {
      console.log('\n--- Bytecode Viewer ---');
      await captureBytecodeViewer(opts, path.join(editorOutputDir, 'bytecode-viewer.png'), iplcFixture);
    }
    else {
      console.log('\n--- Bytecode Viewer: SKIPPED (no sample.iplc fixture) ---');
    }

    console.log('\nDone. Screenshots written to:', editorOutputDir);
  }
  finally {
    fs.rmSync(userDataDir, { recursive: true, force: true });
  }
}

main().catch((err) => {
  console.error('Screenshot capture failed:', err);
  process.exit(1);
});
