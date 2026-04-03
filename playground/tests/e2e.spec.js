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

  test("start_when_running_multiple_cycles_then_shows_sparklines", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    count : INT;
  END_VAR
  count := count + 1;
END_PROGRAM
`);

    await page.fill('[data-testid="interval-input"]', "100");
    await page.click('[data-testid="start-btn"]');

    // Wait for sparkline canvases to appear (need at least 2 data points)
    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    await expect(variablesPanel.locator("canvas").first()).toBeVisible({ timeout: 10000 });

    await page.click('[data-testid="stop-btn"]');
  });

  test("examples_when_selected_then_changes_editor_content", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    const select = page.locator('[data-testid="examples-select"]');

    // Select "Boolean Logic" example
    await select.selectOption("Boolean Logic");
    const content = await editor.inputValue();
    expect(content).toContain("sensor_a");
    expect(content).toContain("AND");

    // Dropdown should reset to show "Examples" label
    await expect(select).toHaveValue("");
  });

  test("examples_when_selected_while_running_then_stops_execution", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    const select = page.locator('[data-testid="examples-select"]');

    await page.click('[data-testid="start-btn"]');

    // Wait for running
    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    await expect(variablesPanel).not.toContainText("Start a program", { timeout: 10000 });

    // Select a different example while running
    await select.selectOption("Arithmetic");

    // Should stop and be ready to start again
    await expect(page.locator('[data-testid="start-btn"]')).toBeEnabled();
    const content = await editor.inputValue();
    expect(content).toContain("product");
    expect(content).toContain("MOD");
  });

  test("examples_when_embed_mode_then_hidden", async ({ page }) => {
    await page.goto("/?embed=true");
    await expect(page.locator('[data-testid="status"]')).toHaveText("Ready", {
      timeout: 15000,
    });

    await expect(page.locator('[data-testid="examples-select"]')).toBeHidden();
  });

  test("dialect_select_when_loaded_then_defaults_to_2003", async ({ page }) => {
    const select = page.locator('[data-testid="dialect-select"]');
    await expect(select).toBeVisible();
    await expect(select).toHaveValue("2003");
  });

  test("dialect_select_when_embed_mode_then_hidden", async ({ page }) => {
    await page.goto("/?embed=true");
    await expect(page.locator('[data-testid="status"]')).toHaveText("Ready", {
      timeout: 15000,
    });

    await expect(page.locator('[data-testid="dialect-select"]')).toBeHidden();
  });

  test("dialect_badge_when_embed_with_dialect_2013_then_shows_badge", async ({ page }) => {
    await page.goto("/?embed=true&dialect=2013");
    await expect(page.locator('[data-testid="status"]')).toHaveText("Ready", {
      timeout: 15000,
    });

    const badge = page.locator('[data-testid="dialect-badge"]');
    await expect(badge).toBeVisible();
    await expect(badge).toHaveText("IEC 61131-3:2013");
  });

  test("start_when_dialect_2013_and_ltime_program_then_runs", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    const select = page.locator('[data-testid="dialect-select"]');

    await select.selectOption("2013");
    await editor.fill(`PROGRAM main
  VAR
    duration : LTIME;
  END_VAR
  duration := LTIME#500ms;
END_PROGRAM
`);

    await page.click('[data-testid="start-btn"]');

    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    await expect(variablesPanel).toContainText("duration", { timeout: 10000 });

    await page.click('[data-testid="stop-btn"]');
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
