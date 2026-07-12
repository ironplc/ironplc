import { test, expect } from "@playwright/test";

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
    const pausedText = (await cyclesDisplay.textContent()) ?? "";

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

  // A direct hardware-address write. Hardware I/O is out of scope for the
  // software playground VM, so this reliably produces P9999 in code generation.
  const P9999_PROGRAM = `PROGRAM main
  VAR
    x : BOOL;
  END_VAR
  %QX0.0 := TRUE;
END_PROGRAM
`;

  test("start_when_p9999_then_shows_submit_code_panel", async ({ page }) => {
    await page.locator('[data-testid="editor"]').fill(P9999_PROGRAM);
    await page.click('[data-testid="start-btn"]');

    const diagnosticsPanel = page.locator('[data-testid="diagnostics-panel"]');
    await expect(diagnosticsPanel).toContainText("P9999", { timeout: 10000 });

    // The report panel appears with an explicit consent line and a button that
    // says exactly what it does.
    const panel = page.locator('[data-testid="report-panel"]');
    await expect(panel).toBeVisible();
    await expect(panel).toContainText("may be published publicly");
    await expect(page.locator('[data-testid="submit-code-btn"]')).toHaveText("Submit Code");
  });

  test("start_when_ordinary_syntax_error_then_no_submit_code_panel", async ({ page }) => {
    await page.locator('[data-testid="editor"]').fill("PROGRAM main INVALID END_PROGRAM");
    await page.click('[data-testid="start-btn"]');

    const diagnosticsPanel = page.locator('[data-testid="diagnostics-panel"]');
    await expect(diagnosticsPanel).not.toContainText("No diagnostics", { timeout: 10000 });

    // Non-P9999 diagnostics must not offer the "Submit Code" affordance.
    await expect(page.locator('[data-testid="report-panel"]')).toHaveCount(0);
  });

  test("submit_code_when_clicked_then_captures_event_and_confirms", async ({ page }) => {
    await page.locator('[data-testid="editor"]').fill(P9999_PROGRAM);
    await page.click('[data-testid="start-btn"]');

    await expect(page.locator('[data-testid="report-panel"]')).toBeVisible({ timeout: 10000 });

    // Record PostHog captures. `ph()` reads window.posthog on every call, so
    // swapping it here is enough to intercept the submission event.
    await page.evaluate(() => {
      const w = window as unknown as { posthog?: unknown; __captured: unknown[] };
      w.__captured = [];
      const real = w.posthog as { register?: (p: unknown) => void } | undefined;
      w.posthog = {
        capture: (event: string, props: unknown) => w.__captured.push({ event, props }),
        register: real?.register ? real.register.bind(real) : () => {},
      };
    });

    await page.click('[data-testid="submit-code-btn"]');

    // The user gets a clear confirmation and cannot re-submit.
    await expect(page.locator('[data-testid="report-confirmation"]')).toBeVisible();
    await expect(page.locator('[data-testid="submit-code-btn"]')).toHaveCount(0);

    // The event carried the program source and the P9999 code.
    const captured = await page.evaluate(
      () => (window as unknown as { __captured: Array<{ event: string; props: { error_codes?: string[]; program?: string } }> }).__captured,
    );
    const report = captured.find((c) => c.event === "todo_report_submitted");
    expect(report).toBeTruthy();
    expect(report?.props.error_codes).toContain("P9999");
    expect(report?.props.program).toContain("%QX0.0");
  });

  test("submit_code_when_p9999_then_github_link_is_prefilled", async ({ page }) => {
    await page.locator('[data-testid="editor"]').fill(P9999_PROGRAM);
    await page.click('[data-testid="start-btn"]');

    const link = page.locator('[data-testid="report-github-link"]');
    await expect(link).toBeVisible({ timeout: 10000 });
    const href = (await link.getAttribute("href")) ?? "";
    expect(href).toContain("https://github.com/ironplc/ironplc/issues/new");
    expect(href).toContain("labels=P9999");
    // The program body is prefilled (URL-encoded) for this small program.
    expect(decodeURIComponent(href)).toContain("%QX0.0");
  });

  test("compile_finished_when_p9999_then_auto_reports_compiler_location_without_program", async ({ page }) => {
    await page.locator('[data-testid="editor"]').fill(P9999_PROGRAM);

    // Intercept captures before compiling.
    await page.evaluate(() => {
      const w = window as unknown as { posthog?: unknown; __captured: unknown[] };
      w.__captured = [];
      const real = w.posthog as { register?: (p: unknown) => void } | undefined;
      w.posthog = {
        capture: (event: string, props: unknown) => w.__captured.push({ event, props }),
        register: real?.register ? real.register.bind(real) : () => {},
      };
    });

    await page.click('[data-testid="start-btn"]');
    await expect(page.locator('[data-testid="report-panel"]')).toBeVisible({ timeout: 10000 });

    const captured = await page.evaluate(
      () =>
        (window as unknown as {
          __captured: Array<{
            event: string;
            props: { success?: boolean; error_locations?: string[]; program?: string };
          }>;
        }).__captured,
    );
    const finished = captured.find((c) => c.event === "compile_finished");
    expect(finished).toBeTruthy();
    expect(finished?.props.success).toBe(false);
    // The compiler file#line is reported automatically...
    expect(finished?.props.error_locations?.some((l) => /\.rs#L\d+/.test(l))).toBe(true);
    // ...but the program itself is never on this automatic event.
    expect(finished?.props.program).toBeUndefined();
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

    // Wait for sparkline canvases to appear (need at least 2 data points).
    // Assert on uPlot's bitmap attributes rather than layout visibility:
    // uPlot only sets the canvas width/height attributes after commit()
    // sizes the wrap, so seeing them proves the chart fully rendered. This
    // avoids a race where renderVariables replaces innerHTML every 500ms
    // and Playwright's toBeVisible occasionally sees a zero-sized layout box.
    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    const canvas = variablesPanel.locator("canvas").first();
    await expect(canvas).toBeAttached({ timeout: 10000 });
    await expect(canvas).toHaveAttribute("width", "120");
    await expect(canvas).toHaveAttribute("height", "24");

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

  test("dialect_badge_when_allows_set_then_shows_custom_with_tooltip", async ({ page }) => {
    await page.goto("/?embed=true&dialect=2013&allows=sizeof,c-style-comments");
    await expect(page.locator('[data-testid="status"]')).toHaveText("Ready", {
      timeout: 15000,
    });

    const badge = page.locator('[data-testid="dialect-badge"]');
    await expect(badge).toBeVisible();
    await expect(badge).toHaveText("Custom");
    await expect(badge).toHaveAttribute(
      "title",
      "IEC 61131-3:2013 + --allow-sizeof, --allow-c-style-comments",
    );
  });

  test("start_when_allows_sizeof_set_and_strict_dialect_then_compiles", async ({ page }) => {
    await page.goto("/?embed=true&dialect=2013&allows=sizeof");
    await expect(page.locator('[data-testid="status"]')).toHaveText("Ready", {
      timeout: 15000,
    });

    const editor = page.locator('[data-testid="editor"]');
    await editor.fill(`PROGRAM main
  VAR
    x : INT;
    s : DINT;
  END_VAR
  s := SIZEOF(x);
END_PROGRAM
`);

    await page.click('[data-testid="start-btn"]');

    const variablesPanel = page.locator('[data-testid="variables-panel"]');
    await expect(variablesPanel).toContainText("2", { timeout: 10000 });

    await page.click('[data-testid="stop-btn"]');
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

  test("editor_when_loaded_then_shows_line_number_gutter", async ({ page }) => {
    const gutter = page.locator("#editor-gutter");
    await expect(gutter).toBeVisible();

    const editor = page.locator('[data-testid="editor"]');
    const source = await editor.inputValue();
    const expectedLines = Math.max(1, source.split("\n").length);

    const gutterText = (await gutter.textContent()) ?? "";
    const gutterLines = gutterText.split("\n");
    expect(gutterLines.length).toBe(expectedLines);
    expect(gutterLines[0]).toBe("1");
    expect(gutterLines[gutterLines.length - 1]).toBe(String(expectedLines));
  });

  test("editor_when_content_changed_then_line_numbers_update", async ({ page }) => {
    const editor = page.locator('[data-testid="editor"]');
    await editor.fill("line one\nline two\nline three");

    const gutter = page.locator("#editor-gutter");
    await expect(gutter).toHaveText("1\n2\n3");
  });
});
