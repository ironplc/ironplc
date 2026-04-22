import * as path from 'path';
import * as fs from 'fs';
import * as UPNG from 'upng-js';
import { _electron, ElectronApplication, Page } from 'playwright';

const WINDOW_WIDTH = 1200;
const WINDOW_HEIGHT = 800;

// Source files live under src/, not out/, so resolve relative to the workspace root.
const VALID_ST = path.resolve(__dirname, '../../src/test/functional/resources/valid.st');
const INVALID_ST = path.resolve(__dirname, '../../src/screenshots/fixtures/invalid.st');
const MCP_CONFIG = path.resolve(__dirname, '../../src/screenshots/fixtures/mcp-workspace/.vscode/mcp.json');
const QUICKSTART_HELLOWORLD_ST = path.resolve(__dirname, '../../src/screenshots/fixtures/quickstart-helloworld.st');
const QUICKSTART_TIMER_ST = path.resolve(__dirname, '../../src/screenshots/fixtures/quickstart-timer.st');

interface LaunchOptions {
  vscodePath: string;
  extensionPath: string;
  userDataDir: string;
  filePath: string;
}

async function launchVSCode(opts: LaunchOptions): Promise<ElectronApplication> {
  const args = [
    `--extensionDevelopmentPath=${opts.extensionPath}`,
    `--user-data-dir=${opts.userDataDir}`,
    '--disable-extensions',
    '--locale=en',
    '--disable-gpu',
    '--skip-welcome',
    '--skip-release-notes',
    '--disable-workspace-trust',
    opts.filePath,
  ];
  const electronEnv: Record<string, string> = { ...process.env as Record<string, string> };
  electronEnv['ELECTRON_DISABLE_SANDBOX'] = '1';
  const app = await _electron.launch({
    executablePath: opts.vscodePath,
    args,
    env: electronEnv,
  });
  return app;
}

async function waitForEditor(page: Page): Promise<void> {
  await page.waitForSelector('.monaco-editor', { timeout: 30000 });
  await page.waitForTimeout(2000);
}

async function dismissNotifications(page: Page): Promise<void> {
  // The "extensions temporarily disabled" notification appears after the editor loads.
  // Wait briefly for it to appear, then dismiss all toasts.
  await page.waitForTimeout(2000);
  const toasts = page.locator('.notifications-toasts .notification-toast');
  const count = await toasts.count();
  for (let i = count - 1; i >= 0; i--) {
    const toast = toasts.nth(i);
    if (await toast.isVisible()) {
      const closeBtn = toast.locator('a.action-label[title="Close Notification"], .codicon-notifications-clear, .codicon-close');
      const closeBtnCount = await closeBtn.count();
      if (closeBtnCount > 0 && await closeBtn.first().isVisible()) {
        await closeBtn.first().click();
        await page.waitForTimeout(200);
      }
    }
  }
}

async function hideSideBar(page: Page): Promise<void> {
  const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
  // Hide the primary side bar only if it is visible (toggle would reopen it).
  const primarySideBar = page.locator('.part.sidebar');
  if (await primarySideBar.isVisible()) {
    await page.keyboard.press(`${modifier}+b`);
    await page.waitForTimeout(500);
  }
  // Hide the secondary side bar (AI chat panels live here) only if it is visible.
  const secondarySideBar = page.locator('.part.auxiliarybar');
  if (await secondarySideBar.isVisible()) {
    await page.keyboard.press(`${modifier}+Alt+b`);
    await page.waitForTimeout(500);
  }
}

async function setWindowSize(page: Page): Promise<void> {
  await page.setViewportSize({ width: WINDOW_WIDTH, height: WINDOW_HEIGHT });
}

export async function captureSyntaxHighlighting(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  const app = await launchVSCode({ ...opts, filePath: VALID_ST });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
    await dismissNotifications(page);
    await hideSideBar(page);
    await page.waitForTimeout(1000);
    await page.screenshot({ path: outputPath, type: 'png' });
    console.log(`Captured: ${outputPath}`);
  }
  finally {
    await app.close();
  }
}

export async function captureDiagnostics(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  const app = await launchVSCode({ ...opts, filePath: INVALID_ST });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
    await dismissNotifications(page);
    await hideSideBar(page);
    try {
      await page.waitForSelector('.squiggly-error, .squiggly-warning', { timeout: 15000 });
    }
    catch {
      console.warn('Warning: diagnostic squiggles did not appear within timeout');
    }
    await page.waitForTimeout(1000);
    await page.screenshot({ path: outputPath, type: 'png' });
    console.log(`Captured: ${outputPath}`);
  }
  finally {
    await app.close();
  }
}

export async function captureDiagnosticsWithProblems(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  const app = await launchVSCode({ ...opts, filePath: INVALID_ST });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
    await dismissNotifications(page);
    await hideSideBar(page);
    try {
      await page.waitForSelector('.squiggly-error, .squiggly-warning', { timeout: 15000 });
    }
    catch {
      console.warn('Warning: diagnostic squiggles did not appear within timeout');
    }
    // Open the Problems panel via keyboard shortcut
    const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
    await page.keyboard.press(`${modifier}+Shift+m`);
    try {
      // Wait for at least one problem entry to appear in the panel
      await page.waitForSelector('.markers-panel .monaco-list-row', { timeout: 10000 });
    }
    catch {
      console.warn('Warning: problem entries did not appear in the Problems panel within timeout');
    }
    await page.waitForTimeout(1000);
    await page.screenshot({ path: outputPath, type: 'png' });
    console.log(`Captured: ${outputPath}`);
  }
  finally {
    await app.close();
  }
}

export async function captureRunProgramCodeLens(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  const app = await launchVSCode({ ...opts, filePath: VALID_ST });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
    await dismissNotifications(page);
    await hideSideBar(page);
    try {
      // Wait for the "Run Program" code lens to appear above the PROGRAM declaration.
      await page.waitForSelector('.codelens-decoration a', { timeout: 15000 });
    }
    catch {
      console.warn('Warning: Run Program code lens did not appear within timeout');
    }
    await page.waitForTimeout(1000);
    await page.screenshot({ path: outputPath, type: 'png' });
    console.log(`Captured: ${outputPath}`);
  }
  finally {
    await app.close();
  }
}

export async function captureRunningProgram(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  const app = await launchVSCode({ ...opts, filePath: VALID_ST });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
    await dismissNotifications(page);
    await hideSideBar(page);
    try {
      // Wait for the "Run Program" code lens, then click it to start execution.
      const runLens = page.locator('.codelens-decoration a', { hasText: 'Run Program' }).first();
      await runLens.waitFor({ timeout: 15000 });
      await runLens.click();
    }
    catch {
      console.warn('Warning: Run Program code lens did not appear or could not be clicked');
    }
    try {
      // Wait for the Stop status bar item — signals the program is running.
      await page.waitForSelector('.statusbar-item[id*="stopProgram"], .statusbar-item', {
        timeout: 10000,
      });
      // Also wait for the Pause/Stop code lenses to replace "Run Program".
      await page.locator('.codelens-decoration a', { hasText: 'Stop' }).first().waitFor({ timeout: 10000 });
    }
    catch {
      console.warn('Warning: running state indicators did not appear within timeout');
    }
    await page.waitForTimeout(1000);
    await page.screenshot({ path: outputPath, type: 'png' });
    console.log(`Captured: ${outputPath}`);
  }
  finally {
    await app.close();
  }
}

export async function captureSettings(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  const app = await launchVSCode({ ...opts, filePath: VALID_ST });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
    await dismissNotifications(page);
    const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
    await page.keyboard.press(`${modifier}+,`);
    await page.waitForSelector('.settings-editor', { timeout: 10000 });
    await page.waitForTimeout(500);

    // The settings search bar is a hidden Monaco widget — type directly since
    // VS Code focuses it automatically when settings opens via the keyboard shortcut.
    await page.keyboard.type('ironplc');
    await page.waitForTimeout(2000);
    await page.screenshot({ path: outputPath, type: 'png' });
    console.log(`Captured: ${outputPath}`);
  }
  finally {
    await app.close();
  }
}

export async function captureMcpConfig(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  const app = await launchVSCode({ ...opts, filePath: MCP_CONFIG });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
    await dismissNotifications(page);
    await hideSideBar(page);
    await page.waitForTimeout(1000);
    await page.screenshot({ path: outputPath, type: 'png' });
    console.log(`Captured: ${outputPath}`);
  }
  finally {
    await app.close();
  }
}

export async function captureBytecodeViewer(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
  iplcFilePath: string,
): Promise<void> {
  const app = await launchVSCode({ ...opts, filePath: iplcFilePath });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await page.waitForTimeout(5000);
    await hideSideBar(page);
    await page.waitForTimeout(1000);
    await page.screenshot({ path: outputPath, type: 'png' });
    console.log(`Captured: ${outputPath}`);
  }
  finally {
    await app.close();
  }
}

async function clickRunProgramAndWaitForOutput(page: Page): Promise<void> {
  try {
    const runLens = page.locator('.codelens-decoration a', { hasText: 'Run Program' }).first();
    await runLens.waitFor({ timeout: 15000 });
    await runLens.click();
  }
  catch {
    console.warn('Warning: Run Program code lens did not appear or could not be clicked');
  }
  try {
    // Wait for the IronPLC Run output panel to show scan cycle output.
    await page.waitForSelector('.output-view-container .view-line', { timeout: 15000 });
  }
  catch {
    console.warn('Warning: IronPLC Run output did not appear within timeout');
  }
  // Allow a few render cycles so variable values stabilise.
  await page.waitForTimeout(1500);
}

export async function captureQuickstartHelloworld(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  const app = await launchVSCode({ ...opts, filePath: QUICKSTART_HELLOWORLD_ST });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
    await dismissNotifications(page);
    await hideSideBar(page);
    await page.waitForTimeout(1000);
    await page.screenshot({ path: outputPath, type: 'png' });
    console.log(`Captured: ${outputPath}`);
  }
  finally {
    await app.close();
  }
}

export async function captureQuickstartRunOutput(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  // Use the helloworld fixture (no CONFIGURATION — runs with defaults).
  // The run output panel opens automatically when Run Program is clicked.
  const app = await launchVSCode({ ...opts, filePath: QUICKSTART_HELLOWORLD_ST });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
    await dismissNotifications(page);
    await hideSideBar(page);
    await clickRunProgramAndWaitForOutput(page);
    await page.screenshot({ path: outputPath, type: 'png' });
    console.log(`Captured: ${outputPath}`);
  }
  finally {
    await app.close();
  }
}

export async function captureQuickstartTimerOutput(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  // Use the timer fixture which includes a CONFIGURATION block.
  const app = await launchVSCode({ ...opts, filePath: QUICKSTART_TIMER_ST });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
    await dismissNotifications(page);
    await hideSideBar(page);
    await clickRunProgramAndWaitForOutput(page);
    // Wait long enough for the TON timer to fire (PT = 500 ms, scan = 100 ms → ~5 scans).
    await page.waitForTimeout(2000);
    await page.screenshot({ path: outputPath, type: 'png' });
    console.log(`Captured: ${outputPath}`);
  }
  finally {
    await app.close();
  }
}

/** Encode an array of PNG Buffers (with per-frame delays in ms) into an APNG file. */
async function encodeApng(
  frames: { png: Buffer; delayMs: number }[],
  outputPath: string,
): Promise<void> {
  // Decode each PNG to raw RGBA so UPNG can re-encode them together.
  const rgbaFrames: ArrayBuffer[] = [];
  let width = 0;
  let height = 0;
  for (const { png } of frames) {
    const img = UPNG.decode(png.buffer as ArrayBuffer);
    width = img.width;
    height = img.height;
    rgbaFrames.push(UPNG.toRGBA8(img)[0]);
  }
  const delays = frames.map(f => f.delayMs);
  const apng = UPNG.encode(rgbaFrames, width, height, 0, delays);
  fs.writeFileSync(outputPath, Buffer.from(apng));
}

export async function captureQuickstartAnimation(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  // Open VS Code with the timer fixture — includes TON, PulseTimer, and CONFIGURATION.
  // The animation shows: file open with code → Run Program clicked → output updating with timer variables.
  const app = await launchVSCode({ ...opts, filePath: QUICKSTART_TIMER_ST });
  const frames: { png: Buffer; delayMs: number }[] = [];

  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
    await dismissNotifications(page);
    await hideSideBar(page);

    // Frame 1 — editor open, code visible, syntax highlighted, no errors.
    // Hold for 2 s so the viewer can read the code.
    await page.waitForTimeout(1000);
    frames.push({ png: await page.screenshot({ type: 'png' }), delayMs: 2000 });

    // Frame 2 — just before clicking Run Program; code lens visible.
    try {
      await page.waitForSelector('.codelens-decoration a', { timeout: 15000 });
    }
    catch {
      console.warn('Warning: Run Program code lens did not appear within timeout');
    }
    await page.waitForTimeout(500);
    frames.push({ png: await page.screenshot({ type: 'png' }), delayMs: 1500 });

    // Click Run Program.
    try {
      const runLens = page.locator('.codelens-decoration a', { hasText: 'Run Program' }).first();
      await runLens.click();
    }
    catch {
      console.warn('Warning: could not click Run Program code lens');
    }

    // Frame 3 — output panel opening / first scan cycle.
    try {
      await page.waitForSelector('.output-view-container .view-line', { timeout: 15000 });
    }
    catch {
      console.warn('Warning: IronPLC Run output did not appear within timeout');
    }
    await page.waitForTimeout(600);
    frames.push({ png: await page.screenshot({ type: 'png' }), delayMs: 1000 });

    // Frame 4 — a few more scan cycles have elapsed.
    await page.waitForTimeout(1500);
    frames.push({ png: await page.screenshot({ type: 'png' }), delayMs: 1000 });

    // Frame 5 — pause briefly then show Stop/Pause code lenses.
    await page.waitForTimeout(1500);
    frames.push({ png: await page.screenshot({ type: 'png' }), delayMs: 3000 });

    console.log(`Encoding APNG with ${frames.length} frames...`);
    await encodeApng(frames, outputPath);
    console.log(`Captured: ${outputPath}`);
  }
  finally {
    await app.close();
  }
}
