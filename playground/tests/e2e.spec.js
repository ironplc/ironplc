// @ts-check
const { test, expect } = require("@playwright/test");

test.describe("IronPLC Playground", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    // Wait for WASM to load
    await expect(page.locator('[data-testid="status"]')).toHaveText("Ready", {
      timeout: 15000,
    });
  });

  test("page_when_loaded_then_shows_editor_and_ready_status", async ({ page }) => {
    await expect(page).toHaveTitle(/IronPLC/);
    await expect(page.locator('[data-testid="editor"]')).toBeVisible();
    await expect(page.locator('[data-testid="start-btn"]')).toBeEnabled();
    await expect(page.locator('[data-testid="stop-btn"]')).toBeDisabled();
    await expect(page.locator('[data-testid="pause-btn"]')).toBeDisabled();
  });

  test("start_when_valid_program_then_shows_variable_values", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 10;
  y := x + 32;
END_PROGRAM
`);

    await page.click('[data-testid="start-btn"]');

    // Wait for variables to appear
    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    await expect(variablesPanel).toContainText("10", { timeout: 10000 });
    await expect(variablesPanel).toContainText("42");

    // Stop to clean up
    await page.click('[data-testid="stop-btn"]');
  });

  test("start_when_syntax_error_then_shows_diagnostics", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill("PROGRAM main INVALID END_PROGRAM");

    await page.click('[data-testid="start-btn"]');

    const diagnosticsPanel = page.locator('[data-testid="diagnostics-panel"]');
    await expect(diagnosticsPanel).toBeVisible({ timeout: 10000 });
    await expect(diagnosticsPanel).not.toContainText("No diagnostics");

    // Start button should be re-enabled after compilation failure
    await expect(page.locator('[data-testid="start-btn"]')).toBeEnabled();
  });

  test("start_when_running_then_cycles_increment_over_time", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    count : DINT;
  END_VAR
  count := count + 1;
END_PROGRAM
`);

    // Use a short interval to accumulate cycles quickly
    await page.fill('[data-testid="interval-input"]', "100");
    await page.click('[data-testid="start-btn"]');

    // Wait for cycles to accumulate
    const cyclesDisplay = page.locator('[data-testid="cycles-display"]');
    await expect(cyclesDisplay).not.toHaveText("0 cycles", { timeout: 10000 });

    // Duration should be counting up
    const durationDisplay = page.locator('[data-testid="duration-display"]');
    await expect(durationDisplay).not.toHaveText("0.0s", { timeout: 5000 });

    await page.click('[data-testid="stop-btn"]');
  });

  test("stop_when_clicked_then_resets_state", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 1;
END_PROGRAM
`);

    await page.click('[data-testid="start-btn"]');

    // Wait for at least one cycle
    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    await expect(variablesPanel).toContainText("1", { timeout: 10000 });

    await page.click('[data-testid="stop-btn"]');
    await expect(page.locator('[data-testid="status"]')).toContainText("Stopped", {
      timeout: 10000,
    });

    // Start button should be re-enabled
    await expect(page.locator('[data-testid="start-btn"]')).toBeEnabled();
    await expect(page.locator('[data-testid="stop-btn"]')).toBeDisabled();
  });

  test("stop_when_clicked_then_resets_memory", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    count : DINT;
  END_VAR
  count := count + 1;
END_PROGRAM
`);

    await page.fill('[data-testid="interval-input"]', "100");
    await page.click('[data-testid="start-btn"]');

    // Wait for count to be > 1
    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    await expect(variablesPanel).not.toContainText("Start a program", { timeout: 10000 });

    await page.click('[data-testid="stop-btn"]');

    // Start again - count should restart from 1
    await page.click('[data-testid="start-btn"]');
    await expect(variablesPanel).toContainText("1", { timeout: 10000 });

    await page.click('[data-testid="stop-btn"]');
  });

  test("pause_when_clicked_then_stops_cycle_counting", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    count : DINT;
  END_VAR
  count := count + 1;
END_PROGRAM
`);

    await page.fill('[data-testid="interval-input"]', "100");
    await page.click('[data-testid="start-btn"]');

    // Wait for some cycles
    const cyclesDisplay = page.locator('[data-testid="cycles-display"]');
    await expect(cyclesDisplay).not.toHaveText("0 cycles", { timeout: 10000 });

    // Pause
    await page.click('[data-testid="pause-btn"]');
    await expect(page.locator('[data-testid="status"]')).toContainText("Paused", {
      timeout: 5000,
    });

    // Record cycle count after pause
    const pausedText = await cyclesDisplay.textContent();

    // Wait a moment and verify count hasn't changed
    await page.waitForTimeout(600);
    await expect(cyclesDisplay).toHaveText(pausedText, { timeout: 1000 });

    // Resume
    await page.click('[data-testid="pause-btn"]');
    await expect(page.locator('[data-testid="status"]')).toContainText("Running", {
      timeout: 5000,
    });

    // Cycles should continue
    await expect(cyclesDisplay).not.toHaveText(pausedText, { timeout: 10000 });

    await page.click('[data-testid="stop-btn"]');
  });

  test("editor_when_default_content_then_contains_example_program", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    const content = await editor.inputValue();
    expect(content).toContain("PROGRAM main");
    expect(content).toContain("count := count + 1");
  });

  test("source_change_when_running_then_stops_execution", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 10;
END_PROGRAM
`);

    await page.click('[data-testid="start-btn"]');

    // Wait for running
    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    await expect(variablesPanel).toContainText("10", { timeout: 10000 });

    // Change source while running
    await editor.fill(`PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 99;
END_PROGRAM
`);

    // Should stop and show message
    await expect(page.locator('[data-testid="status"]')).toContainText("Source changed", {
      timeout: 10000,
    });
    await expect(page.locator('[data-testid="start-btn"]')).toBeEnabled();
  });

  test("start_when_syntax_error_then_diagnostic_code_links_to_documentation", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill("PROGRAM main INVALID END_PROGRAM");

    await page.click('[data-testid="start-btn"]');

    const diagnosticsPanel = page.locator('[data-testid="diagnostics-panel"]');
    await expect(diagnosticsPanel).not.toContainText("No diagnostics", { timeout: 10000 });

    // P-code should be a clickable link
    const link = diagnosticsPanel.locator("a.diagnostic-code");
    await expect(link).toBeVisible();
    await expect(link).toHaveAttribute("href", /https:\/\/www\.ironplc\.com\/reference\/compiler\/problems\/P\d{4}\.html\?version=/);
    await expect(link).toHaveAttribute("target", "_blank");

    // Diagnostic message should include the label context
    const message = diagnosticsPanel.locator(".diagnostic-message");
    await expect(message).not.toHaveText("");
  });

  test("embed_when_loaded_then_shows_start_and_stop_only", async ({ page }) => {
    await page.goto("/?embed=true");
    await expect(page.locator('[data-testid="status"]')).toHaveText("Ready", {
      timeout: 15000,
    });

    // Start and stop should be visible
    await expect(page.locator('[data-testid="start-btn"]')).toBeVisible();
    await expect(page.locator('[data-testid="stop-btn"]')).toBeVisible();

    // Pause should be hidden
    await expect(page.locator('[data-testid="pause-btn"]')).toBeHidden();

    // Interval input should be visible but disabled
    await expect(page.locator('[data-testid="interval-input"]')).toBeDisabled();

    // Duration and cycles should be visible
    await expect(page.locator('[data-testid="duration-display"]')).toBeVisible();
    await expect(page.locator('[data-testid="cycles-display"]')).toBeVisible();
  });
});
