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
  captureMcpConfig,
  captureQuickstartHelloworld,
  captureQuickstartRunOutput,
  captureQuickstartTimerOutput,
  captureQuickstartAnimation,
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
  // eslint-disable-next-line @typescript-eslint/naming-convention
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
  const outputDir = path.resolve(__dirname, '../screenshots/output');
  fs.mkdirSync(outputDir, { recursive: true });

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
    await captureSyntaxHighlighting(opts, path.join(outputDir, 'syntax-highlighting.png'));

    if (ironplccAvailable) {
      console.log('\n--- Diagnostics ---');
      await captureDiagnostics(opts, path.join(outputDir, 'diagnostics-squiggles.png'));

      console.log('\n--- Diagnostics with Problems Panel ---');
      await captureDiagnosticsWithProblems(opts, path.join(outputDir, 'diagnostics-problems.png'));

      console.log('\n--- Run Program Code Lens ---');
      await captureRunProgramCodeLens(opts, path.join(outputDir, 'run-program-code-lens.png'));

      console.log('\n--- Running Program ---');
      await captureRunningProgram(opts, path.join(outputDir, 'run-program-running.png'));
    }
    else {
      console.log('\n--- Diagnostics: SKIPPED (ironplcc not found on PATH) ---');
      console.log('\n--- Diagnostics with Problems Panel: SKIPPED (ironplcc not found on PATH) ---');
      console.log('\n--- Run Program Code Lens: SKIPPED (ironplcc not found on PATH) ---');
      console.log('\n--- Running Program: SKIPPED (ironplcc not found on PATH) ---');
    }

    console.log('\n--- Settings Panel ---');
    await captureSettings(opts, path.join(outputDir, 'settings-panel.png'));

    console.log('\n--- MCP Config ---');
    await captureMcpConfig(opts, path.join(outputDir, 'mcp-config.png'));

    if (ironplccAvailable) {
      console.log('\n--- Quickstart: Hello World ---');
      await captureQuickstartHelloworld(opts, path.join(outputDir, 'quickstart-helloworld.png'));

      console.log('\n--- Quickstart: Run Output ---');
      await captureQuickstartRunOutput(opts, path.join(outputDir, 'quickstart-run-output.png'));

      console.log('\n--- Quickstart: Timer Output ---');
      await captureQuickstartTimerOutput(opts, path.join(outputDir, 'quickstart-timer-output.png'));

      console.log('\n--- Quickstart: Animation ---');
      await captureQuickstartAnimation(opts, path.join(outputDir, 'quickstart-animation.png'));
    }
    else {
      console.log('\n--- Quickstart screenshots: SKIPPED (ironplcc not found on PATH) ---');
    }

    const iplcFixture = path.resolve(__dirname, 'fixtures/sample.iplc');
    if (fs.existsSync(iplcFixture)) {
      console.log('\n--- Bytecode Viewer ---');
      await captureBytecodeViewer(opts, path.join(outputDir, 'bytecode-viewer.png'), iplcFixture);
    }
    else {
      console.log('\n--- Bytecode Viewer: SKIPPED (no sample.iplc fixture) ---');
    }

    console.log('\nDone. Screenshots written to:', outputDir);
  }
  finally {
    fs.rmSync(userDataDir, { recursive: true, force: true });
  }
}

main().catch((err) => {
  console.error('Screenshot capture failed:', err);
  process.exit(1);
});
