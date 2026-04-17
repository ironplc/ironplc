import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { execSync } from 'child_process';
import { downloadAndUnzipVSCode } from '@vscode/test-electron';
import {
  captureSyntaxHighlighting,
  captureDiagnostics,
  captureSettings,
  captureBytecodeViewer,
} from './captureScreenshots';

function hasIronplcc(): boolean {
  try {
    execSync('ironplcc --version', { stdio: 'ignore' });
    return true;
  }
  catch {
    return false;
  }
}

async function main(): Promise<void> {
  const editorOutputDir = path.resolve(__dirname, '../../docs/reference/editor/images');
  fs.mkdirSync(editorOutputDir, { recursive: true });

  const extensionPath = path.resolve(__dirname, '../');
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

  try {
    console.log('\n--- Syntax Highlighting ---');
    await captureSyntaxHighlighting(opts, path.join(editorOutputDir, 'syntax-highlighting.png'));

    if (ironplccAvailable) {
      console.log('\n--- Diagnostics ---');
      await captureDiagnostics(opts, path.join(editorOutputDir, 'diagnostics-squiggles.png'));
    }
    else {
      console.log('\n--- Diagnostics: SKIPPED (ironplcc not found on PATH) ---');
    }

    console.log('\n--- Settings Panel ---');
    await captureSettings(opts, path.join(editorOutputDir, 'settings-panel.png'));

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
