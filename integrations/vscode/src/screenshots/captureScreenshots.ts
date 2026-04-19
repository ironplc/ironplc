import * as path from 'path';
import { _electron, ElectronApplication, Page } from 'playwright';

const WINDOW_WIDTH = 1200;
const WINDOW_HEIGHT = 800;

// Source files live under src/, not out/, so resolve relative to the workspace root.
const VALID_ST = path.resolve(__dirname, '../../src/test/functional/resources/valid.st');
const INVALID_ST = path.resolve(__dirname, 'fixtures/invalid.st');

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
  // Hide the primary side bar.
  await page.keyboard.press(`${modifier}+b`);
  await page.waitForTimeout(500);
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
