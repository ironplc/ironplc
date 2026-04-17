import * as path from 'path';
import { _electron, ElectronApplication, Page } from 'playwright';

const WINDOW_WIDTH = 1200;
const WINDOW_HEIGHT = 800;

interface LaunchOptions {
  vscodePath: string;
  extensionPath: string;
  userDataDir: string;
  filePath: string;
}

async function launchVSCode(opts: LaunchOptions): Promise<ElectronApplication> {
  const args = [
    opts.vscodePath,
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
    args,
    env: electronEnv,
  });
  return app;
}

async function waitForEditor(page: Page): Promise<void> {
  await page.waitForSelector('.monaco-editor', { timeout: 30000 });
  await page.waitForTimeout(2000);
}

async function hideSideBar(page: Page): Promise<void> {
  const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
  await page.keyboard.press(`${modifier}+b`);
  await page.waitForTimeout(500);
}

async function setWindowSize(page: Page): Promise<void> {
  await page.setViewportSize({ width: WINDOW_WIDTH, height: WINDOW_HEIGHT });
}

export async function captureSyntaxHighlighting(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  const validSt = path.resolve(__dirname, '../test/functional/resources/valid.st');
  const app = await launchVSCode({ ...opts, filePath: validSt });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
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
  const invalidSt = path.resolve(__dirname, 'fixtures/invalid.st');
  const app = await launchVSCode({ ...opts, filePath: invalidSt });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
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

export async function captureSettings(
  opts: Omit<LaunchOptions, 'filePath'>,
  outputPath: string,
): Promise<void> {
  const validSt = path.resolve(__dirname, '../test/functional/resources/valid.st');
  const app = await launchVSCode({ ...opts, filePath: validSt });
  try {
    const page = await app.firstWindow();
    await setWindowSize(page);
    await waitForEditor(page);
    const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
    await page.keyboard.press(`${modifier}+,`);
    await page.waitForSelector('.settings-editor', { timeout: 10000 });
    await page.waitForTimeout(500);
    const searchInput = page.locator('.settings-editor .suggest-input-container input');
    await searchInput.fill('ironplc');
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
