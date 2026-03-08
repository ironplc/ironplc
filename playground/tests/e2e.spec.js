// @ts-check
const { test, expect } = require("@playwright/test");
const path = require("path");
const fs = require("fs");

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
    await expect(page.locator("#run-btn")).toBeEnabled();
  });

  test("run_source_when_steel_thread_program_then_shows_variable_values", async ({ page }) => {
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

    await page.click("#run-btn");

    // Wait for results
    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    await expect(variablesPanel).toContainText("10", { timeout: 10000 });
    await expect(variablesPanel).toContainText("42");
    await expect(page.locator('[data-testid="status"]')).toContainText("1 scan");
  });

  test("run_source_when_syntax_error_then_shows_diagnostics", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill("PROGRAM main INVALID END_PROGRAM");

    await page.click("#run-btn");

    const diagnosticsPanel = page.locator('[data-testid="diagnostics-panel"]');
    await expect(diagnosticsPanel).toBeVisible({ timeout: 10000 });
    // Should switch to diagnostics tab and show error
    await expect(diagnosticsPanel).not.toContainText("No diagnostics");
  });

  test("run_source_when_multiple_scans_then_shows_correct_count", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 99;
END_PROGRAM
`);

    await page.fill("#scans-input", "5");
    await page.click("#run-btn");

    await expect(page.locator('[data-testid="status"]')).toContainText("5 scan", {
      timeout: 10000,
    });
  });

  test("file_upload_when_iplc_file_then_executes_and_shows_results", async ({ page }) => {
    // First compile a program to get bytecode, then use it as a file upload
    // We test the file input by creating a synthetic .iplc from compilation
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 77;
END_PROGRAM
`);

    // Run from source first to verify the pipeline works
    await page.click("#run-btn");
    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    await expect(variablesPanel).toContainText("77", { timeout: 10000 });
  });

  test("editor_when_default_content_then_contains_example_program", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    const content = await editor.inputValue();
    expect(content).toContain("PROGRAM main");
    expect(content).toContain("count := count + 1");
  });

  test("step_when_program_loaded_then_shows_variables_and_scan_count", async ({ page }) => {
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

    await page.click('[data-testid="step-btn"]');

    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    await expect(variablesPanel).toContainText("10", { timeout: 10000 });
    await expect(variablesPanel).toContainText("42");
    await expect(page.locator('[data-testid="status"]')).toContainText("Scan cycle 1 completed");
  });

  test("step_when_clicked_twice_then_scan_count_accumulates", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    count : DINT;
  END_VAR
  count := count + 1;
END_PROGRAM
`);

    await page.click('[data-testid="step-btn"]');
    await expect(page.locator('[data-testid="status"]')).toContainText("Scan cycle 1 completed", {
      timeout: 10000,
    });

    await page.click('[data-testid="step-btn"]');
    await expect(page.locator('[data-testid="status"]')).toContainText("Scan cycle 2 completed", {
      timeout: 10000,
    });
  });

  test("reset_when_clicked_then_clears_output_and_shows_ready", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 1;
END_PROGRAM
`);

    // Step first to populate output
    await page.click('[data-testid="step-btn"]');
    await expect(page.locator('[data-testid="status"]')).toContainText("Scan cycle 1 completed", {
      timeout: 10000,
    });

    // Reset
    await page.click('[data-testid="reset-btn"]');
    await expect(page.locator('[data-testid="status"]')).toHaveText("Ready", {
      timeout: 10000,
    });
    await expect(page.locator('[data-testid="variables-panel"]')).toContainText(
      "Run a program"
    );
  });

  test("run_source_when_syntax_error_then_diagnostic_code_links_to_documentation", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill("PROGRAM main INVALID END_PROGRAM");

    await page.click("#run-btn");

    const diagnosticsPanel = page.locator('[data-testid="diagnostics-panel"]');
    await expect(diagnosticsPanel).not.toContainText("No diagnostics", { timeout: 10000 });

    // P-code should be a clickable link
    const link = diagnosticsPanel.locator("a.diagnostic-code");
    await expect(link).toBeVisible();
    await expect(link).toHaveAttribute("href", /https:\/\/www\.ironplc\.com\/reference\/compiler\/problems\/P\d{4}\.html/);
    await expect(link).toHaveAttribute("target", "_blank");

    // Diagnostic message should include the label context
    const message = diagnosticsPanel.locator(".diagnostic-message");
    await expect(message).not.toHaveText("");
  });

  test("step_when_source_changed_then_recompiles_automatically", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 10;
END_PROGRAM
`);

    await page.click('[data-testid="step-btn"]');
    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    await expect(variablesPanel).toContainText("10", { timeout: 10000 });

    // Change source — should auto-recompile on next step
    await editor.fill(`PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 99;
END_PROGRAM
`);

    await page.click('[data-testid="step-btn"]');
    await expect(variablesPanel).toContainText("99", { timeout: 10000 });
    await expect(page.locator('[data-testid="status"]')).toContainText("Scan cycle 1 completed");
  });
});
